use super::Player;
use crate::entities::{Entity,EntityArena,EntityId};
use crate::metrics::Metrics;
use crate::{
    arena::{Column, ColumnPos, Template, column},
    terrain::Generator,
};
use crossbeam_channel::{Receiver, Sender, bounded};
use std::time::Instant;
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const CAPACITY: usize = 32;
const IDLE_COLUMNS: usize = 32;
type GeneratedColumn = (Column, Vec<Entity>);

pub(super) struct Generation {
    thread: std::thread::JoinHandle<()>,
    jobs: Sender<(ColumnPos, Arc<AtomicBool>)>,
    ready: Receiver<(ColumnPos, Option<GeneratedColumn>)>,
    mobs: HashMap<ColumnPos, Vec<EntityId>>,
    tokens: Vec<Arc<AtomicBool>>,
    pending: HashMap<ColumnPos, Arc<AtomicBool>>,
}

impl Generation {
    pub(super) fn new(
        seed: i64,
        legacy: bool,
        revision: u32,
        dimension: Option<Arc<crate::overworld::DimensionTerrain>>,
        mut population: Option<crate::overworld::populated::Population>,
        metrics: Arc<Metrics>,
    ) -> io::Result<Self> {
        let (jobs, rx) = bounded::<(ColumnPos, Arc<AtomicBool>)>(CAPACITY);
        let (tx, ready) = bounded(CAPACITY);
        let thread = std::thread::Builder::new()
            .name("overworld-generator".into())
            .spawn(move || {
                let generator = Generator(seed as u64);
                let overworld =
                    (!legacy && dimension.is_none() && population.is_none()).then(|| {
                        let mut provider = crate::overworld::Overworld::new(seed);
                        provider.structures = revision >= 2;
                        provider
                    });
                let mut batch = Vec::with_capacity(CAPACITY);
                let mut positions = Vec::with_capacity(CAPACITY);
                let mut prepared = HashMap::with_capacity(CAPACITY);
                while let Ok(first) = rx.recv() {
                    batch.push(first);
                    batch.extend(rx.try_iter().take(CAPACITY - 1));
                    if batch.len() > 1
                        && let Some(generator) = &mut population
                    {
                        positions.extend(
                            batch
                                .iter()
                                .filter(|(_, wanted)| wanted.load(Ordering::Acquire))
                                .map(|(pos, _)| *pos),
                        );
                        let start = Instant::now();
                        match generator.columns(&positions) {
                            Ok(cols) => prepared.extend(positions.iter().copied().zip(cols)),
                            Err(e) => eprintln!("Overworld population batch failed: {e}"),
                        }
                        metrics
                            .generation_ns
                            .fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
                        positions.clear();
                    }
                    for (pos, wanted) in batch.drain(..) {
                        let col = wanted
                            .load(Ordering::Acquire)
                            .then(|| {
                                let start = Instant::now();
                                let col = if let Some(col) = prepared.remove(&pos) {
                                    col
                                } else if let Some(d) = &dimension {
                                    d.column(pos.0, pos.1)
                                } else if let Some(population) = &mut population {
                                    match population.column(pos) {
                                        Ok(c) => c,
                                        Err(e) => {
                                            eprintln!(
                                                "Overworld population failed at {pos:?}: {e}"
                                            );
                                            return None;
                                        }
                                    }
                                } else {
                                    match &overworld {
                                        Some(provider) => provider.column(pos.0, pos.1),
                                        None => generator.column(pos.0, pos.1),
                                    }
                                };
                                col.packet(pos);
                                metrics.generation_ns.fetch_add(
                                    start.elapsed().as_nanos() as u64,
                                    Ordering::Relaxed,
                                );
                                metrics.generated_columns.fetch_add(1, Ordering::Relaxed);
                                let mobs = if population.is_some() {
                                    match crate::anvil::population_entities(&col) {
                                        Ok(mobs) => mobs,
                                        Err(error) => {
                                            eprintln!(
                                                "Invalid generated entities at {pos:?}: {error}"
                                            );
                                            return None;
                                        }
                                    }
                                } else {
                                    Vec::new()
                                };
                                Some((col, mobs))
                            })
                            .flatten();
                        if tx.send((pos, col)).is_err() {
                            return;
                        }
                    }
                    prepared.clear();
                }
            })?;
        Ok(Self {
            thread,
            jobs,
            ready,
            mobs: HashMap::new(),
            pending: HashMap::with_capacity(CAPACITY),
            tokens: (0..CAPACITY)
                .map(|_| Arc::new(AtomicBool::new(false)))
                .collect(),
        })
    }

    pub(super) fn finish(self) {
        for token in self.pending.values() {
            token.store(false, Ordering::Release);
        }
        drop(self.jobs);
        drop(self.ready);
        if self.thread.join().is_err() {
            eprintln!("Generation worker panicked on shutdown");
        }
    }

    pub(super) fn request(&mut self, pos: ColumnPos) {
        if self.pending.len() >= CAPACITY || self.pending.contains_key(&pos) {
            return;
        }
        let Some(wanted) = self.tokens.pop() else {
            return;
        };
        wanted.store(true, Ordering::Release);
        if self.jobs.try_send((pos, wanted.clone())).is_ok() {
            self.pending.insert(pos, wanted);
        } else {
            self.tokens.push(wanted);
        }
    }

    pub(super) fn collect(&mut self, cache: &mut Template, entities: &mut EntityArena) {
        for (pos, col) in self.ready.try_iter() {
            if let Some(token) = self.pending.remove(&pos) {
                if token.load(Ordering::Acquire)
                    && let Some((col, mobs)) = col
                {
                    if !mobs.is_empty() && !cache.columns.contains_key(&pos) {
                        self.mobs.insert(
                            pos,
                            mobs.into_iter()
                                .filter_map(|e| entities.insert(e).ok())
                                .collect(),
                        );
                    }
                    cache.columns.insert(pos, col);
                }
                self.tokens.push(token);
            }
        }
    }

    pub(super) fn trim(
        &mut self,
        players: &[Player],
        radius: i32,
        cache: &mut Template,
        entities: &mut EntityArena,
    ) {
        let visible = |&(x, z): &ColumnPos| {
            players.iter().any(|p| {
                let (cx, cz) = column(p.pos);
                x.abs_diff(cx) <= radius as u32 && z.abs_diff(cz) <= radius as u32
            })
        };
        for (pos, token) in &self.pending {
            if !visible(pos) {
                token.store(false, Ordering::Release);
            }
        }
        let limit = players.len() * (2 * radius as usize + 1).pow(2) + IDLE_COLUMNS;
        if cache.columns.len() > limit {
            cache.columns.retain(|pos, _| visible(pos));
            self.mobs.retain(|pos, ids| {
                if cache.contains(*pos) {
                    return true;
                }
                for &id in ids.iter() {
                    entities.remove(id);
                }
                false
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returning_to_same_coordinates_cannot_publish_other_dimensions_work() {
        use crate::overworld::Dimension;
        let pos = (9, -7);
        let mut hell = Generation::new(
            12345,
            false,
            2,
            Some(Arc::new(crate::overworld::DimensionTerrain::new(
                12345,
                Dimension::Nether,
            ))),
            None,
            Arc::default(),
        )
        .unwrap();
        let mut end = Generation::new(
            12345,
            false,
            2,
            Some(Arc::new(crate::overworld::DimensionTerrain::new(
                12345,
                Dimension::End,
            ))),
            None,
            Arc::default(),
        )
        .unwrap();
        let mut entities = EntityArena::with_capacity(32);
        let mut first = Template::flat(0);
        let mut second = Template::flat(0);
        hell.request(pos);
        hell.request(pos);
        end.request(pos);
        assert_eq!(hell.pending.len(), 1);
        hell.trim(&[], 1, &mut first, &mut entities);
        let until = Instant::now() + std::time::Duration::from_secs(10);
        while !hell.pending.is_empty() || !end.pending.is_empty() {
            assert!(Instant::now() < until);
            hell.collect(&mut first, &mut entities);
            end.collect(&mut second, &mut entities);
            std::thread::yield_now();
        }
        assert!(!first.contains(pos));
        assert_eq!(second.columns[&pos].biomes, [9; 256]);
        hell.request(pos);
        while !first.contains(pos) {
            assert!(Instant::now() < until);
            hell.collect(&mut first, &mut entities);
            std::thread::yield_now();
        }
        assert_eq!(first.columns[&pos].biomes, [8; 256]);
        let before_eviction = first.columns[&pos].packet(pos);
        first.columns.extend(Template::flat(3).columns);
        hell.trim(&[], 1, &mut first, &mut entities);
        assert!(first.columns.is_empty());
        hell.request(pos);
        let until = Instant::now() + std::time::Duration::from_secs(10);
        while !first.contains(pos) {
            assert!(Instant::now() < until);
            hell.collect(&mut first, &mut entities);
            std::thread::yield_now();
        }
        assert_eq!(
            first.columns[&pos].packet(pos).body(),
            before_eviction.body()
        );
    }

    #[test]
    fn abandoned_work_is_bounded_discarded_and_retryable() {
        let mut queue = Generation::new(9581, false, 2, None, None, Arc::default()).unwrap();
        let mut entities = EntityArena::with_capacity(32);
        let mut cache = Template::flat(3);
        for x in 0..64 {
            queue.request((x, 9));
        }
        assert_eq!(queue.pending.len(), CAPACITY);
        queue.trim(&[], 1, &mut cache, &mut entities);
        assert!(cache.columns.is_empty());
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        while !queue.pending.is_empty() {
            assert!(Instant::now() < deadline);
            queue.collect(&mut cache, &mut entities);
            std::thread::yield_now();
        }
        assert!(cache.columns.is_empty());
        assert_eq!(queue.tokens.len(), CAPACITY);
        queue.request((0, 9));
        while !cache.contains((0, 9)) {
            assert!(Instant::now() < deadline);
            queue.collect(&mut cache, &mut entities);
            std::thread::yield_now();
        }
        assert!(cache.cached((0, 9)));
    }
}
