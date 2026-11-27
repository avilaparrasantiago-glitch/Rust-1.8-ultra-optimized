use super::chests::LOOT;
use crate::anvil::structures::StructureFile;
use crate::overworld::{
    JavaRandom, Overworld, StructureGrid,
    decoration::DecorationWorld,
    dungeons::{DungeonWorld, book, chest_facing, fill_chest},
};
use fastnbt::{IntArray, Value};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    io,
};

type Pos = [i32; 3];
#[derive(Default, Serialize, Deserialize)]
pub(in crate::overworld) struct ScatteredChanges {
    pub starts: Vec<Vec<u8>>,
    pub witches: Vec<Pos>,
}
impl ScatteredChanges {
    pub fn apply(&self, file: &mut StructureFile) -> io::Result<()> {
        for bytes in &self.starts {
            file.insert(fastnbt::from_bytes(bytes).map_err(io::Error::other)?)?;
        }
        Ok(())
    }
}
fn layout(mut bytes: &[u8], dir: i32) -> &[u8] {
    for i in 0..4 {
        let n = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let (piece, rest) = bytes[4..].split_at(n);
        if i == dir {
            return piece;
        }
        bytes = rest;
    }
    &[]
}
fn tile(id: &str, [x, y, z]: Pos) -> HashMap<String, Value> {
    HashMap::from([
        ("id".into(), Value::String(id.into())),
        ("x".into(), Value::Int(x)),
        ("y".into(), Value::Int(y)),
        ("z".into(), Value::Int(z)),
    ])
}
fn dispenser(r: &mut JavaRandom, pos: Pos) -> Value {
    let mut slots = BTreeMap::new();
    for _ in 0..2 {
        r.bounded(30);
        let n = 2 + r.bounded(6);
        let slot = r.bounded(9) as i8;
        slots.insert(
            slot,
            Value::Compound(HashMap::from([
                ("id".into(), Value::String("minecraft:arrow".into())),
                ("Slot".into(), Value::Byte(slot)),
                ("Count".into(), Value::Byte(n as i8)),
                ("Damage".into(), Value::Short(0)),
            ])),
        );
    }
    let mut tag = tile("Trap", pos);
    tag.insert("Lock".into(), Value::String(String::new()));
    tag.insert("Items".into(), Value::List(slots.into_values().collect()));
    Value::Compound(tag)
}
pub(in crate::overworld) fn structure_mob(seed: i64, pos: Pos, elder: bool) -> io::Result<Value> {
    use flate2::read::GzDecoder;
    let [x, y, z] = pos;
    let template: &[u8] = if elder {
        include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-3/elder.nbt.gz")
    } else {
        include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-3/witch.nbt.gz")
    };
    let mut tag: HashMap<String, Value> =
        fastnbt::from_reader(GzDecoder::new(template)).map_err(io::Error::other)?;
    let mut rng = JavaRandom::new(
        seed ^ i64::from(x).wrapping_mul(341873128712)
            ^ i64::from(z).wrapping_mul(132897987541)
            ^ i64::from(y),
    );
    fn assign_uuid(fields: &mut HashMap<String, Value>, rng: &mut JavaRandom) {
        fields.insert(
            "UUIDMost".into(),
            Value::Long((rng.long() & !0xf000) | 0x4000),
        );
        fields.insert(
            "UUIDLeast".into(),
            Value::Long((rng.long() & 0x3fff_ffff_ffff_ffff) | i64::MIN),
        );
    }
    assign_uuid(&mut tag, &mut rng);
    tag.insert(
        "Pos".into(),
        Value::List(vec![
            Value::Double(f64::from(x) + 0.5),
            Value::Double(y.into()),
            Value::Double(f64::from(z) + 0.5),
        ]),
    );
    if let Some(Value::List(attributes)) = tag.get_mut("Attributes") {
        for attr in attributes {
            if let Value::Compound(attr) = attr
                && let Some(Value::List(mods)) = attr.get_mut("Modifiers")
            {
                for entry in mods {
                    if let Value::Compound(fields) = entry {
                        assign_uuid(fields, &mut rng);
                        let amount = loop {
                            let a = 2.0 * rng.double() - 1.0;
                            let b = 2.0 * rng.double() - 1.0;
                            let radius = a * a + b * b;
                            if radius > 0.0 && radius < 1.0 {
                                break 0.05 * a * (-2.0 * radius.ln() / radius).sqrt();
                            }
                        };
                        fields.insert("Amount".into(), Value::Double(amount));
                    }
                }
            }
        }
    }
    Ok(Value::Compound(tag))
}
impl Overworld {
    pub(in crate::overworld) fn scattered_pieces(
        &self,
        index: &StructureFile,
        (cx, cz): (i32, i32),
        world: &mut (impl DungeonWorld + DecorationWorld),
        r: &mut JavaRandom,
    ) -> io::Result<ScatteredChanges> {
        let mut changes = ScatteredChanges::default();
        let (left, front) = (cx * 16 + 8, cz * 16 + 8);
        for sx in cx..=cx + 1 {
            for sz in cz..=cz + 1 {
                if StructureGrid::ScatteredFeature.candidate(self.seed, sx, sz) != (sx, sz) {
                    continue;
                }
                let Some(start) = index.get(sx, sz) else {
                    continue;
                };
                let mut updated = start.clone();
                let Some(Value::List(children)) = updated.get_mut("Children") else {
                    continue;
                };
                let Some(Value::Compound(piece)) = children.first_mut() else {
                    continue;
                };
                let hut = match piece.get("id") {
                    Some(Value::String(id)) if id == "TeSH" => true,
                    Some(Value::String(id)) if id == "TeJP" => false,
                    _ => continue,
                };
                let Some(Value::IntArray(bb)) = piece.get("BB") else {
                    return Err(io::Error::other("missing scattered bounds"));
                };
                let mut bounds: [i32; 6] = bb[..].try_into().map_err(io::Error::other)?;
                let (x0, x1, z0, z1) = (
                    left.max(bounds[0]),
                    (left + 15).min(bounds[3]),
                    front.max(bounds[2]),
                    (front + 15).min(bounds[5]),
                );
                if x0 > x1 || z0 > z1 {
                    continue;
                }
                if piece.get("HPos") == Some(&Value::Int(-1)) {
                    let mut sum = 0;
                    for z in z0..=z1 {
                        for x in x0..=x1 {
                            sum += world.top_solid_or_liquid(x, z).max(64);
                        }
                    }
                    let h = sum / ((x1 - x0 + 1) * (z1 - z0 + 1));
                    bounds[4] += h - bounds[1];
                    bounds[1] = h;
                    piece.insert("HPos".into(), Value::Int(h));
                    piece.insert("BB".into(), Value::IntArray(IntArray::new(bounds.to_vec())));
                }
                let Some(&Value::Int(dir @ 0..=3)) = piece.get("O") else {
                    return Err(io::Error::other("invalid piece facing"));
                };
                let data: &[u8] = if hut {
                    include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-3/hut.bin")
                } else {
                    include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-3/jungle.bin")
                };
                let inside = |x: i32, z: i32| {
                    (left..=left + 15).contains(&x) && (front..=front + 15).contains(&z)
                };
                for &[dx, dy, dz, hi, lo] in layout(data, dir).as_chunks::<5>().0 {
                    let (x, y, z) = (
                        bounds[0] + i32::from(dx as i8),
                        bounds[1] + i32::from(dy as i8),
                        bounds[2] + i32::from(dz as i8),
                    );
                    let pos = [x, y, z];
                    let clipped = !inside(x, z);
                    match u16::from_be_bytes([hi, lo]) {
                        0xff00 => {
                            let block = if r.float() < 0.4 { 4 } else { 48 };
                            if !clipped {
                                world.set_state(pos, block << 4);
                            }
                        }
                        0xff01 if !clipped => {
                            for by in (2..=y).rev() {
                                let p = [x, by, z];
                                if !matches!(world.state(p) >> 4, 0 | 8..=11) {
                                    break;
                                }
                                world.set_state(p, 17 << 4);
                            }
                        }
                        op @ (0xff02 | 0xff03) => {
                            let flag = if op == 0xff02 {
                                "placedMainChest"
                            } else {
                                "placedHiddenChest"
                            };
                            if piece.get(flag) != Some(&Value::Byte(1)) {
                                let ench = book(r);
                                let rolls = 2 + r.bounded(5);
                                if !clipped && world.state(pos) >> 4 != 54 {
                                    world.set_state(pos, (54 << 4) | chest_facing(world, pos));
                                    world.set_tile(pos, fill_chest(r, pos, LOOT, ench, rolls));
                                    piece.insert(flag.into(), Value::Byte(1));
                                }
                            }
                        }
                        op @ 0xff04..=0xff23 if !clipped => {
                            let v = op - 0xff04;
                            let flag = if v < 16 { "placedTrap1" } else { "placedTrap2" };
                            if piece.get(flag) != Some(&Value::Byte(1))
                                && world.state(pos) >> 4 != 23
                            {
                                world.set_state(pos, (23 << 4) | (v % 16));
                                world.set_tile(pos, dispenser(r, pos));
                                piece.insert(flag.into(), Value::Byte(1));
                            }
                        }
                        state if state < 0xff00 && !clipped => {
                            world.set_state(pos, state);
                            if state >> 4 == 140 {
                                let mut tag = tile("FlowerPot", pos);
                                tag.insert(
                                    "Item".into(),
                                    Value::String("minecraft:red_mushroom".into()),
                                );
                                tag.insert("Data".into(), Value::Int(0));
                                world.set_tile(pos, Value::Compound(tag));
                            }
                        }
                        _ => {}
                    }
                }
                if hut && piece.get("Witch") != Some(&Value::Byte(1)) {
                    let (x, z) = match dir {
                        0 => (bounds[0] + 2, bounds[2] + 5),
                        1 => (bounds[3] - 5, bounds[2] + 2),
                        2 => (bounds[0] + 2, bounds[5] - 5),
                        _ => (bounds[0] + 5, bounds[2] + 2),
                    };
                    if inside(x, z) {
                        changes.witches.push([x, bounds[1] + 2, z]);
                        piece.insert("Witch".into(), Value::Byte(1));
                    }
                }
                if &updated != start {
                    changes
                        .starts
                        .push(fastnbt::to_bytes(&updated).map_err(io::Error::other)?);
                }
            }
        }
        Ok(changes)
    }
}
