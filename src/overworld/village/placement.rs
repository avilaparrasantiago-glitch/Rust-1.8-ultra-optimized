use super::*;
use crate::anvil::structures::StructureFile;
use crate::overworld::{
    decoration::DecorationWorld,
    dungeons::{DungeonWorld, fill_chest},
};
use serde::{Deserialize, Serialize};
use std::{io, sync::LazyLock};

type Pos = [i32; 3];
type Key = [u8; 4];
type Templates = HashMap<Key, &'static [u8]>;
static BUILDINGS: LazyLock<Templates> = LazyLock::new(|| {
    let mut bytes =
        include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-4/village-buildings.bin").as_slice();
    let mut out = HashMap::new();
    while !bytes.is_empty() {
        let key = [bytes[0], bytes[1], bytes[2], bytes[3]];
        let len = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
        out.insert(key, &bytes[8..8 + len]);
        bytes = &bytes[8 + len..];
    }
    out
});
const LOOT: &[(&str, i32, i32, i32)] = &[
    ("minecraft:diamond", 1, 3, 3),
    ("minecraft:iron_ingot", 1, 5, 10),
    ("minecraft:gold_ingot", 1, 3, 5),
    ("minecraft:bread", 1, 3, 15),
    ("minecraft:apple", 1, 3, 15),
    ("minecraft:iron_pickaxe", 1, 1, 5),
    ("minecraft:iron_sword", 1, 1, 5),
    ("minecraft:iron_chestplate", 1, 1, 5),
    ("minecraft:iron_helmet", 1, 1, 5),
    ("minecraft:iron_leggings", 1, 1, 5),
    ("minecraft:iron_boots", 1, 1, 5),
    ("minecraft:obsidian", 3, 7, 5),
    ("minecraft:sapling", 3, 7, 5),
    ("minecraft:saddle", 1, 1, 3),
    ("minecraft:iron_horse_armor", 1, 1, 1),
    ("minecraft:golden_horse_armor", 1, 1, 1),
    ("minecraft:diamond_horse_armor", 1, 1, 1),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::lakes::LakeWorld;
    use std::io::Read;
    #[derive(Default)]
    struct Flat(HashMap<Pos, u16>);
    impl LakeWorld for Flat {
        fn state(&self, p: Pos) -> u16 {
            self.0
                .get(&p)
                .copied()
                .unwrap_or(if p[1] < 64 { 16 } else { 0 })
        }
        fn set_state(&mut self, p: Pos, s: u16) {
            self.0.insert(p, s);
        }
        fn liquid(&self, p: Pos) -> bool {
            matches!(self.state(p) >> 4, 8..=11)
        }
        fn solid(&self, p: Pos) -> bool {
            !matches!(self.state(p) >> 4, 0 | 8..=11 | 50 | 59 | 141 | 142)
        }
        fn can_freeze(&self, _: Pos) -> bool {
            false
        }
        fn mycelium_surface(&self, _: Pos) -> bool {
            false
        }
        fn skylight(&self, _: Pos) -> u8 {
            15
        }
    }
    impl DecorationWorld for Flat {
        fn top_solid_or_liquid(&self, x: i32, z: i32) -> i32 {
            (0..256)
                .rev()
                .find(|&y| self.solid([x, y, z]))
                .unwrap_or(-1)
                + 1
        }
    }
    impl DungeonWorld for Flat {
        fn loaded_biome(&self, _: i32, _: i32) -> Option<u8> {
            Some(1)
        }
        fn full_block(&self, p: Pos) -> bool {
            self.solid(p)
        }
        fn set_tile(&mut self, _: Pos, _: Value) {}
    }
    #[test]
    fn plains_group_matches_the_official_spawn_pass() {
        let mut lines =
            include_str!("../../../FONTS EXTRAS/vectors/T2.4b-4/plains-group.txt").lines();
        let (seed, next) = lines.next().unwrap().split_once(' ').unwrap();
        let mut random = JavaRandom::new(seed.parse().unwrap());
        let mut delta = PopulationDelta::default();
        delta
            .spawn_animals(&Flat::default(), 12345, (0, 0), &mut random)
            .unwrap();
        let actual: Vec<_> = delta
            .mobs
            .iter()
            .map(|([x, y, z], bytes)| {
                let tag: Fields = fastnbt::from_bytes(bytes).unwrap();
                let Value::String(id) = &tag["id"] else {
                    panic!("entity id")
                };
                let Value::List(rot) = &tag["Rotation"] else {
                    panic!("rotation")
                };
                let Value::Float(yaw) = rot[0] else {
                    panic!("yaw")
                };
                format!("{id} {x} {y} {z} {}", yaw.to_bits())
            })
            .collect();
        assert_eq!(actual, lines.collect::<Vec<_>>());
        assert_eq!(random.long(), next.parse::<i64>().unwrap());
    }
    #[test]
    fn complete_villages_match_official_placement() {
        for (seed, cx, cz, reference) in [
            (
                12345,
                -128,
                118,
                &include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-4/placed-12345.bin.gz")[..],
            ),
            (
                -9581,
                -127,
                114,
                include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-4/placed--9581.bin.gz"),
            ),
        ] {
            let provider = Overworld::new(seed);
            let mut start = provider.village_start_nbt(cx, cz).unwrap();
            let b = bb(&start).unwrap();
            let mut world = Flat::default();
            let mut delta = PopulationDelta::default();
            let mut rng = JavaRandom::new(1234);
            let Some(Value::List(pieces)) = start.get_mut("Children") else {
                panic!("missing village pieces")
            };
            for x in (b[0] - 8) >> 4..=(b[3] - 8) >> 4 {
                for z in (b[2] - 8) >> 4..=(b[5] - 8) >> 4 {
                    let pass = Window {
                        x: x * 16 + 8,
                        z: z * 16 + 8,
                    };
                    for p in pieces.iter_mut() {
                        let Value::Compound(p) = p else {
                            panic!("invalid village")
                        };
                        pass.place(p, &mut world, &mut rng, &mut delta, seed)
                            .unwrap();
                    }
                }
            }
            let mut input = flate2::read::GzDecoder::new(reference);
            let mut continuation = [0; 8];
            input.read_exact(&mut continuation).unwrap();
            let mut diffs = std::collections::BTreeMap::new();
            for x in b[0] - 2..=b[3] + 2 {
                for z in b[2] - 2..=b[5] + 2 {
                    for y in 48..=80 {
                        let mut bytes = [0; 2];
                        input.read_exact(&mut bytes).unwrap();
                        let expected = u16::from_be_bytes(bytes);
                        let got = world.state([x, y, z]);
                        if got != expected {
                            *diffs.entry((got, expected)).or_insert(0) += 1;
                        }
                    }
                }
            }
            assert!(
                diffs.is_empty(),
                "seed {seed}: (Rust, official) and counts: {diffs:?}"
            );
            assert_eq!(rng.long(), i64::from_be_bytes(continuation));
        }
    }
    #[test]
    fn blacksmith_inventory_matches_official_chest() {
        let mut r = JavaRandom::new(12345);
        let rolls = 3 + r.bounded(6);
        let actual = fill_chest(
            &mut r,
            [-17, 65, 23],
            LOOT,
            Value::Compound(Fields::new()),
            rolls,
        );
        let expected: Value = fastnbt::from_reader(flate2::read::GzDecoder::new(
            &include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-4/blacksmith-chest.nbt.gz")[..],
        ))
        .unwrap();
        assert_eq!(actual, expected);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub(in crate::overworld) struct PopulationDelta {
    pub starts: Vec<Vec<u8>>,
    pub mobs: Vec<(Pos, Vec<u8>)>,
    #[serde(skip)]
    pub generated: bool,
}
impl PopulationDelta {
    pub fn spawn_animals(
        &mut self,
        world: &(impl DungeonWorld + DecorationWorld),
        seed: i64,
        (cx, cz): (i32, i32),
        rng: &mut JavaRandom,
    ) -> io::Result<()> {
        struct Entry {
            id: &'static str,
            weight: i32,
            min: i32,
            max: i32,
        }
        static TABLE: LazyLock<[Vec<Entry>; 256]> = LazyLock::new(|| {
            let mut table: [Vec<Entry>; 256] = std::array::from_fn(|_| vec![]);
            for row in
                include_str!("../../../FONTS EXTRAS/vectors/T2.4b-4/passive-spawns.tsv").lines()
            {
                let mut f = row.split_whitespace();
                let biome: usize = f.next().unwrap().parse().unwrap();
                let id = f.next().unwrap();
                let [weight, min, max] =
                    std::array::from_fn(|_| f.next().unwrap().parse().unwrap());
                table[biome].push(Entry {
                    id,
                    weight,
                    min,
                    max,
                });
            }
            table
        });
        let (x0, z0) = (cx * 16 + 8, cz * 16 + 8);
        let biome = world
            .loaded_biome(x0 + 8, z0 + 8)
            .ok_or_else(|| io::Error::other("missing animal spawn biome"))?;
        let pool = &TABLE[biome as usize];
        if pool.is_empty() {
            return Ok(());
        }
        // Vanilla uses World.rand for species and the population stream for group placement.
        let mut species =
            JavaRandom::new(super::super::population_seed(seed, cx, cz) ^ 0x5deece66d);
        let mut individuals = JavaRandom::new(species.long());
        while rng.float() < 0.1 {
            let mut ticket = species.bounded(pool.iter().map(|e| e.weight).sum());
            let Some(entry) = pool.iter().find(|e| {
                ticket -= e.weight;
                ticket < 0
            }) else {
                return Err(io::Error::other("invalid spawn weights"));
            };
            let count = entry.min + rng.bounded(entry.max - entry.min + 1);
            let (start_x, start_z) = (x0 + rng.bounded(16), z0 + rng.bounded(16));
            let (mut x, mut z) = (start_x, start_z);
            for _ in 0..count {
                for _ in 0..4 {
                    let y = world.top_solid_or_liquid(x, z);
                    let ground = [x, y - 1, z];
                    let pos = [x, y, z];
                    let valid = (1..255).contains(&y)
                        && world.full_block(ground)
                        && !matches!(world.state(ground) >> 4, 7 | 166)
                        && !world.full_block(pos)
                        && !world.liquid(pos)
                        && !world.full_block([x, y + 1, z]);
                    if valid {
                        self.mob(entry.id, pos, None, &mut individuals, rng.float() * 360.0)?;
                    }
                    x += rng.bounded(5) - rng.bounded(5);
                    z += rng.bounded(5) - rng.bounded(5);
                    while !(x0..x0 + 16).contains(&x) || !(z0..z0 + 16).contains(&z) {
                        x = start_x + rng.bounded(5) - rng.bounded(5);
                        z = start_z + rng.bounded(5) - rng.bounded(5);
                    }
                    if valid {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn apply(&self, index: &mut StructureFile) -> io::Result<()> {
        for bytes in &self.starts {
            index.insert(fastnbt::from_bytes(bytes).map_err(io::Error::other)?)?;
        }
        Ok(())
    }
    pub fn mob(
        &mut self,
        id: &str,
        pos: Pos,
        profession: Option<i32>,
        rng: &mut JavaRandom,
        yaw: f32,
    ) -> io::Result<()> {
        let [x, y, z] = pos;
        let hp = match id {
            "Chicken" => 4.0,
            "Sheep" => 8.0,
            "Rabbit" => 3.0,
            "Villager" => 20.0,
            _ => 10.0,
        };
        let mut fields = Fields::from([
            ("id".into(), Value::String(id.into())),
            (
                "Pos".into(),
                Value::List(vec![
                    Value::Double(x as f64 + 0.5),
                    Value::Double(y as f64),
                    Value::Double(z as f64 + 0.5),
                ]),
            ),
            ("Motion".into(), Value::List(vec![Value::Double(0.0); 3])),
            (
                "Rotation".into(),
                Value::List(vec![Value::Float(yaw), Value::Float(0.0)]),
            ),
            ("Health".into(), Value::Short(hp as i16)),
            ("HealF".into(), Value::Float(hp)),
            ("Age".into(), Value::Int(0)),
            ("Air".into(), Value::Short(300)),
            ("Fire".into(), Value::Short(-1)),
            (
                "UUIDMost".into(),
                Value::Long((rng.long() & !0xf000) | 0x4000),
            ),
            (
                "UUIDLeast".into(),
                Value::Long((rng.long() & 0x3fff_ffff_ffff_ffff) | i64::MIN),
            ),
        ]);
        if let Some(p) = profession {
            fields.insert("Profession".into(), Value::Int(p));
        }
        self.mobs
            .push((pos, fastnbt::to_bytes(&fields).map_err(io::Error::other)?));
        Ok(())
    }
}
fn int(tag: &Fields, key: &str) -> io::Result<i32> {
    match tag.get(key) {
        Some(Value::Int(v)) => Ok(*v),
        _ => Err(io::Error::other(format!("missing village {key}"))),
    }
}
fn bb(tag: &Fields) -> io::Result<Box3> {
    match tag.get("BB") {
        Some(Value::IntArray(a)) => a[..]
            .try_into()
            .map_err(|_| io::Error::other("bad village bounds")),
        _ => Err(io::Error::other("missing village bounds")),
    }
}
struct Window {
    x: i32,
    z: i32,
}
impl Window {
    fn contains(&self, x: i32, z: i32) -> bool {
        (self.x..self.x + 16).contains(&x) && (self.z..self.z + 16).contains(&z)
    }
    fn overlaps(&self, b: Box3) -> bool {
        self.x <= b[3] && self.z <= b[5] && self.x + 15 >= b[0] && self.z + 15 >= b[2]
    }
    fn place(
        &self,
        p: &mut Fields,
        world: &mut (impl DungeonWorld + DecorationWorld),
        rng: &mut JavaRandom,
        delta: &mut PopulationDelta,
        seed: i64,
    ) -> io::Result<()> {
        let mut b = bb(p)?;
        if !self.overlaps(b) {
            return Ok(());
        }
        let Some(Value::String(id)) = p.get("id") else {
            return Err(io::Error::other("missing village piece id"));
        };
        let Some(kind) = IDS.iter().position(|s| s == id) else {
            return Err(io::Error::other("unknown village piece"));
        };
        let desert = p.get("Desert") == Some(&Value::Byte(1));
        if kind == ROAD {
            for x in b[0].max(self.x)..=b[3].min(self.x + 15) {
                for z in b[2].max(self.z)..=b[5].min(self.z + 15) {
                    let y = world.top_solid_or_liquid(x, z) - 1;
                    world.set_state([x, y, z], if desert { 24 << 4 } else { 13 << 4 });
                    world.set_state([x, y - 1, z], if desert { 24 << 4 } else { 4 << 4 });
                }
            }
            return Ok(());
        }
        if int(p, "HPos")? < 0 {
            let mut sum = 0;
            let mut count = 0;
            for z in b[2].max(self.z)..=b[5].min(self.z + 15) {
                for x in b[0].max(self.x)..=b[3].min(self.x + 15) {
                    sum += world.top_solid_or_liquid(x, z).max(64);
                    count += 1;
                }
            }
            let ground = sum / count;
            let offset = if kind == WELL {
                ground - b[4] + 3
            } else {
                ground - b[1]
            };
            b[1] += offset;
            b[4] += offset;
            p.insert("HPos".into(), Value::Int(ground));
            p.insert("BB".into(), bounds(b));
        }
        let variant = match kind {
            0 => u8::from(p.get("Terrace") == Some(&Value::Byte(1))),
            3 => int(p, "T")? as u8 + 3 * u8::from(p.get("C") == Some(&Value::Byte(1))),
            _ => 0,
        };
        let key = [
            kind.min(10) as u8,
            desert as u8,
            int(p, "O")? as u8,
            variant,
        ];
        let shape = BUILDINGS
            .get(&key)
            .ok_or_else(|| io::Error::other("invalid village variant"))?;
        for op in shape.as_chunks::<6>().0 {
            let pos = [
                b[0] + op[0] as i8 as i32,
                b[1] + op[1] as i8 as i32,
                b[2] + op[2] as i8 as i32,
            ];
            let [x, y, z] = pos;
            let mut state = u16::from_be_bytes([op[4], op[5]]);
            if op[3] == 5 {
                let crop = ["CA", "CB", "CC", "CD"]
                    .get(state as usize)
                    .ok_or_else(|| io::Error::other("invalid village crop"))?;
                state = ((int(p, crop)? as u16) << 4) | (2 + rng.bounded(6) as u16);
            }
            if !self.contains(x, z) {
                continue;
            }
            match op[3] {
                0 | 5 => world.set_state(pos, state),
                1 => {
                    for h in y..255 {
                        if world.state([x, h, z]) == 0 {
                            break;
                        }
                        world.set_state([x, h, z], 0);
                    }
                }
                2 => {
                    for h in (2..=y).rev() {
                        let dst = [x, h, z];
                        if world.state(dst) != 0 && !world.liquid(dst) {
                            break;
                        }
                        world.set_state(dst, state);
                    }
                }
                3 if world.state(pos) == 0 && world.state([x, y - 1, z]) != 0 => {
                    world.set_state(pos, state)
                }
                4 => {
                    let (dx, dz) = [(0, 1), (-1, 0), (0, -1), (1, 0)][state as usize];
                    let side = |sign: i32| {
                        let blocks =
                            [y, y + 1].map(|h| world.state([x + sign * dx, h, z + sign * dz]) >> 4);
                        (
                            blocks
                                .iter()
                                .filter(|&&id| {
                                    super::super::climate::block_properties(id)[2] & 2 != 0
                                })
                                .count(),
                            blocks.contains(&64),
                        )
                    };
                    let (right, door_right) = side(1);
                    let (left, door_left) = side(-1);
                    let hinge = u16::from((door_left && !door_right) || right > left);
                    world.set_state(pos, 64 << 4 | state);
                    world.set_state([x, y + 1, z], 64 << 4 | 8 | hinge);
                }
                6 if p.get("Chest") != Some(&Value::Byte(1)) => {
                    let rolls = 3 + rng.bounded(6);
                    p.insert("Chest".into(), Value::Byte(1));
                    if world.state(pos) >> 4 != 54 {
                        world.set_state(pos, 54 << 4 | 2);
                        world.set_tile(
                            pos,
                            fill_chest(rng, pos, LOOT, Value::Compound(Fields::new()), rolls),
                        );
                    }
                }
                7 if int(p, "VCount")? == i32::from(state >> 8) => {
                    let mut individual = JavaRandom::new(
                        seed ^ i64::from(x).wrapping_mul(341873128712)
                            ^ i64::from(z).wrapping_mul(132897987541)
                            ^ i64::from(y),
                    );
                    let job = if state & 255 == 5 {
                        individual.bounded(5)
                    } else {
                        i32::from(state & 255)
                    };
                    delta.mob("Villager", pos, Some(job), &mut individual, 0.0)?;
                    p.insert("VCount".into(), Value::Int(i32::from(state >> 8) + 1));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
pub(in crate::overworld) fn populate_villages(
    provider: &Overworld,
    index: &StructureFile,
    (cx, cz): (i32, i32),
    world: &mut (impl DungeonWorld + DecorationWorld),
    rng: &mut JavaRandom,
) -> io::Result<PopulationDelta> {
    let mut delta = PopulationDelta::default();
    let window = Window {
        x: cx * 16 + 8,
        z: cz * 16 + 8,
    };
    for rx in (cx - 8).div_euclid(32)..=(cx + 8).div_euclid(32) {
        for rz in (cz - 8).div_euclid(32)..=(cz + 8).div_euclid(32) {
            let (sx, sz) = StructureGrid::Village.candidate(provider.seed, rx * 32, rz * 32);
            if (sx - cx).abs() > 8 || (sz - cz).abs() > 8 {
                continue;
            }
            let old = index.get(sx, sz);
            let Some(mut start) = old.cloned().or_else(|| provider.village_start_nbt(sx, sz))
            else {
                continue;
            };
            if start.get("Valid") == Some(&Value::Byte(1)) && window.overlaps(bb(&start)?) {
                delta.generated = true;
                let Some(Value::List(children)) = start.get_mut("Children") else {
                    return Err(io::Error::other("missing village children"));
                };
                for child in children {
                    let Value::Compound(p) = child else {
                        return Err(io::Error::other("invalid village child"));
                    };
                    window.place(p, world, rng, &mut delta, provider.seed)?;
                }
            }
            if old != Some(&start) {
                delta
                    .starts
                    .push(fastnbt::to_bytes(&start).map_err(io::Error::other)?);
            }
        }
    }
    Ok(delta)
}
