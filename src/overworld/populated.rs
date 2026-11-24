use super::{
    JavaRandom, Overworld,
    column::pack,
    decoration::{
        DecorationWorld,
        biomes::{BiomeWorld, DecoratorState},
        plants::PlantWorld,
    },
    dungeons::DungeonWorld,
    lakes::LakeWorld,
};
use crate::arena::{Column, ColumnPos};
use crossbeam_channel::{Receiver, Sender};
use fastnbt::Value;
use flate2::read::GzDecoder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, BufWriter, Write};
use std::sync::Arc;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::File,
    io,
    path::{Path, PathBuf},
};

type Pos = [i32; 3];
const RAW_CACHE_CAPACITY: usize = 2048;
fn column([x, _, z]: Pos) -> ColumnPos {
    (x >> 4, z >> 4)
}
fn index([x, y, z]: Pos) -> usize {
    ((y << 8) | ((z & 15) << 4) | (x & 15)) as usize
}

struct Raw {
    states: Vec<u16>,
    biomes: [u8; 256],
    tiles: BTreeMap<usize, Value>,
    ticks: BTreeMap<usize, (u16, i32)>,
    last_used: u64,
    heightmap: [u8; 256],
    solid_top: [u8; 256],
    // One cutoff per light level. Zero denotes an unobstructed level.
    sky: Box<[[u16; 15]; 256]>,
}

fn opacity(id: u16) -> u8 {
    match id {
        0 | 6 | 31 | 32 | 37..=40 | 50 | 78 | 83 | 106 | 111 | 175 => 0,
        18 | 161 => 1,
        8 | 9 | 79 => 3,
        _ => 15,
    }
}
fn material_solid(id: u16) -> bool {
    !matches!(id,0|6|8..=11|31|32|37..=40|50|51|59|78|83|104..=106|111|127|141|142|175)
}
fn surface(id: u16) -> bool {
    material_solid(id) && !matches!(id, 18 | 161)
}
impl Raw {
    fn refresh(&mut self, ray: usize, from: usize) {
        let mut cutoffs = [0; 15];
        let mut lost = 0;
        let mut height = None;
        let mut solid = None;
        for y in (0..=from).rev() {
            let id = self.states[y * 256 + ray] >> 4;
            let attenuation = opacity(id);
            if attenuation != 0 {
                height.get_or_insert(y as u8);
                let next = (lost + usize::from(attenuation)).min(15);
                cutoffs[lost..next].fill(y as u16 + 1);
                lost = next;
            }
            if surface(id) {
                solid.get_or_insert(y as u8);
            }
            if lost == 15 && solid.is_some() {
                break;
            }
        }
        self.sky[ray] = cutoffs;
        self.heightmap[ray] = height.unwrap_or(0);
        self.solid_top[ray] = solid.unwrap_or(0);
    }
    fn write(&mut self, i: usize, state: u16) {
        let old = self.states[i] >> 4;
        self.states[i] = state;
        let new = state >> 4;
        let y = i >> 8;
        let ray = i & 255;
        if (opacity(old) != opacity(new)
            && (y + 1 >= usize::from(self.sky[ray][14]) || y >= usize::from(self.heightmap[ray])))
            || (surface(old) != surface(new) && y >= usize::from(self.solid_top[ray]))
        {
            self.refresh(
                ray,
                y.max(usize::from(self.heightmap[ray]))
                    .max(usize::from(self.solid_top[ray])),
            );
        }
    }
}
#[derive(Default, Serialize, Deserialize)]
struct Changes {
    blocks: Vec<(u16, u16)>,
    tiles: Vec<(u16, Vec<u8>)>,
    ticks: Vec<(u16, u16, i32)>,
}
impl Changes {
    fn apply(&self, col: &mut Raw) -> io::Result<()> {
        for &(offset, state) in &self.blocks {
            let i = usize::from(offset);
            // Dirty offsets store the final snapshot. Decoration may replace
            // a block and restore its ID while removing its tile or tick.
            col.tiles.remove(&i);
            col.ticks.remove(&i);
            col.write(i, state);
        }
        for (i, nbt) in &self.tiles {
            col.tiles.insert(
                usize::from(*i),
                fastnbt::from_bytes(nbt).map_err(io::Error::other)?,
            );
        }
        for &(i, fluid, delay) in &self.ticks {
            col.ticks.insert(i.into(), (fluid, delay));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Record {
    owner: ColumnPos,
    generators: DecoratorState,
    columns: Vec<(ColumnPos, Changes)>,
}
struct Neighbors<'a> {
    origin: ColumnPos,
    columns: [Option<&'a mut Raw>; 16],
    dirty: [Vec<u16>; 16],
    outside: Cell<bool>,
}
impl<'a> Neighbors<'a> {
    fn new(raw: &'a mut HashMap<ColumnPos, Raw>, owner: ColumnPos) -> Self {
        let origin = (owner.0 - 1, owner.1 - 1);
        let keys: [ColumnPos; 16] =
            std::array::from_fn(|n| (origin.0 + (n / 4) as i32, origin.1 + (n % 4) as i32));
        let columns = raw.get_disjoint_mut(keys.each_ref());
        Self {
            origin,
            columns,
            dirty: Default::default(),
            outside: Cell::new(false),
        }
    }
    fn slot(&self, (x, z): ColumnPos) -> Option<usize> {
        let (dx, dz) = (
            x.wrapping_sub(self.origin.0) as usize,
            z.wrapping_sub(self.origin.1) as usize,
        );
        if dx < 4 && dz < 4 {
            Some(dx * 4 + dz)
        } else {
            None
        }
    }
    fn get(&self, pos: ColumnPos) -> Option<&Raw> {
        self.columns[self.slot(pos)?].as_deref()
    }
    fn get_mut(&mut self, pos: ColumnPos) -> Option<&mut Raw> {
        let slot = self.slot(pos)?;
        self.columns[slot].as_deref_mut()
    }
}
impl LakeWorld for Neighbors<'_> {
    fn state(&self, p: Pos) -> u16 {
        if !(0..256).contains(&p[1]) {
            return 0;
        }
        match self.get(column(p)) {
            Some(c) => c.states[index(p)],
            None => {
                self.outside.set(true);
                0
            }
        }
    }
    fn set_state(&mut self, p: Pos, s: u16) {
        if !(0..256).contains(&p[1]) {
            return;
        }
        let pos = column(p);
        let Some(slot) = self.slot(pos) else {
            self.outside.set(true);
            return;
        };
        let Some(c) = self.columns[slot].as_deref_mut() else {
            self.outside.set(true);
            return;
        };
        let i = index(p);
        if c.states[i] != s {
            if c.states[i] >> 4 != s >> 4 {
                c.tiles.remove(&i);
                c.ticks.remove(&i);
            }
            self.dirty[slot].push(i as u16);
            c.write(i, s);
        }
    }
    fn solid(&self, p: Pos) -> bool {
        material_solid(self.state(p) >> 4)
    }
    fn liquid(&self, p: Pos) -> bool {
        matches!(self.state(p) >> 4, 8..=11)
    }
    fn skylight(&self, [x, y, z]: Pos) -> u8 {
        let p = [x, 0, z];
        if let Some(c) = self.get(column(p)) {
            let y = y.max(0);
            c.sky[index(p)]
                .iter()
                .filter(|&&h| i32::from(h) <= y)
                .count() as u8
        } else {
            self.outside.set(true);
            15
        }
    }
    fn mycelium_surface(&self, p: Pos) -> bool {
        self.get(column(p))
            .is_some_and(|c| matches!(c.biomes[index([p[0], 0, p[2]])], 14 | 15))
    }
    fn can_freeze(&self, p: Pos) -> bool {
        matches!(self.state(p), 128 | 144)
            && self.get(column(p)).is_some_and(|c| {
                matches!(
                    c.biomes[index([p[0], 0, p[2]])],
                    10..=13 | 26 | 30 | 31 | 140 | 158
                )
            })
    }
}
impl DungeonWorld for Neighbors<'_> {
    fn loaded_biome(&self, x: i32, z: i32) -> Option<u8> {
        self.get((x >> 4, z >> 4))
            .map(|c| c.biomes[index([x, 0, z])])
    }
    fn full_block(&self, p: Pos) -> bool {
        PlantWorld::full_block(self, p)
    }
    fn set_tile(&mut self, p: Pos, nbt: Value) {
        if let Some(c) = self.get_mut(column(p)) {
            c.tiles.insert(index(p), nbt);
        }
    }
}
impl PlantWorld for Neighbors<'_> {
    fn combined_light(&self, p: Pos) -> u8 {
        self.skylight(p)
    }
    fn full_block(&self, p: Pos) -> bool {
        self.solid(p)
            && !matches!(
                self.state(p) >> 4,
                44 | 52 | 53 | 54 | 60 | 81 | 85 | 96 | 101 | 102 | 109 | 128 | 134
                    ..=136 | 139 | 156 | 163 | 164 | 171 | 182
            )
    }
}
impl DecorationWorld for Neighbors<'_> {
    fn replace_stone(&mut self, p: Pos, ore: u16) {
        if !(0..256).contains(&p[1]) {
            return;
        }
        let Some(slot) = self.slot(column(p)) else {
            self.outside.set(true);
            return;
        };
        let Some(c) = self.columns[slot].as_deref_mut() else {
            self.outside.set(true);
            return;
        };
        let i = index(p);
        if c.states[i] >> 4 == 1 && c.states[i] != ore {
            // Stone, its variants, and all DEFAULT ore replacements are opaque
            // solid blocks with neither tile entities nor scheduled fluid ticks.
            c.states[i] = ore;
            self.dirty[slot].push(i as u16);
        }
    }

    fn top_solid_or_liquid(&self, x: i32, z: i32) -> i32 {
        let p = [x, 0, z];
        if let Some(c) = self.get(column(p)) {
            let ray = index(p);
            let top = c.solid_top[ray];
            i32::from(top) + i32::from(top != 0 || surface(c.states[ray] >> 4))
        } else {
            self.outside.set(true);
            0
        }
    }
}
impl BiomeWorld for Neighbors<'_> {
    fn height(&self, x: i32, z: i32) -> i32 {
        let p = [x, 0, z];
        if let Some(c) = self.get(column(p)) {
            let ray = index(p);
            let top = c.heightmap[ray];
            i32::from(top) + i32::from(top != 0 || opacity(c.states[ray] >> 4) != 0)
        } else {
            self.outside.set(true);
            0
        }
    }
    fn tick_spring(&mut self, p: Pos, fluid: u16, _: &mut JavaRandom) {
        let delay = if fluid == 8 { 5 } else { 30 };
        let [x, y, z] = p;
        self.set_state(p, (fluid + 1) << 4);
        for dest in [[x - 1, y, z], [x + 1, y, z], [x, y, z - 1], [x, y, z + 1]] {
            if self.state(dest) == 0 {
                self.set_state(dest, (fluid << 4) | if fluid == 8 { 1 } else { 2 });
                if let Some(c) = self.get_mut(column(dest)) {
                    c.ticks.insert(index(dest), (fluid, delay));
                }
            }
        }
    }
}

/// Flush on the existing Anvil save worker. Failed snapshots keep all records
/// queued for the next save. Each published batch is an atomic sequence.
pub(crate) struct JournalWriter {
    dir: PathBuf,
    _lease: File,
    rx: Receiver<Arc<Record>>,
    pending: Vec<Arc<Record>>,
    saved: usize,
}
impl JournalWriter {
    pub(crate) fn flush(&mut self) -> io::Result<()> {
        self.pending.extend(self.rx.try_iter().take(self.rx.len()));
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut tmp = tempfile::NamedTempFile::new_in(&self.dir)?;
        {
            let mut out = BufWriter::new(tmp.as_file_mut());
            out.write_all(b"POP1")?;
            bincode::serialize_into(&mut out, &(self.pending.len() as u64))
                .map_err(io::Error::other)?;
            for rec in &self.pending {
                bincode::serialize_into(&mut out, rec.as_ref()).map_err(io::Error::other)?;
            }
            out.flush()?;
        }
        tmp.as_file().sync_all()?;
        tmp.persist_noclobber(self.dir.join(format!("{:016x}.bin", self.saved)))
            .map_err(|e| e.error)?;
        self.saved += self.pending.len();
        self.pending.clear();
        Ok(())
    }
}
/// Each committed owner records its writes to every neighbor together with
/// the biome generators' continuation state. A cached column can be evicted
/// and reconstructed from its primer and the ordered records that touched it.
pub(crate) struct Population {
    provider: Arc<Overworld>,
    journal_tx: Option<Sender<Arc<Record>>>,
    pub(crate) journal: Option<JournalWriter>,
    records: Vec<Arc<Record>>,
    raw: HashMap<ColumnPos, Raw>,
    owners: HashSet<ColumnPos>,
    history: HashMap<ColumnPos, Vec<usize>>,
    generators: DecoratorState,
    recency: BTreeSet<(u64, ColumnPos)>,
    clock: u64,
}
impl Population {
    pub(crate) fn open(seed: i64, path: Option<&Path>) -> io::Result<Self> {
        let (journal_tx, journal) = if let Some(dir) = path {
            std::fs::create_dir_all(dir)?;
            let lease = File::options()
                .create(true)
                .write(true)
                .truncate(false)
                .open(dir.join("owner.lock"))?;
            lease.try_lock().map_err(io::Error::other)?;
            let (tx, rx) = crossbeam_channel::unbounded();
            (
                Some(tx),
                Some(JournalWriter {
                    dir: dir.to_owned(),
                    _lease: lease,
                    rx,
                    pending: Vec::new(),
                    saved: 0,
                }),
            )
        } else {
            (None, None)
        };
        let mut this = Self {
            provider: Arc::new(Overworld::new(seed)),
            journal_tx,
            journal,
            records: Vec::new(),
            raw: HashMap::new(),
            owners: HashSet::new(),
            history: HashMap::new(),
            generators: Default::default(),
            recency: Default::default(),
            clock: 0,
        };
        if let Some(dir) = path {
            let mut files = Vec::new();
            for entry in std::fs::read_dir(dir)? {
                let p = entry?.path();
                if p.extension().is_some_and(|e| e == "gz" || e == "bin") {
                    files.push(p);
                }
            }
            files.sort();
            for file in files {
                let seq = this.records.len();
                let mut reader = BufReader::new(File::open(&file)?);
                if file == dir.join(format!("{seq:016x}.gz")) {
                    this.remember(serde_json::from_reader(GzDecoder::new(reader))?)?;
                } else if file == dir.join(format!("{seq:016x}.bin")) {
                    use std::io::Read;
                    let mut magic = [0; 4];
                    reader.read_exact(&mut magic)?;
                    if &magic != b"POP1" {
                        return Err(io::Error::other("invalid population journal header"));
                    }
                    let count: u64 =
                        bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                    if count == 0 {
                        return Err(io::Error::other("empty population journal batch"));
                    }
                    for _ in 0..count {
                        this.remember(
                            bincode::deserialize_from(&mut reader).map_err(io::Error::other)?,
                        )?;
                    }
                    if reader.read(&mut [0; 1])? != 0 {
                        return Err(io::Error::other("trailing population journal bytes"));
                    }
                } else {
                    return Err(io::Error::other("incomplete population journal"));
                }
            }
            if let Some(j) = &mut this.journal {
                j.saved = this.records.len();
            }
        }
        Ok(this)
    }
    fn remember(&mut self, rec: Record) -> io::Result<()> {
        if !self.owners.insert(rec.owner) {
            return Err(io::Error::other("duplicate population journal owner"));
        }
        self.generators = rec.generators.clone();
        for (pos, _) in &rec.columns {
            self.history
                .entry(*pos)
                .or_default()
                .push(self.records.len());
        }
        self.records.push(Arc::new(rec));
        Ok(())
    }
    fn primer_builder(
        provider: Arc<Overworld>,
        missing: &[ColumnPos],
    ) -> impl Fn(ColumnPos) -> Raw + Send + Sync + 'static {
        // GenLayer produces identical overlapping samples. Amortize its
        // recursive parent layers when a batch covers a compact rectangle.
        let region = missing.first().and_then(|&(x, z)| {
            let (x0, z0, x1, z1) = missing.iter().fold((x, z, x, z), |(a, b, c, d), &(x, z)| {
                (a.min(x), b.min(z), c.max(x), d.max(z))
            });
            let w = (i64::from(x1) - i64::from(x0) + 1) as usize;
            let h = (i64::from(z1) - i64::from(z0) + 1) as usize;
            (missing.len() > 1 && w.saturating_mul(h) <= missing.len() * 3).then(|| {
                let surface = provider.biomes.blocks(x0 * 16, z0 * 16, w * 16, h * 16);
                let quarter = provider
                    .biomes
                    .quarter(x0 * 4 - 2, z0 * 4 - 2, w * 4 + 6, h * 4 + 6);
                (x0, z0, w, surface, quarter)
            })
        });
        move |pos: ColumnPos| {
            let (states, biomes) = if let Some((x0, z0, w, surface, quarter)) = &region {
                let dx = (pos.0 - x0) as usize;
                let dz = (pos.1 - z0) as usize;
                let ids: [i32; 256] = std::array::from_fn(|i| {
                    surface[(dz * 16 + i / 16) * w * 16 + dx * 16 + i % 16]
                });
                let density: [i32; 100] = std::array::from_fn(|i| {
                    quarter[(dz * 4 + i / 10) * (w * 4 + 6) + dx * 4 + i % 10]
                });
                provider.primer_with_biomes(pos.0, pos.1, &ids, &density)
            } else {
                provider.primer(pos.0, pos.1)
            };
            let mut col = Raw {
                states,
                biomes,
                tiles: Default::default(),
                ticks: Default::default(),
                last_used: 0,
                heightmap: [0; 256],
                solid_top: [0; 256],
                sky: Box::new([[0; 15]; 256]),
            };
            let highest = col
                .states
                .as_chunks::<256>()
                .0
                .iter()
                .rposition(|layer| layer.iter().any(|&s| s != 0))
                .unwrap_or(0);
            for ray in 0..256 {
                col.refresh(ray, highest);
            }
            col
        }
    }
    fn insert_raw(&mut self, pos: ColumnPos, mut col: Raw) -> io::Result<()> {
        if let Some(ids) = self.history.get(&pos) {
            for &seq in ids {
                for (target, changes) in &self.records[seq].columns {
                    if *target == pos {
                        changes.apply(&mut col)?;
                        break;
                    }
                }
            }
        }
        self.raw.insert(pos, col);
        Ok(())
    }
    #[cfg(test)]
    fn prepare(&mut self, positions: &[ColumnPos]) -> io::Result<()> {
        let missing: Vec<_> = positions
            .iter()
            .copied()
            .filter(|p| !self.raw.contains_key(p))
            .collect();
        let build = Self::primer_builder(self.provider.clone(), &missing);
        for p in missing {
            self.insert_raw(p, build(p))?;
        }
        self.touch(positions);
        Ok(())
    }
    fn touch(&mut self, positions: &[ColumnPos]) {
        for &p in positions {
            self.clock += 1;
            let c = self.raw.get_mut(&p).unwrap();
            self.recency.remove(&(c.last_used, p));
            c.last_used = self.clock;
            self.recency.insert((self.clock, p));
        }
        while self.raw.len() > RAW_CACHE_CAPACITY {
            let (_, oldest) = self.recency.pop_first().unwrap();
            self.raw.remove(&oldest);
        }
    }
    #[cfg(test)]
    fn ensure(&mut self, pos: ColumnPos) -> io::Result<()> {
        self.prepare(&[pos])
    }
    #[cfg(test)]
    fn populate_owner(&mut self, pos: ColumnPos) -> io::Result<()> {
        if self.owners.contains(&pos) {
            return Ok(());
        }
        let positions: Vec<_> = (pos.0 - 1..=pos.0 + 2)
            .flat_map(|x| (pos.1 - 1..=pos.1 + 2).map(move |z| (x, z)))
            .collect();
        self.prepare(&positions)?;
        self.decorate_owner(pos)
    }
    fn decorate_owner(&mut self, pos: ColumnPos) -> io::Result<()> {
        let mut world = Neighbors::new(&mut self.raw, pos);
        let mut generators = self.generators.clone();
        self.provider
            .populate(&mut world, [pos.0, pos.1], &mut generators, |_, _| false);
        let result = (|| {
            if world.outside.get() {
                return Err(io::Error::other("population exceeded loaded neighbors"));
            }
            let mut columns = Vec::new();
            for (slot, mut blocks) in world.dirty.into_iter().enumerate() {
                if blocks.is_empty() {
                    continue;
                }
                let pos = (
                    world.origin.0 + (slot / 4) as i32,
                    world.origin.1 + (slot % 4) as i32,
                );
                let c = world.columns[slot].as_deref().unwrap();
                blocks.sort_unstable();
                blocks.dedup();
                let mut changes = Changes::default();
                for i in blocks {
                    changes.blocks.push((i, c.states[usize::from(i)]));
                    if let Some(tile) = c.tiles.get(&usize::from(i)) {
                        changes
                            .tiles
                            .push((i, fastnbt::to_bytes(tile).map_err(io::Error::other)?));
                    }
                    if let Some(&(fluid, delay)) = c.ticks.get(&usize::from(i)) {
                        changes.ticks.push((i, fluid, delay));
                    }
                }
                columns.push((pos, changes));
            }
            let rec = Record {
                owner: pos,
                generators,
                columns,
            };
            Ok(rec)
        })();
        if result.is_err() {
            self.raw.clear();
            self.recency.clear();
        }
        self.remember(result?)?;
        if let Some(tx) = &self.journal_tx {
            tx.send(self.records.last().unwrap().clone())
                .map_err(io::Error::other)?;
        }
        Ok(())
    }
    pub(crate) fn columns(&mut self, positions: &[ColumnPos]) -> io::Result<Vec<Column>> {
        let mut owners = Vec::new();
        let mut seen = HashSet::new();
        let mut required = Vec::new();
        let mut wanted = HashSet::new();
        for &(x, z) in positions {
            for cx in x - 2..=x + 1 {
                for cz in z - 2..=z + 1 {
                    if self.owners.contains(&(cx, cz)) || !seen.insert((cx, cz)) {
                        continue;
                    }
                    owners.push((cx, cz));
                    for rx in cx - 1..=cx + 2 {
                        for rz in cz - 1..=cz + 2 {
                            if wanted.insert((rx, rz)) {
                                required.push((rx, rz));
                            }
                        }
                    }
                }
            }
            if wanted.insert((x, z)) {
                required.push((x, z));
            }
        }
        if required.len() > RAW_CACHE_CAPACITY {
            let (first, rest) = positions.split_at(positions.len() / 2);
            let mut out = self.columns(first)?;
            out.extend(self.columns(rest)?);
            return Ok(out);
        }
        let missing: Vec<_> = required
            .iter()
            .copied()
            .filter(|p| !self.raw.contains_key(p))
            .collect();
        let (tx, rx) = crossbeam_channel::unbounded();
        let build = Self::primer_builder(self.provider.clone(), &missing);
        if missing.len() == 1 {
            self.insert_raw(missing[0], build(missing[0]))?;
        } else if rayon::current_thread_index().is_some() {
            // A caller inside a saturated Rayon pool must participate in the
            // fork/join work instead of blocking a worker on the receiver.
            let generated: Vec<_> = missing.into_par_iter().map(|p| (p, build(p))).collect();
            for (p, raw) in generated {
                self.insert_raw(p, raw)?;
            }
        } else if !missing.is_empty() {
            rayon::spawn(move || {
                missing.into_iter().par_bridge().for_each_with(tx, |tx, p| {
                    let _ = tx.send((p, build(p)));
                });
            });
        }
        for (cx, cz) in owners {
            for x in cx - 1..=cx + 2 {
                for z in cz - 1..=cz + 2 {
                    while !self.raw.contains_key(&(x, z)) {
                        let (p, raw) = rx.recv().map_err(io::Error::other)?;
                        self.insert_raw(p, raw)?;
                    }
                }
            }
            self.decorate_owner((cx, cz))?;
        }
        for p in &required {
            while !self.raw.contains_key(p) {
                let (p, raw) = rx.recv().map_err(io::Error::other)?;
                self.insert_raw(p, raw)?;
            }
        }
        self.touch(&required);
        positions.par_iter().map(|&p| self.pack_column(p)).collect()
    }
    pub(crate) fn column(&mut self, pos: ColumnPos) -> io::Result<Column> {
        Ok(self.columns(&[pos])?.pop().unwrap())
    }
    fn pack_column(&self, pos: ColumnPos) -> io::Result<Column> {
        let raw = &self.raw[&pos];
        let mut col = pack(&raw.states, raw.biomes);
        let ticks = raw
            .ticks
            .iter()
            .map(|(&i, &(id, delay))| {
                Value::Compound(
                    [
                        (
                            "i".into(),
                            Value::String(
                                if id == 8 {
                                    "minecraft:flowing_water"
                                } else {
                                    "minecraft:flowing_lava"
                                }
                                .into(),
                            ),
                        ),
                        ("t".into(), Value::Int(delay)),
                        ("p".into(), Value::Int(0)),
                        ("x".into(), Value::Int(pos.0 * 16 + (i & 15) as i32)),
                        ("y".into(), Value::Int((i >> 8) as i32)),
                        ("z".into(), Value::Int(pos.1 * 16 + ((i >> 4) & 15) as i32)),
                    ]
                    .into(),
                )
            })
            .collect();
        crate::anvil::store_overworld_tiles(
            pos,
            &mut col,
            raw.tiles.values().cloned().collect(),
            ticks,
        )?;
        Ok(col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "release performance measurement"]
    fn cold_population_timing() -> io::Result<()> {
        let mut genr = Population::open(4862413858852300341, None)?;
        let start = std::time::Instant::now();
        let batch: Vec<_> = (-6..6).flat_map(|x| (-6..6).map(move |z| (x, z))).collect();
        let cols = genr.columns(&batch)?;
        println!(
            "144 cold columns: {:.3} ms/column",
            start.elapsed().as_secs_f64() * 1000. / batch.len() as f64
        );
        // Outside the timer, compare every packed block state and light plane
        // with the previous implementation on exactly the measured footprint.
        use sha2::{Digest, Sha256};
        let mut digest = Sha256::new();
        for col in cols {
            digest.update(col.biomes);
            for (sy, section) in col.sections {
                digest.update([sy]);
                for state in section.palette {
                    digest.update(state.to_le_bytes());
                }
                digest.update(section.indices);
                if let Some(light) = section.light {
                    for plane in *light {
                        digest.update(plane);
                    }
                }
            }
        }
        assert_eq!(
            format!("{:x}", digest.finalize()),
            "9b71c95a263204c4af5bb815a8aa847856d6460bcd37ba8a4f06b01ba7db23af"
        );
        Ok(())
    }

    #[test]
    #[ignore = "release performance measurement"]
    fn exploration_population_timing() -> io::Result<()> {
        use std::time::Instant;
        let mut population = Population::open(4862413858852300341, None)?;
        let start = Instant::now();
        std::hint::black_box(population.column((0, 0))?);
        let first = start.elapsed();
        let mut samples = Vec::new();
        for x in 1..=64 {
            let start = Instant::now();
            std::hint::black_box(population.column((x, 0))?);
            samples.push(start.elapsed());
        }
        samples.sort_unstable();
        println!(
            "single-column exploration, ms: cold {:.3}, adjacent p50 {:.3}, p95 {:.3}, p99 {:.3}",
            first.as_secs_f64() * 1000.,
            samples[31].as_secs_f64() * 1000.,
            samples[60].as_secs_f64() * 1000.,
            samples[63].as_secs_f64() * 1000.
        );
        Ok(())
    }

    #[test]
    fn batched_primers_match_individual_negative_columns() -> io::Result<()> {
        let mut population = Population::open(-9581, None)?;
        let positions = [(-2, -1), (-1, -1), (0, -1), (0, 0)];
        population.prepare(&positions)?;
        for pos in positions {
            let (states, biomes) = population.provider.primer(pos.0, pos.1);
            assert_eq!(population.raw[&pos].biomes, biomes);
            assert!(population.raw[&pos].states == states, "{pos:?}");
        }
        Ok(())
    }

    #[test]
    fn cached_queries_follow_updates_and_column_limits() {
        let mut raw = HashMap::from([(
            (-1, -1),
            Raw {
                states: vec![0; 65536],
                biomes: [1; 256],
                tiles: Default::default(),
                ticks: Default::default(),
                last_used: 0,
                heightmap: [0; 256],
                solid_top: [0; 256],
                sky: Box::new([[0; 15]; 256]),
            },
        )]);
        let mut w = Neighbors::new(&mut raw, (-1, -1));
        for (y, id) in [
            (0, 1),
            (255, 18),
            (254, 9),
            (248, 1),
            (254, 0),
            (255, 0),
            (248, 0),
            (0, 0),
        ] {
            w.set_state([-1, y, -1], id << 4);
            let ray = |y| w.state([-1, y, -1]) >> 4;
            let top =
                |f: fn(u16) -> bool| (0..256).rev().find(|&cy| f(ray(cy))).map_or(0, |cy| cy + 1);
            assert_eq!(w.height(-1, -1), top(|id| opacity(id) != 0));
            assert_eq!(w.top_solid_or_liquid(-1, -1), top(surface));
            let mut light = 15u8;
            for cy in (0..256).rev() {
                light = light.saturating_sub(opacity(ray(cy)));
                assert_eq!(w.skylight([-1, cy, -1]), light);
            }
        }
    }

    #[test]
    fn replay_discards_stale_tile_and_tick_when_final_id_matches() -> io::Result<()> {
        let p = [0, 50, 0];
        let i = index(p);
        let mut population = Population::open(12345, None)?;
        population.ensure((0, 0))?;
        let col = population.raw.get_mut(&(0, 0)).unwrap();
        for id in [54, 8] {
            col.write(i, id << 4);
            if id == 54 {
                col.tiles.insert(i, Value::Compound(Default::default()));
            } else {
                col.ticks.insert(i, (id, 5));
            }
            Changes {
                blocks: vec![(i as u16, id << 4)],
                ..Default::default()
            }
            .apply(col)?;
            assert!(!col.tiles.contains_key(&i));
            assert!(!col.ticks.contains_key(&i));
        }
        Ok(())
    }

    #[test]
    fn loaded_neighbors_and_dungeon_tiles_survive_restart() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut generator = Population::open(12345, Some(dir.path()))?;
        Arc::get_mut(&mut generator.provider).unwrap().structures = false;
        generator.populate_owner((-2, -3))?;
        // Start from the JSON+gzip layout of existing revision-3 worlds.
        let path = dir.path().join("0000000000000000.gz");
        let mut gz =
            flate2::write::GzEncoder::new(File::create(path)?, flate2::Compression::fast());
        serde_json::to_writer(&mut gz, generator.records[0].as_ref())?;
        gz.finish()?;
        drop(generator);
        let mut generator = Population::open(12345, Some(dir.path()))?;
        Arc::get_mut(&mut generator.provider).unwrap().structures = false;
        let mut tiles = Vec::new();
        for cx in -2..=-1 {
            for cz in -3..=-2 {
                generator.ensure((cx, cz))?;
                let c = &generator.raw[&(cx, cz)];
                tiles.extend(c.tiles.values().cloned());
            }
        }
        assert!(tiles.iter().any(
            |v| matches!(v,Value::Compound(t) if t.get("id")==Some(&Value::String("Chest".into())))
        ));
        let positions = [(-2, -3), (-2, -2), (-1, -3), (-1, -2)];
        let first = generator.columns(&positions)?;
        assert!(first.iter().any(|col| !col.nbt.is_empty()));
        let count = generator.owners.len();
        let batch = dir.path().join("0000000000000001.bin");
        assert!(!batch.exists());
        generator.journal.as_mut().unwrap().flush()?;
        assert!(batch.is_file());
        drop(generator);
        let mut reopened = Population::open(12345, Some(dir.path()))?;
        Arc::get_mut(&mut reopened.provider).unwrap().structures = false;
        let after = reopened.columns(&positions)?;
        assert_eq!(count, reopened.owners.len());
        for ((before, after), pos) in first.iter().zip(&after).zip(positions) {
            // The packet covers the section mask, block IDs and metadata,
            // both light planes and biomes. NBT compounds are unordered.
            assert!(
                before.packet(pos).body() == after.packet(pos).body(),
                "{pos:?}"
            );
            assert_eq!(before.nbt.is_empty(), after.nbt.is_empty());
            if !before.nbt.is_empty() {
                let decode = |b: &[u8]| fastnbt::from_bytes::<Value>(b).map_err(io::Error::other);
                assert_eq!(decode(&before.nbt)?, decode(&after.nbt)?, "{pos:?}");
            }
        }
        let mut restored = Vec::new();
        for cx in -2..=-1 {
            for cz in -3..=-2 {
                reopened.ensure((cx, cz))?;
                restored.extend(reopened.raw[&(cx, cz)].tiles.values().cloned());
            }
        }
        for tile in tiles {
            assert!(restored.contains(&tile));
        }
        Ok(())
    }
}
