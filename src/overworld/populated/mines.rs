use super::{ColumnPos, DungeonWorld, JavaRandom, Pos, StructureFile};
use crate::overworld::mineshaft::{self, CorridorFeature};
use fastnbt::Value;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io};

#[derive(Default, Serialize, Deserialize)]
pub(super) struct MineDelta {
    pub(super) starts: Vec<Vec<u8>>,
    pub(super) carts: Vec<(Pos, Vec<u8>)>,
}
impl MineDelta {
    pub(super) fn update(&self, index: &mut StructureFile) -> io::Result<()> {
        for raw in &self.starts {
            index.insert(fastnbt::from_bytes(raw).map_err(io::Error::other)?)?;
        }
        Ok(())
    }
}

fn cart(seed: i64, pos: Pos, items: Value) -> Value {
    let [x, y, z] = pos;
    let packed = (i64::from(x as u32) << 32) | i64::from(z as u32);
    let mut uuid = JavaRandom::new(seed ^ packed.rotate_left(17) ^ i64::from(y));
    Value::Compound(HashMap::from([
        ("id".into(), Value::String("MinecartChest".into())),
        (
            "Pos".into(),
            Value::List(
                pos.map(|coord| Value::Double(f64::from(coord as f32 + 0.5)))
                    .to_vec(),
            ),
        ),
        ("Motion".into(), Value::List(vec![Value::Double(0.0); 3])),
        ("Rotation".into(), Value::List(vec![Value::Float(0.0); 2])),
        (
            "UUIDMost".into(),
            Value::Long((uuid.long() & !0xf000) | 0x4000),
        ),
        (
            "UUIDLeast".into(),
            Value::Long((uuid.long() & 0x3fff_ffff_ffff_ffff) | i64::MIN),
        ),
        ("Air".into(), Value::Short(300)),
        ("Fire".into(), Value::Short(-1)),
        ("FallDistance".into(), Value::Float(0.0)),
        ("OnGround".into(), Value::Byte(0)),
        ("Invulnerable".into(), Value::Byte(0)),
        ("Dimension".into(), Value::Int(0)),
        ("PortalCooldown".into(), Value::Int(0)),
        ("Items".into(), items),
    ]))
}

pub(super) fn place(
    index: &StructureFile,
    seed: i64,
    owner: ColumnPos,
    world: &mut impl DungeonWorld,
    rng: &mut JavaRandom,
) -> io::Result<MineDelta> {
    let mut delta = MineDelta::default();
    let [left, front] = [owner.0, owner.1].map(|coord| coord * 16 + 8);
    for cx in owner.0 - 8..=owner.0 + 8 {
        for cz in owner.1 - 8..=owner.1 + 8 {
            let loaded = index.get(cx, cz);
            let temporary;
            let start = if let Some(start) = loaded {
                start
            } else {
                temporary = mineshaft::start_nbt(seed, cx, cz);
                let Some(start) = temporary.as_ref() else {
                    continue;
                };
                start
            };
            let Some(Value::IntArray(bb)) = start.get("BB") else {
                return Err(io::Error::other("missing mine bounds"));
            };
            if bb.len() != 6 {
                return Err(io::Error::other("invalid mine bounds"));
            }
            if bb[0] > left + 15 || bb[3] < left || bb[2] > front + 15 || bb[5] < front {
                continue;
            }
            let mut next = start.clone();
            let mut carts = Vec::new();
            mineshaft::populate_start(
                &mut next,
                world,
                [owner.0, owner.1],
                rng,
                |CorridorFeature::Cart { pos, items }| {
                    carts.push((pos, cart(seed, pos, items)));
                },
            )?;
            for (pos, nbt) in carts {
                delta
                    .carts
                    .push((pos, fastnbt::to_bytes(&nbt).map_err(io::Error::other)?));
            }
            if loaded.is_none() || next != *start {
                delta
                    .starts
                    .push(fastnbt::to_bytes(&next).map_err(io::Error::other)?);
            }
        }
    }
    Ok(delta)
}
