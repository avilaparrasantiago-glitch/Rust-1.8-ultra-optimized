use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::{Duration, timeout},
};

#[derive(Default)]
pub struct Metrics {
    pub players: AtomicUsize,
    pub ticks: AtomicU64,
    pub overruns: AtomicU64,
    pub buckets: [AtomicU64; 12],
    pub wire_bytes: AtomicU64,
    pub body_bytes: AtomicU64,
    pub dropped: AtomicU64,
    pub generated_columns: AtomicU64,
    pub generation_ns: AtomicU64,
    pub chunk_hits: AtomicU64,
    pub chunk_lookups: AtomicU64,
}
const MICROS: [u64; 11] = [
    100, 250, 500, 1000, 2500, 5000, 10000, 20000, 35000, 50000, 100000,
];
impl Metrics {
    pub fn tick(&self, us: u64) {
        let n = MICROS.partition_point(|&x| x < us);
        self.buckets[n].fetch_add(1, Relaxed);
        self.ticks.fetch_add(1, Relaxed);
        if us > 50000 {
            self.overruns.fetch_add(1, Relaxed);
        }
    }
    pub fn render(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let mut cumulative = 0;
        for (i, v) in self.buckets.iter().enumerate() {
            cumulative += v.load(Relaxed);
            let bound = MICROS
                .get(i)
                .map_or_else(|| "+Inf".into(), |n| format!("{}", *n as f64 / 1e6));
            writeln!(
                s,
                "coral_tick_seconds_bucket{{le=\"{bound}\"}} {cumulative}"
            )
            .unwrap();
        }
        for (name, n) in [
            ("tick_seconds_count", cumulative),
            ("players", self.players.load(Relaxed) as u64),
            ("tick_overruns_total", self.overruns.load(Relaxed)),
            ("body_bytes_total", self.body_bytes.load(Relaxed)),
            ("wire_bytes_total", self.wire_bytes.load(Relaxed)),
            ("queue_overflows_total", self.dropped.load(Relaxed)),
            (
                "generated_columns_total",
                self.generated_columns.load(Relaxed),
            ),
            (
                "generation_nanoseconds_total",
                self.generation_ns.load(Relaxed),
            ),
            ("chunk_cache_hits_total", self.chunk_hits.load(Relaxed)),
            ("chunk_lookups_total", self.chunk_lookups.load(Relaxed)),
        ] {
            writeln!(s, "coral_{name} {n}").unwrap();
        }
        s
    }
}
pub async fn serve(listener: TcpListener, metrics: std::sync::Arc<Metrics>) {
    while let Ok((mut socket, _)) = listener.accept().await {
        let m = metrics.clone();
        let _=timeout(Duration::from_secs(1),async move{
            let mut req=[0;1024];let mut used=0;
            while used<req.len() && !req[..used].windows(2).any(|w|w==b"\r\n"){
                let n=socket.read(&mut req[used..]).await?;if n==0{break;}used+=n;
            }
            let (status,body)=if req[..used].starts_with(b"GET /metrics HTTP/1."){("200 OK",m.render())}else if req[..used].starts_with(b"GET /health HTTP/1."){("200 OK","ok\n".into())}else{("404 Not Found",String::new())};
            socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await
        }).await;
    }
}
