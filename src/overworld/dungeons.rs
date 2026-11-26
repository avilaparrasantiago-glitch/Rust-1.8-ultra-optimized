use super::{JavaRandom, lakes::LakeWorld};
use fastnbt::Value;
use std::collections::{BTreeMap, HashMap};
mod tables;

type Enchantment = (i16, i16, i32);
type Pos = [i32; 3];

pub trait DungeonWorld: LakeWorld {
    fn loaded_biome(&self, _x: i32, _z: i32) -> Option<u8> {
        None
    }
    fn full_block(&self, pos: Pos) -> bool;
    fn set_tile(&mut self, pos: Pos, nbt: Value);
}

fn compound<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Compound(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}
fn tile(pos: Pos, id: &str) -> HashMap<String, Value> {
    HashMap::from([
        ("x".into(), Value::Int(pos[0])),
        ("y".into(), Value::Int(pos[1])),
        ("z".into(), Value::Int(pos[2])),
        ("id".into(), Value::String(id.into())),
    ])
}
fn choose<T>(r: &mut JavaRandom, entries: &[T], weight: impl Fn(&T) -> i32) -> usize {
    let mut n = r.bounded(entries.iter().map(&weight).sum());
    entries
        .iter()
        .position(|e| {
            n -= weight(e);
            n < 0
        })
        .expect("positive weight sum")
}

pub(super) fn book(r: &mut JavaRandom) -> Value {
    r.bounded(1);
    r.bounded(1);
    let modifier = (r.float() + r.float() - 1.) * 0.15;
    let level = (31. * (1. + modifier) + 0.5) as usize;
    let mut available = tables::candidates(level).to_vec();
    let mut incompatible = 0u64;
    let mut selected = Vec::new();
    let mut threshold = level;
    loop {
        if !available.is_empty() {
            let (id, lvl, _) = available[choose(r, &available, |e| e.2)];
            selected.push(compound([
                ("id", Value::Short(id)),
                ("lvl", Value::Short(lvl)),
            ]));
            incompatible |= tables::incompatible(id);
        }
        if r.bounded(50) as usize > threshold {
            break;
        }
        available.retain(|&(id, _, _)| incompatible & (1u64 << id) == 0);
        threshold >>= 1;
    }
    compound([("StoredEnchantments", Value::List(selected))])
}

fn chest(r: &mut JavaRandom, pos: Pos) -> Value {
    let enchantments = book(r);
    let mut slots = BTreeMap::new();
    for _ in 0..8 {
        let (id, min, max, _) = tables::LOOT[choose(r, tables::LOOT, |e| e.3)];
        let count = min + r.bounded(max - min + 1);
        let slot = r.bounded(27) as i8;
        let mut item = HashMap::from([
            ("id".into(), Value::String(id.into())),
            ("Count".into(), Value::Byte(count as i8)),
            ("Damage".into(), Value::Short(0)),
            ("Slot".into(), Value::Byte(slot)),
        ]);
        if id == "minecraft:enchanted_book" {
            item.insert("tag".into(), enchantments.clone());
        }
        slots.insert(slot, Value::Compound(item));
    }
    let mut nbt = tile(pos, "Chest");
    nbt.insert("Lock".into(), Value::String(String::new()));
    nbt.insert("Items".into(), Value::List(slots.into_values().collect()));
    Value::Compound(nbt)
}
fn spawner(r: &mut JavaRandom, pos: Pos) -> Value {
    let entity = ["Skeleton", "Zombie", "Zombie", "Spider"][r.bounded(4) as usize];
    let mut nbt = tile(pos, "MobSpawner");
    nbt.insert("EntityId".into(), Value::String(entity.into()));
    for (key, v) in [
        ("Delay", 20),
        ("MinSpawnDelay", 200),
        ("MaxSpawnDelay", 800),
        ("SpawnCount", 4),
        ("MaxNearbyEntities", 6),
        ("RequiredPlayerRange", 16),
        ("SpawnRange", 4),
    ] {
        nbt.insert(key.into(), Value::Short(v));
    }
    Value::Compound(nbt)
}

pub(super) fn chest_facing(w: &impl DungeonWorld, p: Pos) -> u16 {
    let [x, y, z] = p;
    let neighbors = [[x, y, z - 1], [x, y, z + 1], [x - 1, y, z], [x + 1, y, z]];
    if neighbors.iter().any(|&n| w.state(n) >> 4 == 54) {
        return 2;
    }
    let full = neighbors.map(|n| w.full_block(n));
    if full.iter().filter(|&&f| f).count() == 1 {
        return [3, 2, 5, 4][full.iter().position(|&v| v).unwrap()];
    }
    // Default north, then opposite, clockwise and opposite as in BlockChest.
    let mut facing = 0usize;
    if full[facing] {
        facing ^= 1;
    }
    if full[facing] {
        facing = [3, 2, 0, 1][facing];
    }
    if full[facing] {
        facing ^= 1;
    }
    2 + facing as u16
}

pub fn generate_dungeon(w: &mut impl DungeonWorld, r: &mut JavaRandom, center: Pos) -> bool {
    let [cx, cy, cz] = center;
    let rx = r.bounded(2) + 2;
    let rz = r.bounded(2) + 2;
    let xx = -rx - 1..=rx + 1;
    let zz = -rz - 1..=rz + 1;
    let mut openings = 0;
    for x in xx.clone() {
        for y in -1..=4 {
            for z in zz.clone() {
                let p = [cx + x, cy + y, cz + z];
                if matches!(y, -1 | 4) && !w.solid(p) {
                    return false;
                }
                if y == 0
                    && (x.abs() == rx + 1 || z.abs() == rz + 1)
                    && w.state(p) == 0
                    && w.state([p[0], p[1] + 1, p[2]]) == 0
                {
                    openings += 1;
                }
            }
        }
    }
    if !(1..=5).contains(&openings) {
        return false;
    }
    for x in xx {
        for y in (-1..=3).rev() {
            for z in zz.clone() {
                let p = [cx + x, cy + y, cz + z];
                let wall = x.abs() == rx + 1 || z.abs() == rz + 1 || y == -1;
                if wall && p[1] >= 0 && !w.solid([p[0], p[1] - 1, p[2]]) {
                    w.set_state(p, 0);
                } else if w.state(p) >> 4 != 54 {
                    if !wall {
                        w.set_state(p, 0);
                    } else if w.solid(p) {
                        let block = if y == -1 && r.bounded(4) != 0 { 48 } else { 4 };
                        w.set_state(p, block << 4);
                    }
                }
            }
        }
    }
    for _ in 0..2 {
        for _ in 0..3 {
            let x = cx + r.bounded(2 * rx + 1) - rx;
            let z = cz + r.bounded(2 * rz + 1) - rz;
            let p = [x, cy, z];
            if w.state(p) != 0 {
                continue;
            }
            let walls = [
                [x - 1, cy, z],
                [x + 1, cy, z],
                [x, cy, z - 1],
                [x, cy, z + 1],
            ]
            .into_iter()
            .filter(|&n| w.solid(n))
            .count();
            if walls == 1 {
                w.set_state(p, (54 << 4) | chest_facing(w, p));
                w.set_tile(p, chest(r, p));
                break;
            }
        }
    }
    w.set_state(center, 52 << 4);
    w.set_tile(center, spawner(r, center));
    true
}

/// The first three DEFAULT populate phases after structure pieces. The
/// caller retains the stream for BiomeGenBase.decorate.
pub fn populate_features(
    world: &mut impl DungeonWorld,
    random: &mut JavaRandom,
    chunk: [i32; 2],
    biome: u8,
    village_generated: bool,
) {
    super::lakes::populate_lakes(world, random, chunk, biome, village_generated);
    let [x, z] = chunk.map(|c| c.wrapping_mul(16).wrapping_add(8));
    for _ in 0..8 {
        let pos = [
            x.wrapping_add(random.bounded(16)),
            random.bounded(256),
            z.wrapping_add(random.bounded(16)),
        ];
        generate_dungeon(world, random, pos);
    }
}

impl super::Overworld {
    /// Populate loaded Overworld columns through BiomeGenBase.decorate.
    /// Keep the generator state across chunks in their population order.
    /// The returned stream continues at the animal-spawning phase.
    pub fn populate<W: DungeonWorld + super::decoration::biomes::BiomeWorld>(
        &self,
        world: &mut W,
        chunk: [i32; 2],
        generators: &mut super::decoration::biomes::DecoratorState,
        structure_pass: impl FnOnce(&mut W, &mut JavaRandom) -> bool,
    ) -> JavaRandom {
        let (id, mut random) = self.populate_base(world, chunk, structure_pass);
        let biome = super::decoration::biomes::Biome::from_id(id)
            .expect("DEFAULT Overworld GenLayer returned an invalid biome");
        biome.decorate(generators, world, &mut random, chunk);
        random
    }

    /// Run the base populate stages against the supplied loaded neighbors.
    /// The structure pass consumes the same stream and reports village
    /// generation. Return the sampled biome and stream for its decorator.
    pub fn populate_base<W: DungeonWorld>(
        &self,
        world: &mut W,
        chunk: [i32; 2],
        structure_pass: impl FnOnce(&mut W, &mut JavaRandom) -> bool,
    ) -> (u8, JavaRandom) {
        let [cx, cz] = chunk;
        let [x, z] = chunk.map(|c| c.wrapping_mul(16).wrapping_add(16));
        let biome = world
            .loaded_biome(x, z)
            .unwrap_or_else(|| self.biomes.blocks(x, z, 1, 1)[0] as u8);
        let mut random = JavaRandom::new(super::population_seed(self.seed, cx, cz));
        let village = self.structures && structure_pass(world, &mut random);
        populate_features(world, &mut random, chunk, biome, village);
        (biome, random)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Room {
        blocks: BTreeMap<Pos, u16>,
        tiles: BTreeMap<Pos, Value>,
    }
    impl LakeWorld for Room {
        fn state(&self, p: Pos) -> u16 {
            *self
                .blocks
                .get(&p)
                .unwrap_or(&if p[0] <= -3 && p[2] == 0 && matches!(p[1], 32 | 33) {
                    0
                } else {
                    16
                })
        }
        fn set_state(&mut self, p: Pos, s: u16) {
            self.blocks.insert(p, s);
        }
        fn solid(&self, p: Pos) -> bool {
            !matches!(self.state(p) >> 4, 0 | 8..=11)
        }
        fn liquid(&self, p: Pos) -> bool {
            matches!(self.state(p) >> 4, 8..=11)
        }
        fn skylight(&self, _: Pos) -> u8 {
            0
        }
        fn mycelium_surface(&self, _: Pos) -> bool {
            false
        }
        fn can_freeze(&self, _: Pos) -> bool {
            false
        }
    }
    impl DungeonWorld for Room {
        fn full_block(&self, p: Pos) -> bool {
            !matches!(self.state(p) >> 4, 0 | 54 | 52)
        }
        fn set_tile(&mut self, p: Pos, tag: Value) {
            self.tiles.insert(p, tag);
        }
    }
    #[test]
    fn official_features_with_loaded_negative_neighbors() {
        loaded_negative_neighbors(false);
    }
    #[test]
    fn official_population_with_loaded_negative_neighbors() {
        loaded_negative_neighbors(true);
    }

    impl super::super::decoration::plants::PlantWorld for Room {
        fn full_block(&self, p: Pos) -> bool {
            // On the dedicated server spruce leaves support mushrooms.
            !matches!(self.state(p) >> 4, 0 | 8..=11 | 31 | 32 | 37..=40 | 52 | 54 | 78 | 81 | 83 | 106 | 111 | 175)
        }
        fn combined_light(&self, _: Pos) -> u8 {
            0
        }
    }
    impl super::super::decoration::DecorationWorld for Room {
        fn top_solid_or_liquid(&self, x: i32, z: i32) -> i32 {
            (0..256).rev().find(|&y| !matches!(self.state([x,y,z])>>4,0|8..=11|18|31|32|37..=40|78|83|106|111|161|175)).map_or(0,|y| y+1)
        }
    }
    impl super::super::decoration::biomes::BiomeWorld for Room {
        fn height(&self, x: i32, z: i32) -> i32 {
            (0..256)
                .rev()
                .find(|&y| self.state([x, y, z]) != 0)
                .map_or(0, |y| y + 1)
        }
        fn tick_spring(&mut self, _: Pos, _: u16, _: &mut JavaRandom) {}
    }
    fn loaded_negative_neighbors(decorate: bool) {
        // TerrainReference.DesertWorld with the vanilla surface and carvers,
        // Structures off, four preloaded chunks, sampled before and after
        // the decorator. The fixture supplies zero light, no freezing, and
        // no immediate fluid ticks in both the Java and Rust worlds.
        use crate::overworld::Overworld;
        let mut provider = Overworld::new(12345);
        provider.structures = false;
        let fixture: Value =
            fastnbt::from_bytes(include_bytes!("dungeons/dungeon-neighbors.nbt")).unwrap();
        let mut states = Vec::new();
        if decorate {
            use std::io::Read;
            flate2::read::GzDecoder::new(&include_bytes!("dungeons/population-neighbors.gz")[..])
                .read_to_end(&mut states)
                .unwrap();
            assert_eq!(states.len(), 4 * 16 * 16 * 256 * 2);
        }
        for reverse in [false, true] {
            let mut world = Room::default();
            for n in 0..4 {
                let i = if reverse { 3 - n } else { n };
                let (cx, cz) = (-2 + i / 2, -3 + i % 2);
                let col = provider.column(cx, cz);
                for y in 0..256 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let state = col
                                .sections
                                .get(&((y / 16) as u8))
                                .map_or(0, |s| s.state(((y % 16) * 256 + z * 16 + x) as usize));
                            world.blocks.insert([cx * 16 + x, y, cz * 16 + z], state);
                        }
                    }
                }
            }
            let expected = if decorate {
                let mut stream = provider.populate(
                    &mut world,
                    [-2, -3],
                    &mut Default::default(),
                    |_, _| unreachable!(),
                );
                assert_eq!(stream.long(), 1520609871099492130);
                0x5139c67ece8a517a
            } else {
                provider.populate_base(&mut world, [-2, -3], |_, _| unreachable!());
                0x548e7fb947eb6588
            };
            let mut hash = 0xcbf29ce484222325u64;
            let mut reference = states.chunks_exact(2);
            for cx in -2..=-1 {
                for cz in -3..=-2 {
                    for y in 0..256 {
                        for z in 0..16 {
                            for x in 0..16 {
                                let p = [cx * 16 + x, y, cz * 16 + z];
                                let state = world.state(p);
                                if decorate {
                                    assert_eq!(
                                        state.to_le_bytes(),
                                        reference.next().unwrap(),
                                        "{p:?}"
                                    );
                                }
                                hash = (hash ^ u64::from(state)).wrapping_mul(0x100000001b3);
                            }
                        }
                    }
                }
            }
            assert_eq!(hash, expected);
            assert_eq!(
                compound([(
                    "TileEntities",
                    Value::List(world.tiles.into_values().collect())
                )]),
                fixture
            );
        }
    }
    #[test]
    fn official_dungeon_blocks_and_tiles() {
        for (seed, hash, next, fixture) in [
            (
                0,
                0x1df0d107736814a4,
                -3608267860102554860,
                include_bytes!("dungeons/dungeon-0.nbt").as_slice(),
            ),
            (
                4,
                0x9ce0e339969352a1,
                8051490743109180124,
                include_bytes!("dungeons/dungeon-4.nbt").as_slice(),
            ),
        ] {
            let mut r = JavaRandom::new(seed);
            let mut w = Room::default();
            assert!(generate_dungeon(&mut w, &mut r, [0, 32, 0]));
            let mut h = 0xcbf29ce484222325u64;
            for x in -4..=4 {
                for z in -4..=4 {
                    for y in 31..=36 {
                        h = (h ^ u64::from(w.state([x, y, z]))).wrapping_mul(0x100000001b3);
                    }
                }
            }
            assert_eq!(h, hash);
            assert_eq!(r.long(), next);
            let expected: Value = fastnbt::from_bytes(fixture).unwrap();
            assert_eq!(
                compound([("TileEntities", Value::List(w.tiles.into_values().collect()))]),
                expected
            );
        }
    }
}
