use super::{Bounds, Fields, JavaRandom, place_portal};
use crate::overworld::{
    dungeons::{DungeonWorld, chest_facing},
    mineshaft::dry_boundary,
    structure_fixtures::prepare_stronghold_chest,
};
use fastnbt::Value;
use std::{
    collections::HashMap,
    io::{self, Read},
    sync::OnceLock,
};

type Shapes = HashMap<u32, Vec<Vec<i16>>>;
fn take<const N: usize>(input: &mut &[u8]) -> [u8; N] {
    let mut bytes = [0; N];
    input.read_exact(&mut bytes).expect("bundled reference");
    bytes
}
fn shapes() -> &'static Shapes {
    static TABLE: OnceLock<Shapes> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-2/stronghold/shapes.gz")[..])
            .read_to_end(&mut bytes)
            .expect("bundled shapes");
        let mut reader = &bytes[..];
        let mut table = HashMap::new();
        while !reader.is_empty() {
            let key = u32::from_be_bytes(take(&mut reader));
            let count = u16::from_be_bytes(take(&mut reader));
            let mut ops = Vec::new();
            for _ in 0..count {
                let len = take::<1>(&mut reader)[0];
                ops.push(
                    (0..len)
                        .map(|_| i16::from_be_bytes(take(&mut reader)))
                        .collect(),
                );
            }
            table.insert(key, ops);
        }
        table
    })
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid stronghold piece")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::{Overworld, lakes::LakeWorld};
    #[derive(Default)]
    struct Stone {
        blocks: HashMap<[i32; 3], u16>,
        tiles: HashMap<String, Value>,
    }
    impl LakeWorld for Stone {
        fn state(&self, p: [i32; 3]) -> u16 {
            self.blocks.get(&p).copied().unwrap_or(16)
        }
        fn set_state(&mut self, p: [i32; 3], s: u16) {
            if self.state(p) >> 4 != s >> 4 {
                self.tiles.remove(&format!("{},{},{}", p[0], p[1], p[2]));
            }
            self.blocks.insert(p, s);
        }
        fn liquid(&self, p: [i32; 3]) -> bool {
            matches!(self.state(p) >> 4, 8..=11)
        }
        fn solid(&self, p: [i32; 3]) -> bool {
            self.full_block(p)
        }
        fn skylight(&self, _: [i32; 3]) -> u8 {
            0
        }
        fn can_freeze(&self, _: [i32; 3]) -> bool {
            false
        }
        fn mycelium_surface(&self, _: [i32; 3]) -> bool {
            false
        }
    }
    impl DungeonWorld for Stone {
        fn full_block(&self, p: [i32; 3]) -> bool {
            matches!(self.state(p) >> 4, 1 | 4 | 5 | 47 | 97 | 98)
        }
        fn set_tile(&mut self, [x, y, z]: [i32; 3], tag: Value) {
            self.tiles.insert(format!("{x},{y},{z}"), tag);
        }
    }
    #[test]
    fn official_entire_stronghold_with_inventories() {
        let mut start = Overworld::new(12345).stronghold_start_nbt(-44, 59).unwrap();
        let bb = bounds(&start).unwrap();
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-2/stronghold/whole.gz")[..])
            .read_to_end(&mut raw)
            .unwrap();
        let mut input = &raw[..];
        let mut world = Stone::default();
        for cx in ((bb[0] - 8) >> 4)..=((bb[3] - 8) >> 4) {
            for cz in ((bb[2] - 8) >> 4)..=((bb[5] - 8) >> 4) {
                let mut rng = JavaRandom::new(12345 + i64::from(cx * 31 + cz));
                populate_start(&mut start, &mut world, [cx, cz], &mut rng).unwrap();
                assert_eq!(
                    rng.bits(32),
                    u32::from_be_bytes(take(&mut input)),
                    "rng {cx},{cz}"
                );
            }
        }
        for y in bb[1] - 1..=bb[4] {
            for z in bb[2]..=bb[5] {
                for x in bb[0]..=bb[3] {
                    assert_eq!(
                        world.state([x, y, z]),
                        u16::from_be_bytes(take(&mut input)),
                        "block {x},{y},{z}"
                    );
                }
            }
        }
        let expected: HashMap<String, Value> = fastnbt::from_bytes(input).unwrap();
        assert!(expected.len() > 2);
        assert_eq!(world.tiles, expected);
    }
}
fn flag(fields: &Fields, key: &str) -> bool {
    matches!(fields.get(key),Some(Value::Byte(v)) if *v!=0)
}
fn number(fields: &Fields, key: &str) -> io::Result<i32> {
    match fields.get(key) {
        Some(Value::Int(v)) => Ok(*v),
        _ => Err(invalid()),
    }
}
pub(super) fn bounds(fields: &Fields) -> io::Result<Bounds> {
    let Some(Value::IntArray(bb)) = fields.get("BB") else {
        return Err(invalid());
    };
    bb[..].try_into().map_err(|_| invalid())
}
pub(crate) fn populate_start(
    start: &mut Fields,
    world: &mut impl DungeonWorld,
    owner: [i32; 2],
    rng: &mut JavaRandom,
) -> io::Result<()> {
    let [left, front] = owner.map(|v| v * 16 + 8);
    let Some(Value::List(children)) = start.get_mut("Children") else {
        return Err(invalid());
    };
    let mut index = 0;
    while index < children.len() {
        let Value::Compound(fields) = &mut children[index] else {
            return Err(invalid());
        };
        let bb = bounds(fields)?;
        if bb[0] > left + 15 || bb[3] < left || bb[2] > front + 15 || bb[5] < front {
            index += 1;
            continue;
        }
        let Some(Value::String(id)) = fields.get("id") else {
            return Err(invalid());
        };
        let facing = number(fields, "O")?;
        if !(0..4).contains(&facing) {
            return Err(invalid());
        }
        if id == "SHPR" {
            let mut spawned = flag(fields, "Mob");
            let mut tile = None;
            place_portal(
                bb,
                facing as usize,
                owner,
                rng,
                &mut spawned,
                |p, s| world.set_state(p, s),
                |p, t| tile = Some((p, t)),
            );
            if let Some((p, t)) = tile {
                world.set_tile(p, t);
            }
            fields.insert("Mob".into(), Value::Byte(spawned.into()));
            index += 1;
            continue;
        }
        let kind = match id.as_str() {
            "SHStart" | "SHSD" => 6,
            "SHFC" => 10,
            id => super::IDS[..10]
                .iter()
                .position(|&s| s == id)
                .ok_or_else(invalid)?,
        };
        if kind != 10 && !dry_boundary(world, bb, owner) {
            children.remove(index);
            continue;
        }
        let variant = match kind {
            0 => i32::from(flag(fields, "Left")) + 2 * i32::from(flag(fields, "Right")),
            4 => number(fields, "Type")?,
            7 => ["leftLow", "leftHigh", "rightLow", "rightHigh"]
                .into_iter()
                .enumerate()
                .fold(0, |mask, (i, k)| mask + (i32::from(flag(fields, k)) << i)),
            9 => i32::from(flag(fields, "Tall")),
            10 => number(fields, "Steps")? - 1,
            _ => 0,
        };
        let Some(Value::String(door)) = fields.get("EntryDoor") else {
            return Err(invalid());
        };
        let door = ["OPENING", "WOOD_DOOR", "GRATES", "IRON_DOOR"]
            .iter()
            .position(|v| v == door)
            .ok_or_else(invalid)?;
        let key = ((kind as u32) << 12)
            | ((facing as u32) << 10)
            | ((door as u32) << 8)
            | (variant as u32);
        let pos = |x: i16, y: i16, z: i16| {
            let (x, z) = (i32::from(x), i32::from(z));
            let (x, z) = match facing {
                0 => (bb[0] + x, bb[2] + z),
                1 => (bb[3] - z, bb[2] + x),
                2 => (bb[0] + x, bb[5] - z),
                _ => (bb[0] + z, bb[2] + x),
            };
            [x, bb[1] + i32::from(y), z]
        };
        let inside = |[x, _, z]: [i32; 3]| {
            (left..left + 16).contains(&x) && (front..front + 16).contains(&z)
        };
        for op in shapes().get(&key).ok_or_else(invalid)? {
            let origin = pos(op[1], op[2], op[3]);
            match op[0] {
                1 | 5 => {
                    if (op[0] == 1 || rng.float() < f32::from(op[5]) / 100.) && inside(origin) {
                        world.set_state(origin, op[4] as u16);
                    }
                }
                2..=4 => {
                    for y in op[2]..=op[5] {
                        for x in op[1]..=op[4] {
                            for z in op[3]..=op[6] {
                                let p = pos(x, y, z);
                                let allowed = inside(p);
                                if op[0] == 4 && rng.float() > f32::from(op[10]) / 100. {
                                    continue;
                                }
                                let existing = if op[0] == 3 { op[7] } else { op[9] };
                                if existing != 0 && (!allowed || world.state(p) >> 4 == 0) {
                                    continue;
                                }
                                let edge = x == op[1]
                                    || x == op[4]
                                    || y == op[2]
                                    || y == op[5]
                                    || z == op[3]
                                    || z == op[6];
                                let state = if op[0] != 3 {
                                    op[if edge { 7 } else { 8 }] as u16
                                } else if edge {
                                    let roll = rng.float();
                                    if roll < 0.2 {
                                        98 * 16 + 2
                                    } else if roll < 0.5 {
                                        98 * 16 + 1
                                    } else if roll < 0.55 {
                                        97 * 16 + 2
                                    } else {
                                        98 * 16
                                    }
                                } else {
                                    0
                                };
                                if allowed {
                                    world.set_state(p, state);
                                }
                            }
                        }
                    }
                }
                6 => {
                    if kind == 8 {
                        if flag(fields, "Chest") || !inside(origin) {
                            continue;
                        }
                        fields.insert("Chest".into(), Value::Byte(1));
                    }
                    let finish = prepare_stronghold_chest(kind, rng);
                    if inside(origin) && world.state(origin) >> 4 != 54 {
                        let facing = chest_facing(world, origin);
                        world.set_state(origin, 54 * 16 + facing);
                        world.set_tile(
                            origin,
                            Value::Compound(HashMap::from([
                                ("id".into(), Value::String("Chest".into())),
                                ("x".into(), Value::Int(origin[0])),
                                ("y".into(), Value::Int(origin[1])),
                                ("z".into(), Value::Int(origin[2])),
                                ("Lock".into(), Value::String(String::new())),
                                ("Items".into(), finish(rng)),
                            ])),
                        );
                    }
                }
                _ => return Err(invalid()),
            }
        }
        index += 1;
    }
    Ok(())
}
