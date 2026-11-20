use crate::{
    engine::{Manager, Player},
    identity::{self, Forwarding, Identity},
    input, packets as p,
    wire::{self, Cursor, Packet, WriteWire, invalid},
};
use bytes::{BufMut, Bytes, BytesMut};
use std::{
    cell::RefCell,
    collections::HashSet,
    io,
    net::IpAddr,
    rc::Rc,
    sync::{
        Arc,
        atomic::Ordering::{Acquire, Relaxed},
    },
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
    time::{MissedTickBehavior, interval, timeout},
};
use uuid::Uuid;

#[cfg(not(target_os = "linux"))]
use tokio::net::{TcpListener, TcpStream};
#[cfg(target_os = "linux")]
use tokio_uring::net::{TcpListener, TcpStream};

struct Socket(TcpStream);
impl Socket {
    async fn read(&self, mut data: Vec<u8>) -> io::Result<(usize, Vec<u8>)> {
        #[cfg(target_os = "linux")]
        {
            let (result, buf) = self.0.read(data).await;
            Ok((result?, buf))
        }
        #[cfg(not(target_os = "linux"))]
        loop {
            self.0.readable().await?;
            match self.0.try_read(&mut data) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                r => return Ok((r?, data)),
            }
        }
    }
    async fn write(&self, data: Vec<u8>) -> io::Result<Vec<u8>> {
        #[cfg(target_os = "linux")]
        {
            let (result, buf) = self.0.write_all(data).await;
            result?;
            Ok(buf)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let mut offset = 0;
            while offset < data.len() {
                self.0.writable().await?;
                match self.0.try_write(&data[offset..]) {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(n) => offset += n,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(data)
        }
    }
    async fn send(&self, packet: Packet, threshold: Option<usize>) -> io::Result<()> {
        self.write(wire::frame(packet.body(), threshold)?.to_vec())
            .await?;
        Ok(())
    }
}

struct Workers {
    pool: rayon::ThreadPool,
    slots: Arc<Semaphore>,
}
impl Workers {
    async fn batch(
        self: &Arc<Self>,
        packets: Vec<Packet>,
        mut buf: Vec<u8>,
        t: usize,
    ) -> io::Result<(Vec<Packet>, Vec<u8>, usize)> {
        let guard = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(io::Error::other)?;
        let (tx, rx) = oneshot::channel();
        self.pool.spawn(move || {
            let _guard = guard;
            if tx.is_closed() {
                return;
            }
            let result = (|| {
                let mut total = 0;
                buf.clear();
                for p in &packets {
                    total += p.body().len();
                    buf.extend_from_slice(&p.encoded(t)?);
                }
                Ok((packets, buf, total))
            })();
            let _ = tx.send(result);
        });
        rx.await.map_err(io::Error::other)?
    }
}

struct Network {
    profiles: Arc<crate::profiles::Profiles>,
    manager: Arc<Manager>,
    workers: Arc<Workers>,
    gate: Arc<Semaphore>,
    queue: Arc<Semaphore>,
    forward: Option<Forwarding>,
    ids: RefCell<HashSet<Uuid>>,
}
struct Login<'a> {
    net: &'a Network,
    uuid: Uuid,
}
impl Drop for Login<'_> {
    fn drop(&mut self) {
        self.net.ids.borrow_mut().remove(&self.uuid);
    }
}
struct Online(Arc<Manager>);
impl Drop for Online {
    fn drop(&mut self) {
        self.0.metrics.players.fetch_sub(1, Relaxed);
    }
}

pub async fn serve(
    manager: Arc<Manager>,
    profiles: Arc<crate::profiles::Profiles>,
    forward: Option<Forwarding>,
) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    let listener = TcpListener::bind(manager.cfg.listen)?;
    #[cfg(not(target_os = "linux"))]
    let listener = TcpListener::bind(manager.cfg.listen).await?;
    let metrics_listener = tokio::net::TcpListener::bind(manager.cfg.metrics).await?;
    tokio::task::spawn_local(crate::metrics::serve(
        metrics_listener,
        manager.metrics.clone(),
    ));
    let n = std::thread::available_parallelism().map_or(1, |v| (v.get() / 3).clamp(1, 16));
    let workers = Arc::new(Workers {
        pool: rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .thread_name(|i| format!("deflate-{i}"))
            .build()
            .map_err(io::Error::other)?,
        slots: Arc::new(Semaphore::new(n * 4)),
    });
    let net = Rc::new(Network {
        profiles,
        gate: Arc::new(Semaphore::new(manager.cfg.max_players)),
        queue: Arc::new(Semaphore::new(1024)),
        manager,
        workers,
        forward,
        ids: Default::default(),
    });
    let handshakes = Arc::new(Semaphore::new(512));
    eprintln!("Coral Protocol 47 listening on {}", net.manager.cfg.listen);
    loop {
        let (stream, peer) = listener.accept().await?;
        let Ok(permit) = handshakes.clone().try_acquire_owned() else {
            continue;
        };
        stream.set_nodelay(true)?;
        let net = net.clone();
        tokio::task::spawn_local(async move {
            let sock = Socket(stream);
            let (tx, rx) = mpsc::channel(4);
            // One persistent reader owns the TCP byte stream through handshake, queue and play.
            tokio::select! {
                _=read_frames(&sock,tx)=>{},
                _=session(&sock,rx,&net,peer.ip(),permit)=>{},
            }
        });
    }
}
async fn read_frames(sock: &Socket, tx: mpsc::Sender<Bytes>) -> io::Result<()> {
    let mut pending = BytesMut::with_capacity(8192);
    let mut buf = vec![0u8; 8192];
    loop {
        while let Some(frame) = wire::split_frame(&mut pending)? {
            tx.send(frame)
                .await
                .map_err(|_| io::ErrorKind::BrokenPipe)?;
        }
        let (n, b) = sock.read(buf).await?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        pending.extend_from_slice(&b[..n]);
        buf = b;
    }
}
async fn next(rx: &mut mpsc::Receiver<Bytes>) -> io::Result<Bytes> {
    rx.recv().await.ok_or(io::ErrorKind::UnexpectedEof.into())
}
async fn handshake(
    sock: &Socket,
    rx: &mut mpsc::Receiver<Bytes>,
    net: &Network,
    ip: IpAddr,
) -> io::Result<Option<Identity>> {
    let b = next(rx).await?;
    let mut r = Cursor(&b);
    if r.var()? != 0 {
        return Err(invalid("expected handshake"));
    }
    let version = r.var()?;
    let host = r.text(255)?;
    r.short()?;
    let state = r.var()?;
    r.end()?;
    if state == 1 {
        let b = next(rx).await?;
        let mut r = Cursor(&b);
        if r.var()? != 0 {
            return Err(invalid("expected status request"));
        }
        r.end()?;
        let cfg = &net.manager.cfg;
        let reply = serde_json::json!({"version":{"name":"Coral 1.8.8ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ1.8.9","protocol":47},"players":{"max":cfg.max_players,"online":net.manager.metrics.players.load(Relaxed)},"description":{"text":cfg.motd}});
        sock.send(Packet::new(0, |w| w.string(&reply.to_string())), None)
            .await?;
        let b = next(rx).await?;
        let mut r = Cursor(&b);
        if r.var()? != 1 {
            return Err(invalid("expected ping"));
        }
        let token = r.long()?;
        r.end()?;
        sock.send(Packet::new(1, |w| w.put_i64(token)), None)
            .await?;
        return Ok(None);
    }
    if state != 2 {
        return Err(invalid("invalid handshake state"));
    }
    if version != 47 {
        sock.send(p::disconnect(0, "Select Minecraft 1.8.8 or 1.8.9"), None)
            .await?;
        return Ok(None);
    }
    let start = next(rx).await?;
    let mut r = Cursor(&start);
    if r.var()? != 0 {
        return Err(invalid("expected Login Start"));
    }
    let name = r.text(16)?;
    r.end()?;
    if !identity::username(name) {
        return Err(invalid("invalid username"));
    }
    let identity = match &net.forward {
        Some(f) => f.verify(host, name)?,
        None => identity::offline(name, ip),
    };
    Ok(Some(identity))
}
async fn session(
    sock: &Socket,
    mut rx: mpsc::Receiver<Bytes>,
    net: &Network,
    ip: IpAddr,
    handshake_permit: OwnedSemaphorePermit,
) -> io::Result<()> {
    let Some(identity) =
        timeout(Duration::from_secs(8), handshake(sock, &mut rx, net, ip)).await??
    else {
        return Ok(());
    };
    if !net.ids.borrow_mut().insert(identity.uuid) {
        sock.send(p::disconnect(0, "Already connected"), None)
            .await?;
        return Ok(());
    }
    let _login = Login {
        net,
        uuid: identity.uuid,
    };
    let threshold = if net.gate.available_permits() < net.manager.cfg.max_players / 4 {
        512
    } else {
        256
    };
    sock.send(Packet::new(3, |w| w.var(threshold as i32)), None)
        .await?;
    sock.send(
        Packet::new(2, |w| {
            w.string(&identity.uuid.to_string());
            w.string(&identity.name);
        }),
        Some(threshold),
    )
    .await?;
    drop(handshake_permit);
    let id = net.manager.next.fetch_add(1, Relaxed);
    let (permit, in_limbo) = match net.gate.clone().try_acquire_owned() {
        Ok(p) => (p, false),
        Err(_) => {
            let Ok(_queued) = net.queue.clone().try_acquire_owned() else {
                sock.send(p::disconnect(0x40, "Queue full"), Some(threshold))
                    .await?;
                return Ok(());
            };
            for packet in [
                p::join(id, 3),
                p::abilities(3, true),
                p::position([0.5, 64., 0.5], [0.; 2]),
                p::text("Waiting for an arena slot"),
            ] {
                sock.send(packet, Some(threshold)).await?;
            }
            let acquire = net.gate.clone().acquire_owned();
            tokio::pin!(acquire);
            let until = tokio::time::sleep(Duration::from_secs(300));
            tokio::pin!(until);
            let mut keep = interval(Duration::from_secs(10));
            let mut pending = None;
            let mut token = 0i32;
            loop {
                tokio::select! {
                    p=&mut acquire=>break (p.map_err(io::Error::other)?,true),
                    _=&mut until=>return Ok(()),
                    _=keep.tick()=>{if pending.is_some(){return Ok(());}token=token.wrapping_add(1);pending=Some(token);sock.send(Packet::new(0,|w|w.var(token)),Some(threshold)).await?;}
                    raw=next(&mut rx)=>{let payload=wire::unpack(raw?,Some(threshold))?;if let input::Action::KeepAlive(n)=input::decode(&payload)? && pending==Some(n){pending=None;}}
                }
            }
        }
    };
    let _permit = permit;
    let (actions, inbox) = crossbeam_channel::bounded(96);
    let (out, mut packets) = mpsc::channel(192);
    eprintln!("{} joined from {}", identity.name, identity.ip);
    let (lease, profile) = match net.profiles.claim(identity.uuid).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Profile admission failed: {e}");
            sock.send(
                p::disconnect(0x40, "Profile temporarily unavailable"),
                Some(threshold),
            )
            .await?;
            return Ok(());
        }
    };
    let mut player = Box::new(Player::new(id, identity, inbox, out));
    player.restore(lease, profile);
    if net
        .manager
        .enter(&net.manager.cfg.worlds[0], player, in_limbo)
        .is_err()
    {
        sock.send(p::disconnect(0x40, "Arena admission busy"), Some(threshold))
            .await?;
        return Ok(());
    }
    net.manager.metrics.players.fetch_add(1, Relaxed);
    let _online = Online(net.manager.clone());
    let reader = async {
        loop {
            let raw = next(&mut rx).await?;
            // Supported client actions are small, including the 32 KiB plugin payload limit.
            let mut r = Cursor(&raw);
            if r.var()? as usize > 132_000 {
                return Err(invalid("inbound play body too large"));
            }
            let payload = wire::unpack(raw, Some(threshold))?;
            let action = input::decode(&payload)?;
            if actions.try_send(action).is_err() {
                net.manager.metrics.dropped.fetch_add(1, Relaxed);
                return Err(invalid("input queue full"));
            }
        }
    };
    let writer = async {
        let mut clock = interval(Duration::from_millis(50));
        clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut batch = Vec::with_capacity(192);
        let mut buf = Vec::with_capacity(16 * 1024);
        loop {
            clock.tick().await;
            if net.manager.stopping.load(Acquire) {
                return Ok(());
            }
            while batch.len() < 192 {
                match packets.try_recv() {
                    Ok(p) => batch.push(p),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(_) => return Ok(()),
                }
            }
            if batch.is_empty() {
                continue;
            }
            let (mut items, bytes, logical) = net.workers.batch(batch, buf, threshold).await?;
            net.manager
                .metrics
                .body_bytes
                .fetch_add(logical as u64, Relaxed);
            net.manager
                .metrics
                .wire_bytes
                .fetch_add(bytes.len() as u64, Relaxed);
            buf = timeout(Duration::from_secs(5), sock.write(bytes)).await??;
            items.clear();
            batch = items;
        }
    };
    tokio::select! {r=reader=>r,r=writer=>r}
}
