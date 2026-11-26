use super::{JavaRandom, mineshaft_random};
use fastnbt::{IntArray, Value};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::super::lakes::LakeWorld;
    use super::*;
    use std::io::Read;

    #[derive(Default)]
    struct Stone(HashMap<[i32; 3], u16>);
    impl LakeWorld for Stone {
        fn state(&self, pos: [i32; 3]) -> u16 {
            *self.0.get(&pos).unwrap_or(&16)
        }
        fn set_state(&mut self, pos: [i32; 3], state: u16) {
            self.0.insert(pos, state);
        }
        fn liquid(&self, pos: [i32; 3]) -> bool {
            matches!(self.state(pos) >> 4, 8..=11)
        }
        fn solid(&self, _: [i32; 3]) -> bool {
            unreachable!()
        }
        fn skylight(&self, _: [i32; 3]) -> u8 {
            unreachable!()
        }
        fn mycelium_surface(&self, _: [i32; 3]) -> bool {
            unreachable!()
        }
        fn can_freeze(&self, _: [i32; 3]) -> bool {
            unreachable!()
        }
    }

    impl crate::overworld::dungeons::DungeonWorld for Stone {
        fn full_block(&self, pos: [i32; 3]) -> bool {
            matches!(self.state(pos) >> 4, 1 | 5)
        }
        fn set_tile(&mut self, pos: [i32; 3], nbt: Value) {
            assert_eq!(self.state(pos), 52 << 4);
            let Value::Compound(tag) = nbt else { panic!() };
            assert_eq!(tag["EntityId"], Value::String("CaveSpider".into()));
        }
    }

    #[test]
    fn mine_corridors_match_official_clipped_population() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-2/mine-corridor.gz")[..])
            .read_to_end(&mut bytes)
            .unwrap();
        let mut input = &bytes[..];
        let integer = |input: &mut &[u8]| {
            let mut buf = [0; 4];
            input.read_exact(&mut buf).unwrap();
            i32::from_be_bytes(buf)
        };
        let mut cart_count = 0;
        for facing in 0..4 {
            for variant in 0..3 {
                let (width, length) = if facing % 2 == 0 { (3, 20) } else { (20, 3) };
                let bb = [-9, 32, 503, -10 + width, 34, 502 + length];
                let mut corridor = Corridor {
                    bounds: bb,
                    facing,
                    sections: 4,
                    rails: variant == 1,
                    spiders: variant == 2,
                    spawned: false,
                };
                let mut world = Stone::default();
                let mut carts = Vec::new();
                for cx in ((bb[0] - 8) >> 4)..=((bb[3] - 8) >> 4) {
                    for cz in ((bb[2] - 8) >> 4)..=((bb[5] - 8) >> 4) {
                        let mut rng = JavaRandom::new(84 + i64::from(31 * cx + cz));
                        assert!(corridor.place(
                            &mut world,
                            [cx, cz],
                            &mut rng,
                            |CorridorFeature::Cart { pos, items }| {
                                carts.push(Value::Compound(HashMap::from([
                                    ("Pos".into(), Value::IntArray(IntArray::new(pos.to_vec()))),
                                    ("Items".into(), items),
                                ])));
                            }
                        ));
                        assert_eq!(
                            rng.bits(32) as i32,
                            integer(&mut input),
                            "rng {facing}/{variant} {cx},{cz}"
                        );
                    }
                }
                for y in 31..=34 {
                    for z in bb[2]..=bb[5] {
                        for x in bb[0]..=bb[3] {
                            let mut buf = [0; 2];
                            input.read_exact(&mut buf).unwrap();
                            assert_eq!(
                                world.state([x, y, z]),
                                u16::from_be_bytes(buf),
                                "{facing}/{variant} at {x},{y},{z}"
                            );
                        }
                    }
                }
                let size = integer(&mut input) as usize;
                let (entities, rest) = input.split_at(size);
                input = rest;
                let fields: HashMap<String, Value> = fastnbt::from_bytes(entities).unwrap();
                cart_count += carts.len();
                assert_eq!(fields["Carts"], Value::List(carts));
                assert_eq!(corridor.spawned, variant == 2);
            }
        }
        assert!(input.is_empty());
        assert!(cart_count > 0);
    }

    #[test]
    fn full_mine_matches_server_across_population_windows() {
        let mut reference = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-2/whole-mine.gz")[..])
            .read_to_end(&mut reference)
            .unwrap();
        let mut input = &reference[..];
        let mut start = start_nbt(12345, -31, -32).unwrap();
        let Value::IntArray(bb) = &start["BB"] else {
            panic!()
        };
        let bb: Bounds = bb[..].try_into().unwrap();
        let mut world = Stone::default();
        for cx in ((bb[0] - 8) >> 4)..=((bb[3] - 8) >> 4) {
            for cz in ((bb[2] - 8) >> 4)..=((bb[5] - 8) >> 4) {
                let mut rng = JavaRandom::new(12345 + i64::from(cx * 31 + cz));
                populate_start(&mut start, &mut world, [cx, cz], &mut rng, |_| {}).unwrap();
                let mut next = [0; 4];
                input.read_exact(&mut next).unwrap();
                assert_eq!(rng.bits(32), u32::from_be_bytes(next), "rng {cx},{cz}");
            }
        }
        for y in bb[1] - 1..=bb[4] {
            for z in bb[2]..=bb[5] {
                for x in bb[0]..=bb[3] {
                    let mut expected = [0; 2];
                    input.read_exact(&mut expected).unwrap();
                    assert_eq!(
                        world.state([x, y, z]),
                        u16::from_be_bytes(expected),
                        "{x},{y},{z}"
                    );
                }
            }
        }
        assert!(input.is_empty());
    }

    #[test]
    fn mine_room_matches_server_after_clipped_passes() {
        let start = start_nbt(12345, -31, -32).unwrap();
        let Value::List(children) = &start["Children"] else {
            panic!()
        };
        let Value::Compound(room) = &children[0] else {
            panic!()
        };
        let bounds = |value: &Value| -> Bounds {
            let Value::IntArray(bb) = value else { panic!() };
            bb[..].try_into().unwrap()
        };
        let bb = bounds(&room["BB"]);
        let Value::List(doors) = &room["Entrances"] else {
            panic!()
        };
        let doors = doors.iter().map(bounds).collect::<Vec<_>>();
        let mut world = Stone::default();
        for cx in ((bb[0] - 8) >> 4)..=((bb[3] - 8) >> 4) {
            for cz in ((bb[2] - 8) >> 4)..=((bb[5] - 8) >> 4) {
                assert!(place_room(&mut world, bb, &doors, [cx, cz]));
            }
        }
        let mut actual = Vec::new();
        for y in bb[1] - 1..=bb[4] + 1 {
            for z in bb[2] - 1..=bb[5] + 1 {
                for x in bb[0] - 1..=bb[3] + 1 {
                    actual.extend(world.state([x, y, z]).to_be_bytes());
                }
            }
        }
        let mut expected = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-2/mine-room.gz")[..])
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn verified_mine_starts_have_bounded_nonoverlapping_children() {
        assert!(start_nbt(12345, 0, 0).is_none());
        let mut crosses_region = false;
        for (cx, cz) in [
            (-31, -32),
            (-24, 4),
            (-22, -4),
            (-11, 30),
            (22, -4),
            (28, 29),
        ] {
            let start = start_nbt(12345, cx, cz).unwrap();
            let Value::List(children) = &start["Children"] else {
                panic!("expected pieces")
            };
            let Value::IntArray(overall) = &start["BB"] else {
                panic!("expected bounds")
            };
            let mut seen = Vec::new();
            for child in children {
                let Value::Compound(fields) = child else {
                    panic!("expected compound")
                };
                let Value::IntArray(bb) = &fields["BB"] else {
                    panic!("expected bounds")
                };
                let bb: Bounds = bb[..].try_into().unwrap();
                assert!(
                    (0..3)
                        .all(|axis| overall[axis] <= bb[axis] && bb[axis + 3] <= overall[axis + 3])
                );
                assert!(!seen.iter().any(|&old| intersects(old, bb)));
                seen.push(bb);
                crosses_region |= bb[0] >> 9 != bb[3] >> 9 || bb[2] >> 9 != bb[5] >> 9;
            }
            assert!(seen.len() > 1);
        }
        assert!(crosses_region);
    }
}

type Bounds = [i32; 6];
mod corridor;
mod placement;
pub use corridor::{Corridor, CorridorFeature};
pub use placement::populate_start;

pub fn place_room(
    world: &mut impl super::lakes::LakeWorld,
    room: Bounds,
    entrances: &[Bounds],
    owner: [i32; 2],
) -> bool {
    if !dry_boundary(world, room, owner) {
        return false;
    }
    let left = owner[0] * 16 + 8;
    let front = owner[1] * 16 + 8;
    place_room_in_window(world, room, entrances, left, front);
    true
}

pub(super) fn dry_boundary(world: &impl super::lakes::LakeWorld, room: Bounds, owner: [i32; 2]) -> bool {
    let [west, floor, north, east, roof, south] = room;
    let [left, front] = owner.map(|coord| coord * 16 + 8);
    let min = [
        (west - 1).max(left),
        (floor - 1).max(0),
        (north - 1).max(front),
    ];
    let max = [
        (east + 1).min(left + 15),
        (roof + 1).min(255),
        (south + 1).min(front + 15),
    ];
    for x in min[0]..=max[0] {
        for z in min[2]..=max[2] {
            if world.liquid([x, min[1], z]) || world.liquid([x, max[1], z]) {
                return false;
            }
        }
    }
    for y in min[1]..=max[1] {
        for x in min[0]..=max[0] {
            if world.liquid([x, y, min[2]]) || world.liquid([x, y, max[2]]) {
                return false;
            }
        }
        for z in min[2]..=max[2] {
            if world.liquid([min[0], y, z]) || world.liquid([max[0], y, z]) {
                return false;
            }
        }
    }
    true
}

fn place_room_in_window(
    world: &mut impl super::lakes::LakeWorld,
    room: Bounds,
    entrances: &[Bounds],
    left: i32,
    front: i32,
) {
    let [west, floor, north, east, roof, south] = room;
    let mut fill = |area: Bounds, state: u16, existing: bool, dome: bool| {
        let width = (area[3] - area[0] + 1) as f32;
        let depth = (area[5] - area[2] + 1) as f32;
        let height = (area[4] - area[1] + 1) as f32;
        for y in area[1].max(0)..=area[4].min(255) {
            for x in area[0].max(left)..=area[3].min(left + 15) {
                for z in area[2].max(front)..=area[5].min(front + 15) {
                    let pos = [x, y, z];
                    if existing && world.state(pos) >> 4 == 0 {
                        continue;
                    }
                    if dome {
                        let dx = (x as f32 - (area[0] as f32 + width / 2.0)) / (width * 0.5);
                        let dy = (y - area[1]) as f32 / height;
                        let dz = (z as f32 - (area[2] as f32 + depth / 2.0)) / (depth * 0.5);
                        if dx * dx + dy * dy + dz * dz > 1.05 {
                            continue;
                        }
                    }
                    world.set_state(pos, state);
                }
            }
        }
    };
    fill(
        [west, floor, north, east, floor, south],
        3 << 4,
        true,
        false,
    );
    fill(
        [west, floor + 1, north, east, (floor + 3).min(roof), south],
        0,
        false,
        false,
    );
    for &entrance in entrances {
        let mut opening = entrance;
        opening[1] = opening[4] - 2;
        fill(opening, 0, false, false);
    }
    fill([west, floor + 4, north, east, roof, south], 0, false, true);
}
#[derive(Clone, Copy)]
enum Kind {
    Room,
    Corridor {
        rails: bool,
        spiders: bool,
        sections: i32,
    },
    Crossing {
        tall: bool,
    },
    Stairs,
}
struct Piece {
    bb: Bounds,
    kind: Kind,
    depth: i32,
    facing: i32,
}
struct Mine {
    pieces: Vec<Piece>,
    entrances: Vec<Bounds>,
    origin: (i32, i32),
}

fn intersects(left: Bounds, right: Bounds) -> bool {
    (0..3).all(|axis| left[axis] <= right[axis + 3] && left[axis + 3] >= right[axis])
}

fn tunnel(x: i32, y: i32, z: i32, facing: i32, length: i32) -> Bounds {
    match facing {
        0 => [x, y, z, x + 2, y + 2, z + length - 1],
        1 => [x - length + 1, y, z, x, y + 2, z + 2],
        2 => [x, y, z - length + 1, x + 2, y + 2, z],
        _ => [x, y, z, x + length - 1, y + 2, z + 2],
    }
}

impl Mine {
    fn free(&self, bb: Bounds) -> bool {
        !self.pieces.iter().any(|piece| intersects(piece.bb, bb))
    }
    fn branch(
        &mut self,
        rng: &mut JavaRandom,
        x: i32,
        y: i32,
        z: i32,
        facing: i32,
        parent: i32,
    ) -> Option<Bounds> {
        if parent > 8 || (x - self.origin.0).abs() > 80 || (z - self.origin.1).abs() > 80 {
            return None;
        }
        let roll = rng.bounded(100);
        let (bb, kind) = if roll >= 80 {
            let mut bb = match facing {
                0 => [x - 1, y, z, x + 3, y + 2, z + 4],
                1 => [x - 4, y, z - 1, x, y + 2, z + 3],
                2 => [x - 1, y, z - 4, x + 3, y + 2, z],
                _ => [x, y, z - 1, x + 4, y + 2, z + 3],
            };
            let tall = rng.bounded(4) == 0;
            if tall {
                bb[4] += 4;
            }
            if !self.free(bb) {
                return None;
            }
            (bb, Kind::Crossing { tall })
        } else if roll >= 70 {
            let mut bb = tunnel(x, y, z, facing, 9);
            bb[1] -= 5;
            if !self.free(bb) {
                return None;
            }
            (bb, Kind::Stairs)
        } else {
            let sections = (1..=2 + rng.bounded(3))
                .rev()
                .find(|sections| self.free(tunnel(x, y, z, facing, 5 * sections)))?;
            let rails = rng.bounded(3) == 0;
            let spiders = !rails && rng.bounded(23) == 0;
            (
                tunnel(x, y, z, facing, 5 * sections),
                Kind::Corridor {
                    rails,
                    spiders,
                    sections,
                },
            )
        };
        let depth = parent + 1;
        self.pieces.push(Piece {
            bb,
            kind,
            depth,
            facing,
        });
        let [west, bottom, north, east, _, south] = bb;
        match kind {
            Kind::Crossing { tall } => {
                let exits = [
                    (west + 1, north - 1, 2),
                    (west - 1, north + 1, 1),
                    (east + 1, north + 1, 3),
                    (west + 1, south + 1, 0),
                ];
                let order: &[usize] = match facing {
                    2 => &[0, 1, 2],
                    0 => &[3, 1, 2],
                    1 => &[0, 3, 1],
                    _ => &[0, 3, 2],
                };
                for &index in order {
                    let (xx, zz, dir) = exits[index];
                    self.branch(rng, xx, bottom, zz, dir, depth);
                }
                if tall {
                    for index in [0, 1, 2, 3] {
                        if rng.bits(1) != 0 {
                            let (xx, zz, dir) = exits[index];
                            self.branch(rng, xx, bottom + 4, zz, dir, depth);
                        }
                    }
                }
            }
            Kind::Corridor { .. } | Kind::Stairs => {
                let corridor = matches!(kind, Kind::Corridor { .. });
                let turn = if corridor { rng.bounded(4) } else { 0 };
                let floor = if corridor {
                    bottom - 1 + rng.bounded(3)
                } else {
                    bottom
                };
                let (xx, zz, dir) = match (facing, turn) {
                    (2, 0 | 1) => (west, north - 1, 2),
                    (0, 0 | 1) => (west, south + 1, 0),
                    (1, 0 | 1) => (west - 1, north, 1),
                    (3, 0 | 1) => (east + 1, north, 3),
                    (2, 2) => (west - 1, north, 1),
                    (2, _) => (east + 1, north, 3),
                    (0, 2) => (west - 1, south - 3, 1),
                    (0, _) => (east + 1, south - 3, 3),
                    (1, 2) => (west, north - 1, 2),
                    (1, _) => (west, south + 1, 0),
                    (3, 2) => (east - 3, north - 1, 2),
                    _ => (east - 3, south + 1, 0),
                };
                self.branch(rng, xx, floor, zz, dir, depth);
                if corridor && depth < 8 {
                    let vertical = facing % 2 == 0;
                    let (first, last) = if vertical {
                        (north, south)
                    } else {
                        (west, east)
                    };
                    for along in ((first + 3)..=(last - 3)).step_by(5) {
                        let side = rng.bounded(5);
                        if side < 2 {
                            let (xx, zz, dir) = match (vertical, side) {
                                (true, 0) => (west - 1, along, 1),
                                (true, _) => (east + 1, along, 3),
                                (false, 0) => (along, north - 1, 2),
                                _ => (along, south + 1, 0),
                            };
                            self.branch(rng, xx, bottom, zz, dir, depth + 1);
                        }
                    }
                }
            }
            Kind::Room => unreachable!(),
        }
        Some(bb)
    }
}

fn bounds(bb: Bounds) -> Value {
    Value::IntArray(IntArray::new(bb.to_vec()))
}

pub fn start_nbt(seed: i64, cx: i32, cz: i32) -> Option<HashMap<String, Value>> {
    let mut rng = mineshaft_random(seed, cx, cz)?;
    let x = cx * 16 + 2;
    let z = cz * 16 + 2;
    let room = [
        x,
        50,
        z,
        x + 7 + rng.bounded(6),
        54 + rng.bounded(6),
        z + 7 + rng.bounded(6),
    ];
    let mut mine = Mine {
        pieces: vec![Piece {
            bb: room,
            kind: Kind::Room,
            depth: 0,
            facing: -1,
        }],
        entrances: Vec::new(),
        origin: (x, z),
    };
    for dir in [2, 0, 1, 3] {
        let width = if dir % 2 == 0 {
            room[3] - x + 1
        } else {
            room[5] - z + 1
        };
        let mut offset = 0;
        while offset < width {
            offset += rng.bounded(width);
            if offset + 3 > width {
                break;
            }
            let y = room[1] + 1 + rng.bounded((room[4] - room[1] - 3).max(1));
            let (xx, zz) = match dir {
                2 => (x + offset, z - 1),
                0 => (x + offset, room[5] + 1),
                1 => (x - 1, z + offset),
                _ => (room[3] + 1, z + offset),
            };
            if let Some(mut door) = mine.branch(&mut rng, xx, y, zz, dir, 0) {
                match dir {
                    2 => {
                        door[2] = z;
                        door[5] = z + 1
                    }
                    0 => {
                        door[2] = room[5] - 1;
                        door[5] = room[5]
                    }
                    1 => {
                        door[0] = x;
                        door[3] = x + 1
                    }
                    _ => {
                        door[0] = room[3] - 1;
                        door[3] = room[3]
                    }
                };
                mine.entrances.push(door);
            }
            offset += 4;
        }
    }
    let mut overall = room;
    for piece in &mine.pieces {
        for axis in 0..3 {
            overall[axis] = overall[axis].min(piece.bb[axis]);
            overall[axis + 3] = overall[axis + 3].max(piece.bb[axis + 3]);
        }
    }
    let mut top = overall[4] - overall[1] + 2;
    if top < 53 {
        top += rng.bounded(53 - top);
    }
    let dy = top - overall[4];
    let shift = |mut bb: Bounds| {
        bb[1] += dy;
        bb[4] += dy;
        bounds(bb)
    };
    let children = mine
        .pieces
        .into_iter()
        .map(|piece| {
            let mut fields = HashMap::from([
                ("BB".into(), shift(piece.bb)),
                ("GD".into(), Value::Int(piece.depth)),
                (
                    "O".into(),
                    Value::Int(if matches!(piece.kind, Kind::Crossing { .. }) {
                        -1
                    } else {
                        piece.facing
                    }),
                ),
            ]);
            let id = match piece.kind {
                Kind::Room => {
                    fields.insert(
                        "Entrances".into(),
                        Value::List(mine.entrances.iter().copied().map(shift).collect()),
                    );
                    "MSRoom"
                }
                Kind::Stairs => "MSStairs",
                Kind::Crossing { tall } => {
                    fields.insert("tf".into(), Value::Byte(tall.into()));
                    fields.insert("D".into(), Value::Int(piece.facing));
                    "MSCrossing"
                }
                Kind::Corridor {
                    rails,
                    spiders,
                    sections,
                } => {
                    for (key, flag) in [("hr", rails), ("sc", spiders), ("hps", false)] {
                        fields.insert(key.into(), Value::Byte(flag.into()));
                    }
                    fields.insert("Num".into(), Value::Int(sections));
                    "MSCorridor"
                }
            };
            fields.insert("id".into(), Value::String(id.into()));
            Value::Compound(fields)
        })
        .collect();
    Some(HashMap::from([
        ("id".into(), Value::String("Mineshaft".into())),
        ("ChunkX".into(), Value::Int(cx)),
        ("ChunkZ".into(), Value::Int(cz)),
        ("BB".into(), shift(overall)),
        ("Children".into(), Value::List(children)),
    ]))
}
