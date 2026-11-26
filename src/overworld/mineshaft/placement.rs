use super::{Bounds, Corridor, CorridorFeature, JavaRandom, dry_boundary, intersects, place_room};
use crate::overworld::dungeons::DungeonWorld;
use fastnbt::Value;
use std::{collections::HashMap, io};

type Fields = HashMap<String, Value>;
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid mineshaft piece")
}
fn bounds(tag: Option<&Value>) -> io::Result<Bounds> {
    let Some(Value::IntArray(array)) = tag else {
        return Err(invalid());
    };
    array[..].try_into().map_err(|_| invalid())
}
fn number(tag: &Fields, key: &str) -> io::Result<i32> {
    match tag.get(key) {
        Some(Value::Int(number)) => Ok(*number),
        _ => Err(invalid()),
    }
}
fn flag(tag: &Fields, key: &str) -> bool {
    matches!(tag.get(key),Some(Value::Byte(flag)) if *flag!=0)
}

pub fn populate_start(
    start: &mut Fields,
    world: &mut impl DungeonWorld,
    owner: [i32; 2],
    rng: &mut JavaRandom,
    mut emit: impl FnMut(CorridorFeature),
) -> io::Result<()> {
    let [left, front] = owner.map(|coord| coord * 16 + 8);
    let window = [left, 0, front, left + 15, 255, front + 15];
    if !intersects(bounds(start.get("BB"))?, window) {
        return Ok(());
    }
    let Some(Value::List(children)) = start.get_mut("Children") else {
        return Err(invalid());
    };
    let mut index = 0;
    while index < children.len() {
        let Value::Compound(fields) = &mut children[index] else {
            return Err(invalid());
        };
        let bb = bounds(fields.get("BB"))?;
        if !intersects(bb, window) {
            index += 1;
            continue;
        }
        let placed = match fields.get("id") {
            Some(Value::String(id)) if id == "MSRoom" => {
                let Some(Value::List(entrances)) = fields.get("Entrances") else {
                    return Err(invalid());
                };
                let doors = entrances
                    .iter()
                    .map(|tag| bounds(Some(tag)))
                    .collect::<io::Result<Vec<_>>>()?;
                place_room(world, bb, &doors, owner)
            }
            Some(Value::String(id)) if id == "MSCorridor" => {
                let mut corridor = Corridor {
                    bounds: bb,
                    facing: number(fields, "O")?,
                    sections: number(fields, "Num")?,
                    rails: flag(fields, "hr"),
                    spiders: flag(fields, "sc"),
                    spawned: flag(fields, "hps"),
                };
                if !(0..4).contains(&corridor.facing) || !(1..=4).contains(&corridor.sections) {
                    return Err(invalid());
                }
                let placed = corridor.place(world, owner, rng, &mut emit);
                fields.insert("hps".into(), Value::Byte(corridor.spawned.into()));
                placed
            }
            Some(Value::String(id)) if id == "MSCrossing" || id == "MSStairs" => {
                if !dry_boundary(world, bb, owner) {
                    false
                } else {
                    let [west, floor, north, east, roof, south] = bb;
                    let mut fill = |area: Bounds, state| {
                        for y in area[1]..=area[4] {
                            for x in area[0].max(left)..=area[3].min(left + 15) {
                                for z in area[2].max(front)..=area[5].min(front + 15) {
                                    world.set_state([x, y, z], state);
                                }
                            }
                        }
                    };
                    if id == "MSCrossing" {
                        let floors = if flag(fields, "tf") { 2 } else { 1 };
                        for level in 0..floors {
                            let base = floor + 4 * level;
                            fill([west + 1, base, north, east - 1, base + 2, south], 0);
                            fill([west, base, north + 1, east, base + 2, south - 1], 0);
                        }
                        if floors == 2 {
                            fill(
                                [
                                    west + 1,
                                    floor + 3,
                                    north + 1,
                                    east - 1,
                                    floor + 3,
                                    south - 1,
                                ],
                                0,
                            );
                        }
                        for x in [west + 1, east - 1] {
                            for z in [north + 1, south - 1] {
                                fill([x, floor, z, x, roof, z], 5 << 4);
                            }
                        }
                        for x in west.max(left)..=east.min(left + 15) {
                            for z in north.max(front)..=south.min(front + 15) {
                                let pos = [x, floor - 1, z];
                                if world.state(pos) >> 4 == 0 {
                                    world.set_state(pos, 5 << 4);
                                }
                            }
                        }
                    } else {
                        let facing = number(fields, "O")?;
                        for offset in 0..9 {
                            let (low, high) = match offset {
                                0 | 1 => (5, 7),
                                7 | 8 => (0, 2),
                                _ => (7 - offset - i32::from(offset < 6), 9 - offset),
                            };
                            let area = match facing {
                                0 => [
                                    west,
                                    floor + low,
                                    north + offset,
                                    east,
                                    floor + high,
                                    north + offset,
                                ],
                                1 => [
                                    east - offset,
                                    floor + low,
                                    north,
                                    east - offset,
                                    floor + high,
                                    south,
                                ],
                                2 => [
                                    west,
                                    floor + low,
                                    south - offset,
                                    east,
                                    floor + high,
                                    south - offset,
                                ],
                                3 => [
                                    west + offset,
                                    floor + low,
                                    north,
                                    west + offset,
                                    floor + high,
                                    south,
                                ],
                                _ => return Err(invalid()),
                            };
                            fill(area, 0);
                        }
                    }
                    true
                }
            }
            _ => return Err(invalid()),
        };
        if placed {
            index += 1;
        } else {
            children.remove(index);
        }
    }
    Ok(())
}
