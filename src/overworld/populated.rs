use super::monument::MonumentChanges;
use super::temple::scattered::{ScatteredChanges, structure_mob};
use super::village::{PopulationDelta, populate_villages};
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
use crate::anvil::structures::{Family, StructureFile};
use crate::arena::{Column, ColumnPos};
mod mines;
use crossbeam_channel::{Receiver, Sender};
use fastnbt::Value;
use flate2::read::GzDecoder;
use mines::MineDelta;
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
fn place_holds(
    provider: &Overworld,
    index: &StructureFile,
    ring: [ColumnPos; 3],
    (cx, cz): ColumnPos,
    world: &mut impl DungeonWorld,
    rng: &mut JavaRandom,
) -> io::Result<Vec<Vec<u8>>> {
    let mut changes = Vec::new();
    for (sx, sz) in ring {
        if (cx - sx).abs() > 8 || (cz - sz).abs() > 8 {
            continue;
        }
        let loaded = index.get(sx, sz);
        let temporary;
        let start = if let Some(start) = loaded {
            start
        } else {
            temporary = provider
                .stronghold_start_nbt(sx, sz)
                .ok_or_else(|| io::Error::other("missing stronghold"))?;
            &temporary
        };
        if let Some(Value::IntArray(bb)) = start.get("BB")
            && bb.len() == 6
            && (bb[0] > cx * 16 + 23
                || bb[3] < cx * 16 + 8
                || bb[2] > cz * 16 + 23
                || bb[5] < cz * 16 + 8)
        {
            if loaded.is_none() {
                changes.push(fastnbt::to_bytes(start).map_err(io::Error::other)?);
            }
            continue;
        }
        let mut next = start.clone();
        super::stronghold::populate_start(&mut next, world, [cx, cz], rng)?;
        if loaded.is_none() || next != *start {
            changes.push(fastnbt::to_bytes(&next).map_err(io::Error::other)?);
        }
    }
    Ok(changes)
}
fn column([x, _, z]: Pos) -> ColumnPos {
    (x >> 4, z >> 4)
}
fn index([x, y, z]: Pos) -> usize {
    ((y << 8) | ((z & 15) << 4) | (x & 15)) as usize
}

struct Raw {
    // A conservative mask of sections containing a light source bright enough
    // to suppress frost. Stale bits after removal only cause an extra search.
    bright_sections: u16,
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
        if emission(state >> 4) >= 10 {
            self.bright_sections |= 1 << (i >> 12);
        }
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
    #[serde(skip)]
    village: PopulationDelta,
    #[serde(skip)]
    features_enabled: bool,
    #[serde(skip)]
    monument: MonumentChanges,
    #[serde(skip)]
    scattered: ScatteredChanges,
    #[serde(skip)]
    pyramid_chests: Vec<[i32; 3]>,
    #[serde(skip)]
    holds: Vec<Vec<u8>>,
    #[serde(skip)]
    mines: MineDelta,
    owner: ColumnPos,
    generators: DecoratorState,
    columns: Vec<(ColumnPos, Changes)>,
}
impl Record {
    fn changes_inputs(&self, generators: &DecoratorState) -> bool {
        self.generators != *generators
            || !self.village.starts.is_empty()
            || !self.mines.starts.is_empty()
            || !self.scattered.starts.is_empty()
            || !self.monument.starts.is_empty()
            || !self.pyramid_chests.is_empty()
            || !self.holds.is_empty()
    }

    fn update_features(
        &self,
        provider: &Overworld,
        features: &mut [(Family, StructureFile, bool); 2],
    ) -> io::Result<()> {
        if !self.features_enabled {
            return Ok(());
        }
        for (family, file, dirty) in features {
            *dirty |= provider.discover_features(*family, self.owner, file)?;
            if *family == Family::Monument && !self.monument.starts.is_empty() {
                self.monument.apply(file)?;
                *dirty = true;
            }
            if *family == Family::Temple
                && (!self.pyramid_chests.is_empty() || !self.scattered.starts.is_empty())
            {
                self.scattered.apply(file)?;
                file.record_pyramid_chests(&self.pyramid_chests)?;
                *dirty = true;
            }
        }
        Ok(())
    }
}
#[derive(Default)]
struct DirtyBlocks {
    sections: [Option<Box<[u64; 64]>>; 16],
    count: usize,
}

impl DirtyBlocks {
    fn mark(&mut self, offset: usize) {
        let section = self.sections[offset >> 12].get_or_insert_with(|| Box::new([0; 64]));
        let word = &mut section[(offset >> 6) & 63];
        let bit = 1 << (offset & 63);
        self.count += usize::from(*word & bit == 0);
        *word |= bit;
    }

    fn offsets(&self) -> impl Iterator<Item = u16> + '_ {
        self.sections.iter().enumerate().filter_map(|(y, s)| Some((y, s.as_deref()?)))
            .flat_map(|(y, section)| section.iter().copied().enumerate().flat_map(move |(word, mut bits)| {
                let base = (y * 4096 + word * 64) as u16;
                std::iter::from_fn(move || {
                    if bits == 0 { return None; }
                    let offset = base + bits.trailing_zeros() as u16;
                    bits &= bits - 1;
                    Some(offset)
                })
            }))
    }
}

struct Neighbors<'a> {
    origin: ColumnPos,
    columns: [Option<&'a mut Raw>; 16],
    dirty: [DirtyBlocks; 16],
    outside: Cell<bool>,
}
fn emission(id: u16) -> u8 {
    super::climate::block_properties(id)[1]
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

    // Only block light >= 10 affects snow and ice. An emitter can reach the
    // target from at most five steps away, including across chunk boundaries.
    fn warm_light(&self, pos: Pos) -> bool {
        let [x, y, z] = pos;
        let sections =
            (u16::MAX << ((y - 5).max(0) >> 4)) & (u16::MAX >> (15 - ((y + 5).min(255) >> 4)));
        if !((x - 5) >> 4..=(x + 5) >> 4).any(|cx| {
            ((z - 5) >> 4..=(z + 5) >> 4).any(|cz| {
                self.get((cx, cz))
                    .is_some_and(|raw| raw.bright_sections & sections != 0)
            })
        }) {
            return false;
        }
        let mut pending = vec![(pos, 10u8)];
        let mut visited = HashMap::from([(pos, 10u8)]);
        while let Some(([x, y, z], needed)) = pending.pop() {
            let id = self.state([x, y, z]) >> 4;
            if emission(id) >= needed {
                return true;
            }
            let next = needed + super::climate::block_properties(id)[0].max(1);
            if next > 15 {
                continue;
            }
            for p in [
                [x - 1, y, z],
                [x + 1, y, z],
                [x, y - 1, z],
                [x, y + 1, z],
                [x, y, z - 1],
                [x, y, z + 1],
            ] {
                if (0..256).contains(&p[1]) && visited.get(&p).is_none_or(|&old| next < old) {
                    visited.insert(p, next);
                    pending.push((p, next));
                }
            }
        }
        false
    }

    fn surface_weather(&mut self, (cx, cz): ColumnPos) {
        for x in cx * 16 + 8..cx * 16 + 24 {
            for z in cz * 16 + 8..cz * 16 + 24 {
                let Some(biome) = self.loaded_biome(x, z) else {
                    continue;
                };
                let mut y = self.height(x, z);
                while y > 0 {
                    let block = [x, y - 1, z];
                    if super::climate::block_properties(self.state(block)>>4)[2]&4!=0 {
                        break;
                    }
                    y -= 1;
                }
                let below = [x, y - 1, z];
                if self.can_freeze(below) {
                    self.set_state(below, 79 << 4);
                }
                let pos = [x, y, z];
                if y >= 256
                    || self.state(pos) != 0
                    || super::climate::temperature(biome, pos) > 0.15
                {
                    continue;
                }
                let support = self.state(below);
                let holds_snow = match support >> 4 {
                    78 => support & 7 == 7,
                    id => super::climate::block_properties(id)[2]&1!=0,
                };
                if holds_snow && !self.warm_light(pos) {
                    self.set_state(pos, 78 << 4);
                }
            }
        }
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
            self.dirty[slot].mark(i);
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
                super::climate::temperature(c.biomes[index([p[0], 0, p[2]])], p) <= 0.15
            })
            && !self.warm_light(p)
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
        if let Some(slot) = self.slot(column(p))
            && let Some(c) = self.columns[slot].as_deref_mut()
        {
            c.tiles.insert(index(p), nbt);
            self.dirty[slot].mark(index(p));
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
            self.dirty[slot].mark(i);
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
    villages: StructureFile,
    dirty_villages: bool,
    provider: Overworld,
    features: [(Family, StructureFile, bool); 2],
    holds: StructureFile,
    dirty_holds: bool,
    mine_index: StructureFile,
    world_dir: PathBuf,
    dirty_index: bool,
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
            return self.save_index();
        }
        let mut tmp = tempfile::NamedTempFile::new_in(&self.dir)?;
        {
            let mut out = BufWriter::new(tmp.as_file_mut());
            out.write_all(b"POP5")?;
            bincode::serialize_into(&mut out, &(self.pending.len() as u64))
                .map_err(io::Error::other)?;
            for rec in &self.pending {
                bincode::serialize_into(&mut out, rec.as_ref()).map_err(io::Error::other)?;
                bincode::serialize_into(&mut out, &rec.mines).map_err(io::Error::other)?;
                bincode::serialize_into(&mut out, &rec.holds).map_err(io::Error::other)?;
                bincode::serialize_into(&mut out, &rec.pyramid_chests).map_err(io::Error::other)?;
                bincode::serialize_into(&mut out, &rec.scattered).map_err(io::Error::other)?;
                bincode::serialize_into(&mut out, &rec.monument).map_err(io::Error::other)?;
                bincode::serialize_into(&mut out, &rec.features_enabled)
                    .map_err(io::Error::other)?;
                bincode::serialize_into(&mut out, &rec.village).map_err(io::Error::other)?;
            }
            out.flush()?;
        }
        tmp.as_file().sync_all()?;
        tmp.persist_noclobber(self.dir.join(format!("{:016x}.bin", self.saved)))
            .map_err(|e| e.error)?;
        self.saved += self.pending.len();
        for record in &self.pending {
            record.village.apply(&mut self.villages)?;
            self.dirty_villages |= !record.village.starts.is_empty();
            record.update_features(&self.provider, &mut self.features)?;
            record.mines.update(&mut self.mine_index)?;
            for raw in &record.holds {
                self.holds
                    .insert(fastnbt::from_bytes(raw).map_err(io::Error::other)?)?;
            }
            self.dirty_holds |= !record.holds.is_empty();
            self.dirty_index |= !record.mines.starts.is_empty();
        }
        self.pending.clear();
        self.save_index()
    }
    fn save_index(&mut self) -> io::Result<()> {
        if self.dirty_villages {
            self.villages.save(&self.world_dir)?;
            self.dirty_villages = false;
        }
        for (_, file, dirty) in &mut self.features {
            if *dirty {
                file.save(&self.world_dir)?;
                *dirty = false;
            }
        }
        if self.dirty_holds {
            self.holds.save(&self.world_dir)?;
            self.dirty_holds = false;
        }
        if self.dirty_index {
            self.mine_index.save(&self.world_dir)?;
            self.dirty_index = false;
        }
        Ok(())
    }
}
/// Each committed owner records its writes to every neighbor together with
/// the biome generators' continuation state. A cached column can be evicted
/// and reconstructed from its primer and the ordered records that touched it.
pub(crate) struct Population {
    villages: StructureFile,
    population_mobs: BTreeMap<ColumnPos, Vec<Value>>,
    temples: StructureFile,
    monuments: StructureFile,
    structure_mobs: BTreeMap<ColumnPos, BTreeMap<Pos, Value>>,
    holds: StructureFile,
    pub(crate) ring: [ColumnPos; 3],
    pub(crate) eye_targets: Arc<std::sync::RwLock<Vec<Pos>>>,
    mine_index: StructureFile,
    carts: BTreeMap<ColumnPos, BTreeMap<Pos, Value>>,
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
    #[cfg(test)]
    pub(crate) fn open(seed: i64, path: Option<&Path>) -> io::Result<Self> {
        Self::open_with_world(seed, path, path)
    }
    pub(crate) fn open_with_world(
        seed: i64,
        path: Option<&Path>,
        world: Option<&Path>,
    ) -> io::Result<Self> {
        let villages = match world {
            Some(root) => StructureFile::load(root, Family::Village)?,
            None => StructureFile::empty(Family::Village),
        };
        let mine_index = match world {
            Some(root) => StructureFile::load(root, Family::Mineshaft)?,
            None => StructureFile::empty(Family::Mineshaft),
        };
        let holds = match world {
            Some(root) => StructureFile::load(root, Family::Stronghold)?,
            None => StructureFile::empty(Family::Stronghold),
        };
        let provider = Arc::new(Overworld::new(seed));
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
                    villages: villages.clone(),
                    dirty_villages: false,
                    provider: Overworld::new(seed),
                    features: [
                        (
                            Family::Temple,
                            StructureFile::load(world.unwrap_or(dir), Family::Temple)?,
                            false,
                        ),
                        (
                            Family::Monument,
                            StructureFile::load(world.unwrap_or(dir), Family::Monument)?,
                            false,
                        ),
                    ],
                    holds: holds.clone(),
                    dirty_holds: false,
                    mine_index: mine_index.clone(),
                    world_dir: world.unwrap_or(dir).to_owned(),
                    dirty_index: false,
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
            villages,
            population_mobs: Default::default(),
            structure_mobs: BTreeMap::new(),
            monuments: match world {
                Some(root) => StructureFile::load(root, Family::Monument)?,
                None => StructureFile::empty(Family::Monument),
            },
            temples: match world {
                Some(root) => StructureFile::load(root, Family::Temple)?,
                None => StructureFile::empty(Family::Temple),
            },
            ring: provider.stronghold_starts(),
            holds,
            eye_targets: Default::default(),
            mine_index,
            carts: BTreeMap::new(),
            provider,
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
                    if &magic != b"POP1"
                        && &magic != b"POP2"
                        && &magic != b"POP3"
                        && &magic != b"POP4"
                        && &magic != b"POP5"
                    {
                        return Err(io::Error::other("invalid population journal header"));
                    }
                    let count: u64 =
                        bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                    if count == 0 {
                        return Err(io::Error::other("empty population journal batch"));
                    }
                    for _ in 0..count {
                        let mut record: Record =
                            bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                        if &magic != b"POP1" {
                            record.mines =
                                bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                        }
                        if matches!(&magic, b"POP3" | b"POP4" | b"POP5") {
                            record.holds =
                                bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                        }
                        if matches!(&magic, b"POP4" | b"POP5") {
                            record.pyramid_chests =
                                bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                            record.scattered =
                                bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                            record.monument =
                                bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                            record.features_enabled =
                                bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                        }
                        if &magic == b"POP5" {
                            record.village =
                                bincode::deserialize_from(&mut reader).map_err(io::Error::other)?;
                        }
                        this.remember(record)?;
                    }
                    if reader.read(&mut [0; 1])? != 0 {
                        return Err(io::Error::other("trailing population journal bytes"));
                    }
                } else {
                    return Err(io::Error::other("incomplete population journal"));
                }
            }
            if let Some(j) = &mut this.journal {
                for record in &this.records {
                    record.update_features(&this.provider, &mut j.features)?;
                }
                j.saved = this.records.len();
                j.villages = this.villages.clone();
                j.dirty_villages = this.records.iter().any(|r| !r.village.starts.is_empty());
                j.mine_index = this.mine_index.clone();
                j.holds = this.holds.clone();
                j.dirty_holds = this.records.iter().any(|r| !r.holds.is_empty());
                j.dirty_index = this.records.iter().any(|r| !r.mines.starts.is_empty());
            }
        }
        this.eye_targets = Arc::new(std::sync::RwLock::new(this.holds.reopened_eye_targets()?));
        Ok(this)
    }
    fn remember(&mut self, rec: Record) -> io::Result<()> {
        if !self.owners.insert(rec.owner) {
            return Err(io::Error::other("duplicate population journal owner"));
        }
        if !rec.pyramid_chests.is_empty() {
            self.provider
                .discover_features(Family::Temple, rec.owner, &mut self.temples)?;
            self.temples.record_pyramid_chests(&rec.pyramid_chests)?;
        }
        rec.scattered.apply(&mut self.temples)?;
        rec.monument.apply(&mut self.monuments)?;
        for (positions, elder) in [
            (&rec.scattered.witches, false),
            (&rec.monument.elders, true),
        ] {
            for &pos in positions {
                self.structure_mobs
                    .entry(column(pos))
                    .or_default()
                    .insert(pos, structure_mob(self.provider.seed, pos, elder)?);
            }
        }
        rec.village.apply(&mut self.villages)?;
        for (pos, bytes) in &rec.village.mobs {
            self.population_mobs
                .entry(column(*pos))
                .or_default()
                .push(fastnbt::from_bytes(bytes).map_err(io::Error::other)?);
        }
        self.generators = rec.generators.clone();
        for raw in &rec.holds {
            let fields: HashMap<String, Value> =
                fastnbt::from_bytes(raw).map_err(io::Error::other)?;
            if let (Some(Value::Int(cx)),Some(Value::Int(cz)),Some(Value::List(children)))=(fields.get("ChunkX"),fields.get("ChunkZ"),fields.get("Children"))
                && self.holds.get(*cx,*cz).is_none()
                && let Some(bb)=children.iter().find_map(|piece|match piece{Value::Compound(piece) if matches!(piece.get("id"),Some(Value::String(id)) if id=="SHPR")=>match piece.get("BB"){Some(Value::IntArray(bb)) if bb.len()==6=>Some(bb),_=>None},_=>None})
            {self.eye_targets.write().map_err(|_|io::Error::other("eye target lock"))?.push(std::array::from_fn(|i|bb[i]+(bb[i+3]-bb[i]+1)/2));}
            self.holds.insert(fields)?;
        }
        rec.mines.update(&mut self.mine_index)?;
        for (pos, bytes) in &rec.mines.carts {
            self.carts
                .entry(column(*pos))
                .or_default()
                .insert(*pos, fastnbt::from_bytes(bytes).map_err(io::Error::other)?);
        }
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
                bright_sections: states.as_chunks::<4096>().0.iter().enumerate().fold(
                    0,
                    |mask, (i, section)| {
                        if section.iter().any(|s| emission(s >> 4) >= 10) {
                            mask | (1 << i)
                        } else {
                            mask
                        }
                    },
                ),
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
        self.decorate_owner(pos).map(|_| ())
    }
    fn decorate_owner(&mut self, pos: ColumnPos) -> io::Result<bool> {
        let discovered = self.discover(pos)?;
        let mut world = std::mem::take(&mut self.raw);
        let result = self.build_record(pos, &mut world, &self.monuments, &self.temples);
        self.raw = world;
        if result.is_err() {
            self.raw.clear();
            self.recency.clear();
        }
        let rec = result?;
        let changed = discovered || rec.changes_inputs(&self.generators);
        self.commit_record(rec)?;
        Ok(changed)
    }

    fn discover(&mut self, pos: ColumnPos) -> io::Result<bool> {
        let monument =
            self.provider
                .discover_features(Family::Monument, pos, &mut self.monuments)?;
        let temple = self
            .provider
            .discover_features(Family::Temple, pos, &mut self.temples)?;
        Ok(monument || temple)
    }

    fn build_record(
        &self,
        pos: ColumnPos,
        raw: &mut HashMap<ColumnPos, Raw>,
        monuments: &StructureFile,
        temples: &StructureFile,
    ) -> io::Result<Record> {
        let mut monument = Ok(MonumentChanges::default());
        let mut scattered = Ok(ScatteredChanges::default());
        let mut pyramid_chests = Vec::new();
        let mut world = Neighbors::new(raw, pos);
        let mut generators = self.generators.clone();
        let mut mines = Ok(MineDelta::default());
        let mut village = Ok(PopulationDelta::default());
        let mut holds = Ok(Vec::new());
        let mut random =
            self.provider
                .populate(&mut world, [pos.0, pos.1], &mut generators, |world, rng| {
                    if self.provider.structures {
                        mines = mines::place(&self.mine_index, self.provider.seed, pos, world, rng);
                        village =
                            populate_villages(&self.provider, &self.villages, pos, world, rng);
                        holds =
                            place_holds(&self.provider, &self.holds, self.ring, pos, world, rng);
                        pyramid_chests = self.provider.pyramid_chests(temples, pos, world, rng);
                        scattered = self.provider.scattered_pieces(temples, pos, world, rng);
                        monument = self.provider.monument_pieces(monuments, pos, world, rng);
                    }
                    village.as_ref().is_ok_and(|v| v.generated)
                });
        let mut village = village?;
        village.spawn_animals(&world, self.provider.seed, pos, &mut random)?;
        world.surface_weather(pos);
        if world.outside.get() {
            return Err(io::Error::other("population exceeded loaded neighbors"));
        }
        let mut columns = Vec::new();
        for (slot, blocks) in world.dirty.into_iter().enumerate() {
            if blocks.count == 0 {
                continue;
            }
            let pos = (
                world.origin.0 + (slot / 4) as i32,
                world.origin.1 + (slot % 4) as i32,
            );
            let c = world.columns[slot]
                .as_deref()
                .ok_or_else(|| io::Error::other("missing dirty column"))?;
            let mut changes = Changes { blocks: Vec::with_capacity(blocks.count), ..Default::default() };
            for i in blocks.offsets() {
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
            village,
            features_enabled: self.provider.structures,
            scattered: scattered?,
            monument: monument?,
            pyramid_chests,
            holds: holds?,
            mines: mines?,
            owner: pos,
            generators,
            columns,
        };
        Ok(rec)
    }

    fn commit_record(&mut self, record: Record) -> io::Result<()> {
        self.remember(record)?;
        if let Some(tx) = &self.journal_tx {
            let record = self
                .records
                .last()
                .ok_or_else(|| io::Error::other("missing population record"))?;
            tx.send(record.clone()).map_err(io::Error::other)?;
        }
        Ok(())
    }

    fn decorate_batch(&mut self, owners: &[ColumnPos]) -> io::Result<()> {
        fn halo((x, z): ColumnPos) -> impl Iterator<Item = ColumnPos> {
            (x - 1..=x + 2).flat_map(move |cx| (z - 1..=z + 2).map(move |cz| (cx, cz)))
        }
        // An owner is ready when every earlier reader or writer of any of
        // its 16 columns has committed. Later disjoint owners may then run
        // ahead without changing what an earlier owner sees.
        let mut last = HashMap::new();
        let dependencies: Vec<_> = owners
            .iter()
            .enumerate()
            .map(|(i, &pos)| {
                halo(pos)
                    .map(|p| last.insert(p, i + 1).unwrap_or(0))
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let width = rayon::current_num_threads().min(4);
        let mut staged = BTreeMap::<usize, (usize, io::Result<Record>)>::new();
        let mut revision = 0;
        for (i, &pos) in owners.iter().enumerate() {
            if !staged.contains_key(&i) && width > 1 {
                let ready: Vec<_> = (i..owners.len())
                    .filter(|j| dependencies[*j] <= i && !staged.contains_key(j))
                    .take(width)
                    .collect();
                if ready.len() > 1 {
                    let mut jobs = Vec::with_capacity(ready.len());
                    for j in ready {
                        let mut local = HashMap::with_capacity(16);
                        for p in halo(owners[j]) {
                            let col = self.raw.remove(&p).ok_or_else(|| {
                                io::Error::other("missing disjoint population column")
                            })?;
                            local.insert(p, col);
                        }
                        jobs.push((j, local));
                    }
                    let done: Vec<_> = jobs
                        .into_par_iter()
                        .map(|(j, mut local)| {
                            let owner = owners[j];
                            let rec = (|| {
                                let mut monuments = self.monuments.clone();
                                let mut temples = self.temples.clone();
                                self.provider.discover_features(
                                    Family::Monument,
                                    owner,
                                    &mut monuments,
                                )?;
                                self.provider.discover_features(
                                    Family::Temple,
                                    owner,
                                    &mut temples,
                                )?;
                                self.build_record(owner, &mut local, &monuments, &temples)
                            })();
                            (j, local, rec)
                        })
                        .collect();
                    for (j, local, rec) in done {
                        self.raw.extend(local);
                        staged.insert(j, (revision, rec));
                    }
                }
            }
            let changed = match staged.remove(&i) {
                Some((version, result)) if version == revision => {
                    let rec = result?;
                    let changed = self.discover(pos)? || rec.changes_inputs(&self.generators);
                    self.commit_record(rec)?;
                    changed
                }
                Some(_) => {
                    // Restore the authoritative primer and journal before retrying
                    // with updated structure indexes or biome-generator state.
                    let positions: Vec<_> = halo(pos).collect();
                    let build = Self::primer_builder(self.provider.clone(), &positions);
                    for p in positions {
                        let mut raw = build(p);
                        raw.last_used = self.raw[&p].last_used;
                        self.insert_raw(p, raw)?;
                    }
                    self.decorate_owner(pos)?
                }
                None => self.decorate_owner(pos)?,
            };
            revision += usize::from(changed);
        }
        Ok(())
    }
    pub(crate) fn columns(&mut self, positions: &[ColumnPos]) -> io::Result<Vec<Column>> {
        #[cfg(test)]
        let start = std::time::Instant::now();
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
        // Pin the entire working set before inserting new primers. Evict only
        // nonparticipants, so replay and decoration always see a full halo.
        let mut excess = (self.raw.len() + missing.len()).saturating_sub(RAW_CACHE_CAPACITY);
        if excess > 0 {
            self.recency.retain(|&(_, pos)| {
                if excess > 0 && !wanted.contains(&pos) {
                    self.raw.remove(&pos);
                    excess -= 1;
                    return false;
                }
                true
            });
        }
        let build = Self::primer_builder(self.provider.clone(), &missing);
        let generated: Vec<_> = missing.par_iter().map(|&p| (p, build(p))).collect();
        for (p, raw) in generated {
            self.insert_raw(p, raw)?;
            self.recency.insert((0, p));
        }
        self.touch(&required);
        #[cfg(test)]
        let primers_done = std::time::Instant::now();
        if let Err(error) = self.decorate_batch(&owners) {
            // Drop uncommitted speculative writes. The next request replays
            // only records that completed in reference order.
            self.raw.clear();
            self.recency.clear();
            return Err(error);
        }
        #[cfg(test)]
        let decorated = std::time::Instant::now();
        let result = positions.par_iter().map(|&p| self.pack_column(p)).collect();
        #[cfg(test)]
        if std::env::var_os("SYRUST_POPULATION_TIMING").is_some() {
            println!(
                "Cold batch: {} primers, {} owners. Primer {:?}, decoration {:?}, packing {:?}",
                missing.len(),
                owners.len(),
                primers_done - start,
                decorated - primers_done,
                decorated.elapsed()
            );
        }
        result
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
            self.carts
                .get(&pos)
                .into_iter()
                .flat_map(|carts| carts.values().cloned())
                .chain(
                    self.structure_mobs
                        .get(&pos)
                        .into_iter()
                        .flat_map(|m| m.values().cloned()),
                )
                .chain(
                    self.population_mobs
                        .get(&pos)
                        .into_iter()
                        .flatten()
                        .cloned(),
                )
                .collect(),
        )?;
        Ok(col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_structure_mobs_survive_anvil_reopening() -> io::Result<()> {
        for (start, family, mob, count) in [
            ((146, -509), Family::Temple, "Witch", 1),
            ((-249, -508), Family::Monument, "Guardian", 3),
        ] {
            let root = tempfile::tempdir()?;
            let log = root.path().join("population");
            let mut expected = BTreeMap::new();
            for pass in 0..2 {
                let mut p = Population::open_with_world(12345, Some(&log), Some(root.path()))?;
                let mut data = StructureFile::load(root.path(), family)?;
                p.provider.discover_features(family, start, &mut data)?;
                let Some(Value::IntArray(b)) = data.get(start.0, start.1).and_then(|s| s.get("BB"))
                else {
                    panic!("missing start")
                };
                for x in ((b[0] - 8) >> 4)..=((b[3] - 8) >> 4) {
                    for z in ((b[2] - 8) >> 4)..=((b[5] - 8) >> 4) {
                        p.populate_owner((x, z))?;
                    }
                }
                p.journal.as_mut().unwrap().flush()?;
                let columns: Vec<_> = p.structure_mobs.keys().copied().collect();
                let mut found = BTreeMap::new();
                for col in columns {
                    p.ensure(col)?;
                    let packed = p.pack_column(col)?;
                    let nbt: HashMap<String, Value> =
                        fastnbt::from_bytes(&packed.nbt).map_err(io::Error::other)?;
                    let Value::Compound(level) = &nbt["Level"] else {
                        panic!()
                    };
                    let Value::List(entities) = &level["Entities"] else {
                        panic!()
                    };
                    for e in entities {
                        let Value::Compound(tag) = e else { panic!() };
                        if tag.get("id") != Some(&Value::String(mob.into())) {
                            continue;
                        }
                        let Value::Long(uuid) = tag["UUIDLeast"] else {
                            panic!()
                        };
                        assert_eq!(tag.get("Elder") == Some(&Value::Byte(1)), mob == "Guardian");
                        assert!(found.insert(uuid, e.clone()).is_none());
                    }
                }
                assert_eq!(found.len(), count);
                if pass == 0 {
                    expected = found;
                } else {
                    assert_eq!(found, expected);
                }
            }
        }
        Ok(())
    }
    #[test]
    fn pyramid_chests_and_trap_survive_reopening() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let journal = root.path().join("population");
        let pos = (-191, -493);
        let mut expected: HashMap<String, Value> = fastnbt::from_reader(GzDecoder::new(
            &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-3/desert-loot.nbt.gz")[..],
        ))
        .map_err(io::Error::other)?;
        expected.remove("Next");
        for _ in 0..2 {
            let mut population =
                Population::open_with_world(12345, Some(&journal), Some(root.path()))?;
            population.populate_owner(pos)?;
            population.ensure(pos)?;
            let col = &population.raw[&pos];
            for z in 9..=11 {
                for x in 9..=11 {
                    assert_eq!(col.states[index([x, 51, z])], 46 << 4);
                }
            }
            assert_eq!(col.states[index([10, 53, 10])], 70 << 4);
            for (key, nbt) in &expected {
                let Value::Compound(tag) = nbt else {
                    panic!("invalid reference chest")
                };
                let xyz = ["x", "y", "z"].map(|axis| match tag[axis] {
                    Value::Int(v) => v,
                    _ => panic!("invalid reference position"),
                });
                assert_eq!(col.tiles.get(&index(xyz)), Some(nbt), "{key}");
            }
            population.journal.as_mut().unwrap().flush()?;
            let saved = StructureFile::load(root.path(), Family::Temple)?;
            let Some(Value::List(children)) = saved
                .get(pos.0, pos.1)
                .and_then(|start| start.get("Children"))
            else {
                panic!("missing pyramid")
            };
            let Value::Compound(piece) = &children[0] else {
                panic!("missing piece")
            };
            for flag in 0..4 {
                assert_eq!(piece[&format!("hasPlacedChest{flag}")], Value::Byte(1));
            }
        }
        Ok(())
    }

    #[test]
    fn underground_population_on_official_carved_neighbors() -> io::Result<()> {
        use std::io::Read;
        for (hold, cx, cz, compressed) in [
            (
                true,
                -44,
                59,
                &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-2/stronghold/terrain.gz")[..],
            ),
            (
                false,
                -31,
                -32,
                &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-2/mineshaft/terrain.gz")[..],
            ),
        ] {
            let mut population = Population::open(12345, None)?;
            let mut start = if hold {
                population.provider.stronghold_start_nbt(cx, cz)
            } else {
                super::super::mineshaft::start_nbt(12345, cx, cz)
            }
            .unwrap();
            let Value::IntArray(bb) = &start["BB"] else {
                panic!()
            };
            let bb: [i32; 6] = bb[..].try_into().unwrap();
            let mut bytes = Vec::new();
            GzDecoder::new(compressed).read_to_end(&mut bytes)?;
            let mut input = &bytes[..];
            let first = [(bb[0] - 8) >> 4, (bb[2] - 8) >> 4];
            let last = [(bb[3] - 8) >> 4, (bb[5] - 8) >> 4];
            let cols = (first[0] - 1..=last[0] + 2)
                .flat_map(|x| (first[1] - 1..=last[1] + 2).map(move |z| (x, z)))
                .collect::<Vec<_>>();
            population.prepare(&cols)?;
            for x in first[0]..=last[0] {
                for z in first[1]..=last[1] {
                    let mut world = Neighbors::new(&mut population.raw, (x, z));
                    let mut rng = JavaRandom::new(super::super::population_seed(12345, x, z));
                    if hold {
                        super::super::stronghold::populate_start(
                            &mut start,
                            &mut world,
                            [x, z],
                            &mut rng,
                        )?;
                    } else {
                        super::super::mineshaft::populate_start(
                            &mut start,
                            &mut world,
                            [x, z],
                            &mut rng,
                            |_| {},
                        )?;
                    }
                    assert!(!world.outside.get());
                    let mut next = [0; 4];
                    input.read_exact(&mut next)?;
                    assert_eq!(
                        rng.bits(32),
                        u32::from_be_bytes(next),
                        "rng hold={hold} {x},{z}"
                    );
                }
            }
            for y in bb[1] - 1..=bb[4] {
                for z in bb[2]..=bb[5] {
                    for x in bb[0]..=bb[3] {
                        let p = [x, y, z];
                        let mut block = [0; 2];
                        input.read_exact(&mut block)?;
                        assert_eq!(
                            population.raw[&column(p)].states[index(p)],
                            u16::from_be_bytes(block),
                            "hold={hold} {x},{y},{z}"
                        );
                    }
                }
            }
            if hold {
                let expected: HashMap<String, Value> =
                    fastnbt::from_bytes(input).map_err(io::Error::other)?;
                let mut actual = HashMap::new();
                for (&(x, z), col) in &population.raw {
                    for (&i, tag) in &col.tiles {
                        actual.insert(
                            format!(
                                "{},{},{}",
                                x * 16 + (i & 15) as i32,
                                i >> 8,
                                z * 16 + ((i >> 4) & 15) as i32
                            ),
                            tag.clone(),
                        );
                    }
                }
                assert_eq!(actual, expected);
            } else {
                assert!(input.is_empty());
            }
        }
        Ok(())
    }
    #[test]
    fn explored_stronghold_reopens_with_library_portal_and_eye() -> io::Result<()> {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path();
        let path = root.join("population");
        let mut population = Population::open_with_world(12345, Some(&path), Some(root))?;
        let start = population.provider.stronghold_start_nbt(-44, 59).unwrap();
        let Value::List(children) = &start["Children"] else {
            panic!()
        };
        let mut owners = BTreeSet::new();
        for child in children {
            let Value::Compound(fields) = child else {
                panic!()
            };
            let Value::String(id) = &fields["id"] else {
                panic!()
            };
            if id != "SHLi" && id != "SHPR" {
                continue;
            }
            let Value::IntArray(bb) = &fields["BB"] else {
                panic!()
            };
            for x in ((bb[0] - 8) >> 4)..=((bb[3] - 8) >> 4) {
                for z in ((bb[2] - 8) >> 4)..=((bb[5] - 8) >> 4) {
                    owners.insert((x, z));
                }
            }
        }
        for &owner in &owners {
            population.populate_owner(owner)?;
        }
        assert_eq!(*population.eye_targets.read().unwrap(), [[-734, 44, 965]]);
        let mut expected = BTreeMap::new();
        for (&pos, col) in &population.raw {
            if !col.tiles.is_empty() {
                expected.insert(pos, col.tiles.clone());
            }
        }
        let tags: Vec<_> = expected.values().flat_map(|tiles| tiles.values()).collect();
        assert!(tags.iter().any(|tag|matches!(tag,Value::Compound(v) if matches!(v.get("EntityId"),Some(Value::String(id)) if id=="Silverfish"))));
        assert!(tags.iter().any(|tag|matches!(tag,Value::Compound(v) if matches!(v.get("Items"),Some(Value::List(items)) if !items.is_empty()))));
        population.journal.as_mut().unwrap().flush()?;
        assert!(root.join("data/Stronghold.dat").is_file());
        drop(population);
        let mut reopened = Population::open_with_world(12345, Some(&path), Some(root))?;
        assert_eq!(*reopened.eye_targets.read().unwrap(), [[-700, 43, 948]]);
        for owner in owners {
            reopened.populate_owner(owner)?;
        }
        let mut columns = BTreeMap::new();
        for (&pos, tiles) in &expected {
            reopened.ensure(pos)?;
            assert_eq!(&reopened.raw[&pos].tiles, tiles);
            columns.insert(pos, reopened.pack_column(pos)?);
        }
        let region = root.join("region");
        crate::anvil::export(
            &region,
            &crate::arena::Template {
                spawn_hint: None,
                columns,
            },
            &Default::default(),
        )?;
        let restored = crate::anvil::import(&region)?;
        for (pos, tiles) in expected {
            let nbt: HashMap<String, Value> =
                fastnbt::from_bytes(&restored.columns[&pos].nbt).map_err(io::Error::other)?;
            let Value::Compound(level) = &nbt["Level"] else {
                panic!()
            };
            assert_eq!(
                level["TileEntities"],
                Value::List(tiles.into_values().collect())
            );
        }
        Ok(())
    }
    #[test]
    fn mined_corridors_reopen_with_the_same_cart_and_spawner() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path();
        let path = root.join("population");
        let mut population = Population::open_with_world(12345, Some(&path), Some(root))?;
        let mut spider = None;
        let mut found_cart = None;
        for (sx, sz) in [
            (-31, -32),
            (-24, 4),
            (-22, -4),
            (-11, 30),
            (22, -4),
            (28, 29),
        ] {
            let start = crate::overworld::mineshaft::start_nbt(12345, sx, sz).unwrap();
            let Value::IntArray(bb) = &start["BB"] else {
                panic!()
            };
            for cx in ((bb[0] - 8) >> 4)..=((bb[3] - 8) >> 4) {
                for cz in ((bb[2] - 8) >> 4)..=((bb[5] - 8) >> 4) {
                    population.populate_owner((cx, cz))?;
                    for (chunk, raw) in &population.raw {
                        for (offset, tag) in &raw.tiles {
                            if let Value::Compound(fields) = tag
                                && matches!(fields.get("EntityId"),Some(Value::String(id)) if id=="CaveSpider")
                            {
                                spider = Some((*chunk, *offset, tag.clone()));
                            }
                        }
                    }
                    if let Some((chunk, carts)) = population.carts.first_key_value() {
                        found_cart = Some((*chunk, carts.clone()));
                    }
                    if spider.is_some() && found_cart.is_some() {
                        break;
                    }
                }
                if spider.is_some() && found_cart.is_some() {
                    break;
                }
            }
            if spider.is_some() && found_cart.is_some() {
                break;
            }
        }
        let (cart_pos, expected_carts) = found_cart.expect("actual generated minecart");
        let (spider_pos, offset, expected_spider) = spider.expect("actual generated spawner");
        population.journal.as_mut().unwrap().flush()?;
        assert!(root.join("data/Mineshaft.dat").is_file());
        let owners = population.owners.len();
        let original_owners: Vec<_> = population.owners.iter().copied().collect();
        drop(population);
        let mut reopened = Population::open_with_world(12345, Some(&path), Some(root))?;
        for owner in original_owners {
            reopened.populate_owner(owner)?;
        }
        reopened.ensure(cart_pos)?;
        reopened.ensure(spider_pos)?;
        assert_eq!(reopened.owners.len(), owners);
        assert_eq!(reopened.raw[&spider_pos].tiles[&offset], expected_spider);
        assert_eq!(reopened.carts[&cart_pos], expected_carts);
        let col = reopened.pack_column(cart_pos)?;
        let nbt: HashMap<String, Value> =
            fastnbt::from_bytes(&col.nbt).map_err(io::Error::other)?;
        let Value::Compound(level) = &nbt["Level"] else {
            panic!()
        };
        assert_eq!(
            level["Entities"],
            Value::List(expected_carts.into_values().collect())
        );
        let mut columns = BTreeMap::from([(cart_pos, col)]);
        columns.insert(spider_pos, reopened.pack_column(spider_pos)?);
        let mut region = crate::arena::Template {
            spawn_hint: None,
            columns,
        };
        let region_dir = root.join("region");
        for _ in 0..2 {
            crate::anvil::export(&region_dir, &region, &crate::anvil::Edits::new())?;
            region = crate::anvil::import(&region_dir)?;
            let cart: HashMap<String, Value> =
                fastnbt::from_bytes(&region.columns[&cart_pos].nbt).map_err(io::Error::other)?;
            assert_eq!(cart["Level"], nbt["Level"]);
            let spider: HashMap<String, Value> =
                fastnbt::from_bytes(&region.columns[&spider_pos].nbt).map_err(io::Error::other)?;
            let Value::Compound(spider) = &spider["Level"] else {
                panic!()
            };
            let Value::List(tiles) = &spider["TileEntities"] else {
                panic!()
            };
            assert_eq!(
                tiles.iter().filter(|tag| **tag == expected_spider).count(),
                1
            );
        }
        Ok(())
    }

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
    fn disjoint_population_windows_match_sequential_records() -> io::Result<()> {
        let owners = [(-5, 0), (4, 0), (5, 0)];
        let positions: Vec<_> = owners
            .iter()
            .copied()
            .flat_map(|(x, z)| {
                (x - 1..=x + 2).flat_map(move |cx| (z - 1..=z + 2).map(move |cz| (cx, cz)))
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut serial = Population::open(12345, None)?;
        let mut parallel = Population::open(12345, None)?;
        serial.prepare(&positions)?;
        parallel.prepare(&positions)?;
        for owner in owners {
            serial.decorate_owner(owner)?;
        }
        rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .map_err(io::Error::other)?
            .install(|| parallel.decorate_batch(&owners))?;
        for p in positions {
            let expected = &serial.raw[&p];
            let actual = &parallel.raw[&p];
            assert!(expected.states == actual.states, "block states at {p:?}");
            assert_eq!(expected.heightmap, actual.heightmap);
            assert_eq!(expected.sky, actual.sky);
            assert_eq!(expected.tiles, actual.tiles);
            assert_eq!(expected.ticks, actual.ticks);
        }
        assert!(parallel.generators == serial.generators);
        Ok(())
    }

    #[test]
    fn batched_population_preserves_order_dependent_ore_and_light() -> io::Result<()> {
        let positions: Vec<_> = (-2..=1)
            .flat_map(|x| (-2..=1).map(move |z| (x, z)))
            .collect();
        let mut reference = Population::open(4862413858852300341, None)?;
        let mut candidate = Population::open(4862413858852300341, None)?;
        for &pos in &positions {
            reference.populate_owner(pos)?;
        }
        candidate.columns(&[(0, 0)])?;
        // Grouping these owners by parity replaces this iron ore with stone.
        assert_eq!(
            candidate.raw[&(-2, -1)].states[index([-24, 1, -5])],
            15 << 4
        );
        for (pos, expected) in &reference.raw {
            let actual = &candidate.raw[pos];
            assert!(actual.states == expected.states, "block states at {pos:?}");
            assert_eq!(actual.heightmap, expected.heightmap);
            assert_eq!(actual.sky, expected.sky);
        }
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
    fn winter_surface_crosses_chunks_and_observes_light() {
        let mut raw = HashMap::new();
        for x in -1..=2 {
            for z in -1..=2 {
                raw.insert(
                    (x, z),
                    Raw {
                        bright_sections: 0,
                        states: vec![0; 65536],
                        biomes: [12; 256],
                        tiles: Default::default(),
                        ticks: Default::default(),
                        last_used: 0,
                        heightmap: [0; 256],
                        solid_top: [0; 256],
                        sky: Box::new([[0; 15]; 256]),
                    },
                );
            }
        }
        let mut w = Neighbors::new(&mut raw, (0, 0));
        for (x, z, id) in [
            (8, 8, 9),
            (15, 16, 2),
            (16, 16, 18),
            (22, 22, 2),
            (23, 22, 50),
            (7, 8, 2),
        ] {
            w.set_state([x, 64, z], id << 4);
        }
        w.surface_weather((0, 0));
        assert_eq!(w.state([8, 64, 8]), 79 << 4);
        assert_eq!(w.state([8, 65, 8]), 0);
        assert_eq!(w.state([15, 65, 16]), 78 << 4);
        assert_eq!(w.state([16, 65, 16]), 78 << 4);
        assert_eq!(w.state([22, 65, 22]), 0);
        assert_eq!(w.state([7, 65, 8]), 0);
    }

    #[test]
    fn complete_village_reopens_without_spawning_twice() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let seed = 12345;
        let (cx, cz) = (-128, 118);
        let dir = root.path().join("journal");
        let mut p = Population::open_with_world(seed, Some(&dir), Some(root.path()))?;
        let start = p.provider.village_start_nbt(cx, cz).unwrap();
        let Value::IntArray(b) = &start["BB"] else {
            panic!("village bounds")
        };
        let mut owners = Vec::new();
        for x in (b[0] - 8) >> 4..=(b[3] - 8) >> 4 {
            for z in (b[2] - 8) >> 4..=(b[5] - 8) >> 4 {
                p.populate_owner((x, z))?;
                owners.push((x, z));
            }
        }
        p.journal.as_mut().unwrap().flush()?;
        assert!(owners.len() > 1);
        let villagers = |p: &Population| {
            p.population_mobs.values().flatten().filter(|v|matches!(v,Value::Compound(tag) if tag.get("id")==Some(&Value::String("Villager".into())))).count()
        };
        let count = villagers(&p);
        assert!(count > 0);
        let disk = StructureFile::load(root.path(), Family::Village)?;
        let saved = disk.get(cx, cz).unwrap().clone();
        assert_eq!(Some(&saved), p.villages.get(cx, cz));
        let Value::List(pieces) = &saved["Children"] else {
            panic!("village pieces")
        };
        for piece in pieces {
            let Value::Compound(tag) = piece else {
                panic!("compound")
            };
            if tag["id"] != Value::String("ViSR".into()) {
                assert_ne!(tag["HPos"], Value::Int(-1));
            }
        }
        drop(p);
        let mut p = Population::open_with_world(seed, Some(&dir), Some(root.path()))?;
        let records = p.records.len();
        for owner in owners {
            p.populate_owner(owner)?;
        }
        assert_eq!(records, p.records.len());
        assert_eq!(villagers(&p), count);
        assert_eq!(Some(&saved), p.villages.get(cx, cz));
        Ok(())
    }

    #[test]
    fn cached_queries_follow_updates_and_column_limits() {
        let mut raw = HashMap::from([(
            (-1, -1),
            Raw {
                bright_sections: 0,
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
