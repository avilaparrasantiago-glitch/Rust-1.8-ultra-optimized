use super::Player;
use crate::arena::{ColumnPos, Template, column};
use crate::wire::{Cursor, Packet, WriteWire};
use bytes::BufMut;
use crossbeam_channel::{Receiver, Sender, bounded};
use rayon::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const TEMPLATE_JOBS: usize = 8;
const CACHE_BYTES: usize = 16 << 20;
const CACHE_ENTRIES: usize = 512;
type Key = (ColumnPos, Option<ColumnPos>);
type Job = (Key, Option<[Packet; 2]>, Arc<AtomicBool>);

fn bulk(packets: &[Packet; 2]) -> Option<Packet> {
    let decode = |packet: &Packet| {
        let mut c = Cursor(packet.body());
        if c.var().ok()? != 0x21 {
            return None;
        }
        let pos = (c.int().ok()?, c.int().ok()?);
        if !c.bool().ok()? {
            return None;
        }
        let mask = c.short().ok()? as u16;
        let len = usize::try_from(c.var().ok()?).ok()?;
        if c.0.len() != len {
            return None;
        }
        Some((pos, mask, len))
    };
    let columns = [decode(&packets[0])?, decode(&packets[1])?];
    let sky = columns
        .iter()
        .any(|&(_, mask, len)| len > 256 + mask.count_ones() as usize * 10240);
    Some(Packet::with_capacity(
        0x26,
        23 + columns[0].2 + columns[1].2,
        |w| {
            w.put_u8(u8::from(sky));
            w.var(2);
            for ((x, z), mask, _) in columns {
                w.put_i32(x);
                w.put_i32(z);
                w.put_u16(mask);
            }
            for (packet, (_, _, len)) in packets.iter().zip(columns) {
                let body = packet.body();
                w.extend_from_slice(&body[body.len() - len..]);
            }
        },
    ))
}

pub(super) fn in_range(pos: ColumnPos, center: ColumnPos, radius: u8) -> bool {
    pos.0.abs_diff(center.0).max(pos.1.abs_diff(center.1)) <= u32::from(radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::WorldDistances, identity::offline};
    use std::time::{Duration, Instant};

    #[test]
    fn distant_template_delivery_and_teleport_cancellation() -> std::io::Result<()> {
        let limits = WorldDistances {
            simulation_distance: 8,
            horizon_view_distance: 64,
        };
        let pos = (64, 0);
        let mut template = Template::flat(0);
        let origin = template.columns.remove(&(0, 0)).unwrap();
        template.columns.insert(pos, origin);
        let template = Arc::new(template);
        let mut horizon = Horizon::new(template.clone(), limits)?;
        let (_, input) = crossbeam_channel::bounded(1);
        let (output, _rx) = tokio::sync::mpsc::channel(8);
        let mut player = Player::new(1, offline("Test", [127, 0, 0, 1].into()), input, output);
        let route = horizon.route([1., 0.]);
        assert_eq!(route.len(), 129 * 129 - 17 * 17);
        assert_eq!(route.last(), Some(&(9, 0)));
        horizon.request_template(pos);
        let until = Instant::now() + Duration::from_secs(10);
        while horizon.get(pos).is_none() {
            assert!(Instant::now() < until);
            horizon.maintain(std::slice::from_ref(&player), 64);
            std::thread::yield_now();
        }
        assert_eq!(
            horizon.get(pos).unwrap().body(),
            template.columns[&pos].build_packet(pos).body()
        );
        assert!(!template.cached(pos));
        player.teleport([-4096., 70., 0.]);
        horizon.request_template((63, 0));
        horizon.maintain(std::slice::from_ref(&player), 64);
        assert!(horizon.get(pos).is_none());
        assert!(horizon.pending.values().all(|t| !t.load(Ordering::Acquire)));
        Ok(())
    }

    #[test]
    fn bulk_matches_the_template_packet() {
        let template = Template::flat(1);
        let positions = [(0, 0), (1, 0)];
        let packets = positions.map(|p| template.columns[&p].build_packet(p));
        assert_eq!(
            bulk(&packets).unwrap().body(),
            template.bulk_packet(positions).unwrap().body()
        );
    }
}

pub(super) struct Horizon {
    routes: [Vec<(i8, i8)>; 8],
    frames: HashMap<Key, (Packet, usize)>,
    order: VecDeque<Key>,
    bytes: usize,
    pending: HashMap<Key, Arc<AtomicBool>>,
    tokens: Vec<Arc<AtomicBool>>,
    jobs: Sender<Job>,
    ready: Receiver<(Key, Option<Packet>)>,
}

impl Horizon {
    pub(super) fn new(
        template: Arc<Template>,
        distances: crate::config::WorldDistances,
    ) -> std::io::Result<Self> {
        let (jobs, rx) = bounded::<Job>(TEMPLATE_JOBS);
        let (tx, ready) = bounded(TEMPLATE_JOBS);
        std::thread::Builder::new()
            .name("horizon-template".into())
            .spawn(move || {
                let mut batch = Vec::with_capacity(TEMPLATE_JOBS);
                while let Ok(first) = rx.recv() {
                    batch.push(first);
                    batch.extend(rx.try_iter().take(TEMPLATE_JOBS - 1));
                    batch.par_drain(..).for_each(|(key, pair, token)| {
                        let packet = token
                            .load(Ordering::Acquire)
                            .then(|| {
                                let packet = match pair {
                                    Some(frames) => bulk(&frames)?,
                                    None => template.columns.get(&key.0)?.build_packet(key.0),
                                };
                                packet.encoded(256).ok()?;
                                packet.encoded(512).ok()?;
                                Some(packet)
                            })
                            .flatten();
                        let _ = tx.send((key, packet));
                    });
                }
            })?;
        let r = distances.horizon_view_distance as i8;
        let routes = std::array::from_fn(|sector| {
            let angle = sector as f64 * std::f64::consts::FRAC_PI_4;
            let mut offsets: Vec<_> = (-r..=r)
                .flat_map(|x| (-r..=r).map(move |z| (x, z)))
                .filter(|&(x, z)| {
                    x.unsigned_abs().max(z.unsigned_abs()) > distances.simulation_distance
                })
                .collect();
            offsets.sort_unstable_by_key(|&(x, z)| {
                let (x, z) = (i32::from(x), i32::from(z));
                (
                    -(x * x + z * z),
                    ((f64::from(x) * angle.cos() + f64::from(z) * angle.sin()) * 100.) as i32,
                    x,
                    z,
                )
            });
            offsets
        });
        Ok(Self {
            routes,
            frames: HashMap::with_capacity(CACHE_ENTRIES),
            order: VecDeque::with_capacity(CACHE_ENTRIES),
            bytes: 0,
            pending: HashMap::with_capacity(TEMPLATE_JOBS),
            tokens: (0..TEMPLATE_JOBS)
                .map(|_| Arc::new(AtomicBool::new(false)))
                .collect(),
            jobs,
            ready,
        })
    }

    pub(super) fn route(&self, direction: [f64; 2]) -> &[(i8, i8)] {
        let sector =
            (direction[1].atan2(direction[0]) / std::f64::consts::FRAC_PI_4).round() as i32;
        &self.routes[sector.rem_euclid(8) as usize]
    }

    pub(super) fn request_template(&mut self, pos: ColumnPos) {
        self.request((pos, None), None);
    }

    pub(super) fn pair(&mut self, a: ColumnPos, b: ColumnPos) -> Option<Packet> {
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        let key = (a, Some(b));
        if let Some((packet, _)) = self.frames.get(&key) {
            return Some(packet.clone());
        }
        let frames = [self.get(a)?.clone(), self.get(b)?.clone()];
        self.request(key, Some(frames));
        None
    }

    fn request(&mut self, pos: Key, pair: Option<[Packet; 2]>) {
        if self.pending.contains_key(&pos) {
            return;
        }
        let Some(token) = self.tokens.pop() else {
            return;
        };
        token.store(true, Ordering::Release);
        if self.jobs.try_send((pos, pair, token.clone())).is_ok() {
            self.pending.insert(pos, token);
        } else {
            self.tokens.push(token);
        }
    }

    pub(super) fn insert(&mut self, pos: ColumnPos, packet: Packet) {
        self.cache((pos, None), packet);
    }

    fn cache(&mut self, pos: Key, packet: Packet) {
        if self.frames.contains_key(&pos) {
            return;
        }
        // The worker has prepared both negotiated frames. Account for the
        // body and both profiles even when they share an allocation.
        let size = packet.body().len() * 3 + 512;
        while self.bytes + size > CACHE_BYTES || self.frames.len() == CACHE_ENTRIES {
            let Some(old) = self.order.pop_front() else {
                return;
            };
            if let Some((_, bytes)) = self.frames.remove(&old) {
                self.bytes -= bytes;
            }
        }
        self.frames.insert(pos, (packet, size));
        self.order.push_back(pos);
        self.bytes += size;
    }

    pub(super) fn get(&self, pos: ColumnPos) -> Option<&Packet> {
        self.frames.get(&(pos, None)).map(|(p, _)| p)
    }

    pub(super) fn maintain(&mut self, players: &[Player], radius: u8) {
        let visible = |(a, b): Key| {
            players.iter().any(|p| {
                in_range(a, column(p.pos), radius)
                    && b.is_none_or(|pos| in_range(pos, column(p.pos), radius))
            })
        };
        for (&pos, token) in &self.pending {
            if !visible(pos) {
                token.store(false, Ordering::Release);
            }
        }
        while let Ok((pos, packet)) = self.ready.try_recv() {
            if let Some(token) = self.pending.remove(&pos) {
                if token.load(Ordering::Acquire)
                    && let Some(p) = packet
                {
                    self.cache(pos, p);
                }
                self.tokens.push(token);
            }
        }
        self.order.retain(|pos| {
            if visible(*pos) {
                return true;
            }
            if let Some((_, size)) = self.frames.remove(pos) {
                self.bytes -= size;
            }
            false
        });
    }
}
