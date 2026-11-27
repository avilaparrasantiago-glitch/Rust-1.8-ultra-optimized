use super::{JavaRandom, Overworld};
use fastnbt::{IntArray, Value};
use std::collections::HashMap;
mod portal;
mod placement;
pub(super) use placement::populate_start;
pub use portal::place_portal;

type Bounds = [i32; 6];
type Fields = HashMap<String, Value>;
const START: usize = 11;
const FILLER: usize = 12;
const CROSS: usize = 7;
const PORTAL: usize = 10;
const IDS: [&str; 13] = [
    "SHS", "SHPH", "SHLT", "SHRT", "SHRC", "SHSSD", "SHSD", "SH5C", "SHCC", "SHLi", "SHPR",
    "SHStart", "SHFC",
];
const WEIGHT: [i32; 11] = [40, 5, 20, 20, 10, 5, 5, 5, 5, 10, 20];
const CAP: [i32; 11] = [0, 5, 0, 0, 6, 5, 5, 4, 4, 2, 1];
const SHAPE: [[i32; 6]; 11] = [
    [-1, -1, 0, 5, 5, 7],
    [-1, -1, 0, 9, 5, 11],
    [-1, -1, 0, 5, 5, 5],
    [-1, -1, 0, 5, 5, 5],
    [-4, -1, 0, 11, 7, 11],
    [-1, -7, 0, 5, 11, 8],
    [-1, -7, 0, 5, 11, 5],
    [-4, -3, 0, 10, 9, 11],
    [-1, -1, 0, 5, 5, 7],
    [-4, -1, 0, 14, 11, 15],
    [-4, -1, 0, 11, 8, 16],
];

#[derive(Clone, Copy)]
struct Piece {
    bb: Bounds,
    dir: i32,
    depth: i32,
    kind: usize,
    flags: [bool; 4],
    variant: i32,
    door: i32,
}
pub(super) fn oriented(at: [i32; 3], dir: i32, shape: Bounds) -> Bounds {
    let [x, y, z] = at;
    let [dx, dy, dz, w, h, l] = shape;
    match dir {
        0 => [
            x + dx,
            y + dy,
            z + dz,
            x + dx + w - 1,
            y + dy + h - 1,
            z + dz + l - 1,
        ],
        1 => [
            x - dz - l + 1,
            y + dy,
            z + dx,
            x - dz,
            y + dy + h - 1,
            z + dx + w - 1,
        ],
        2 => [
            x + dx,
            y + dy,
            z - dz - l + 1,
            x + dx + w - 1,
            y + dy + h - 1,
            z - dz,
        ],
        _ => [
            x + dz,
            y + dy,
            z + dx,
            x + dz + l - 1,
            y + dy + h - 1,
            z + dx + w - 1,
        ],
    }
}
struct Fort {
    pieces: Vec<Piece>,
    pending: Vec<usize>,
    counts: [i32; 11],
    last: Option<usize>,
}
impl Fort {
    fn collision(&self, bb: Bounds) -> Option<Bounds> {
        self.pieces.iter().map(|piece| piece.bb).find(|other| {
            (0..3).all(|axis| bb[axis] <= other[axis + 3] && bb[axis + 3] >= other[axis])
        })
    }
    fn build(
        &self,
        rng: &mut JavaRandom,
        at: [i32; 3],
        dir: i32,
        depth: i32,
        kind: usize,
    ) -> Option<Piece> {
        let mut shape = SHAPE[kind];
        let mut bb = oriented(at, dir, shape);
        if kind == 9 && self.collision(bb).is_some() {
            shape[4] = 6;
            bb = oriented(at, dir, shape);
        }
        if bb[1] <= 10 || self.collision(bb).is_some() {
            return None;
        }
        let door = if kind == PORTAL { 0 } else { rng.bounded(5) };
        let mut flags = [false; 4];
        let variant = match kind {
            0 => {
                flags[0] = rng.bounded(2) == 0;
                flags[1] = rng.bounded(2) == 0;
                0
            }
            4 => rng.bounded(5),
            CROSS => {
                for flag in &mut flags[..3] {
                    *flag = rng.bits(1) != 0;
                }
                flags[3] = rng.bounded(3) > 0;
                0
            }
            _ => 0,
        };
        Some(Piece {
            bb,
            dir,
            depth,
            kind: if kind == 3 { 2 } else { kind },
            flags,
            variant,
            door,
        })
    }
    fn select(
        &mut self,
        rng: &mut JavaRandom,
        at: [i32; 3],
        dir: i32,
        parent: Piece,
    ) -> Option<Piece> {
        let start = self.pieces[0].bb;
        let depth = parent.depth + 1;
        if parent.depth > 50 || (at[0] - start[0]).abs() > 112 || (at[2] - start[2]).abs() > 112 {
            return None;
        }
        if !(0..11).any(|kind| CAP[kind] > self.counts[kind]) {
            return None;
        }
        if parent.kind == START
            && let Some(piece) = self.build(rng, at, dir, depth, CROSS)
        {
            return Some(piece);
        }
        let total = (0..11)
            .filter(|&kind| CAP[kind] == 0 || self.counts[kind] < CAP[kind])
            .map(|kind| WEIGHT[kind])
            .sum();
        for _ in 0..5 {
            let mut ticket = rng.bounded(total);
            for kind in 0..11 {
                if CAP[kind] != 0 && self.counts[kind] >= CAP[kind] {
                    continue;
                }
                ticket -= WEIGHT[kind];
                if ticket >= 0 {
                    continue;
                }
                if self.last == Some(kind)
                    || (kind == 9 && depth <= 4)
                    || (kind == PORTAL && depth <= 5)
                {
                    break;
                }
                if let Some(piece) = self.build(rng, at, dir, depth, kind) {
                    self.counts[kind] += 1;
                    self.last = Some(kind);
                    return Some(piece);
                }
            }
        }
        let shape = |length| oriented(at, dir, [-1, -1, 0, 5, 5, length]);
        let bb = shape(4);
        let hit = self.collision(bb)?;
        if hit[1] != bb[1] {
            return None;
        }
        for length in (1..=3).rev() {
            let smaller = shape(length - 1);
            if !(0..3).all(|axis| smaller[axis] <= hit[axis + 3] && smaller[axis + 3] >= hit[axis])
                && bb[1] > 1
            {
                return Some(Piece {
                    bb: shape(length),
                    dir,
                    depth,
                    kind: FILLER,
                    flags: [false; 4],
                    variant: length,
                    door: 0,
                });
            }
        }
        None
    }
    fn connect(&mut self, rng: &mut JavaRandom, piece: Piece, side: i32, offset: i32, rise: i32) {
        let [west, y, north, east, _, south] = piece.bb;
        let (x, z, dir) = match (side, piece.dir) {
            (0, 0) => (west + offset, south + 1, 0),
            (0, 1) => (west - 1, north + offset, 1),
            (0, 2) => (west + offset, north - 1, 2),
            (0, 3) => (east + 1, north + offset, 3),
            (-1, 0 | 2) => (west - 1, north + offset, 1),
            (-1, _) => (west + offset, north - 1, 2),
            (_, 0 | 2) => (east + 1, north + offset, 3),
            _ => (west + offset, south + 1, 0),
        };
        if let Some(next) = self.select(rng, [x, y + rise, z], dir, piece) {
            self.pending.push(self.pieces.len());
            self.pieces.push(next);
        }
    }
    fn expand(&mut self, rng: &mut JavaRandom, piece: Piece) {
        let mut add = |s, o, y| self.connect(rng, piece, s, o, y);
        match piece.kind {
            START | 1 | 5 | 6 | 8 => add(0, 1, 1),
            0 => {
                add(0, 1, 1);
                if piece.flags[0] {
                    add(-1, 2, 1);
                }
                if piece.flags[1] {
                    add(1, 2, 1);
                }
            }
            2 | 3 => {
                let left = (piece.kind == 2) == matches!(piece.dir, 2 | 3);
                add(if left { -1 } else { 1 }, 1, 1);
            }
            4 => {
                add(0, 4, 1);
                add(-1, 4, 1);
                add(1, 4, 1);
            }
            CROSS => {
                let (lo, hi) = if matches!(piece.dir, 1 | 2) {
                    (5, 3)
                } else {
                    (3, 5)
                };
                add(0, 5, 1);
                for (index, (side, offset, rise)) in
                    [(-1, 1, lo), (-1, 7, hi), (1, 1, lo), (1, 7, hi)]
                        .into_iter()
                        .enumerate()
                {
                    if piece.flags[index] {
                        add(side, offset, rise);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Overworld {
    pub fn stronghold_start_nbt(&self, cx: i32, cz: i32) -> Option<Fields> {
        if !self.stronghold_starts().contains(&(cx, cz)) {
            return None;
        }
        let mut world = JavaRandom::new(self.seed);
        let mut rng = JavaRandom::new(
            self.seed
                ^ world.long().wrapping_mul(i64::from(cx))
                ^ world.long().wrapping_mul(i64::from(cz)),
        );
        rng.bits(32);
        let x = cx * 16 + 2;
        let z = cz * 16 + 2;
        loop {
            let start = Piece {
                bb: [x, 64, z, x + 4, 74, z + 4],
                dir: (rng.bounded(4) + 2) % 4,
                depth: 0,
                kind: START,
                flags: [false; 4],
                variant: 0,
                door: 0,
            };
            let mut fort = Fort {
                pieces: vec![start],
                pending: vec![],
                counts: [0; 11],
                last: None,
            };
            fort.expand(&mut rng, start);
            while !fort.pending.is_empty() {
                let index = rng.bounded(fort.pending.len() as i32) as usize;
                let index = fort.pending.remove(index);
                fort.expand(&mut rng, fort.pieces[index]);
            }
            let mut overall = start.bb;
            for piece in &fort.pieces {
                for axis in 0..3 {
                    overall[axis] = overall[axis].min(piece.bb[axis]);
                    overall[axis + 3] = overall[axis + 3].max(piece.bb[axis + 3]);
                }
            }
            let mut top = overall[4] - overall[1] + 2;
            if top < 53 {
                top += rng.bounded(53 - top);
            }
            if fort.counts[PORTAL] == 0 {
                continue;
            }
            let dy = top - overall[4];
            let bounds = |mut bb: Bounds| {
                bb[1] += dy;
                bb[4] += dy;
                Value::IntArray(IntArray::new(bb.to_vec()))
            };
            let children = fort
                .pieces
                .iter()
                .map(|piece| {
                    let mut fields = Fields::from([
                        ("id".into(), Value::String(IDS[piece.kind].into())),
                        ("BB".into(), bounds(piece.bb)),
                        ("GD".into(), Value::Int(piece.depth)),
                        ("O".into(), Value::Int(piece.dir)),
                        (
                            "EntryDoor".into(),
                            Value::String(
                                ["OPENING", "OPENING", "WOOD_DOOR", "GRATES", "IRON_DOOR"]
                                    [piece.door as usize]
                                    .into(),
                            ),
                        ),
                    ]);
                    match piece.kind {
                        0 => {
                            fields.insert("Left".into(), Value::Byte(piece.flags[0].into()));
                            fields.insert("Right".into(), Value::Byte(piece.flags[1].into()));
                        }
                        4 => {
                            fields.insert("Type".into(), Value::Int(piece.variant));
                        }
                        6 | START => {
                            fields
                                .insert("Source".into(), Value::Byte((piece.kind == START).into()));
                        }
                        CROSS => {
                            for (key, flag) in ["leftLow", "leftHigh", "rightLow", "rightHigh"]
                                .into_iter()
                                .zip(piece.flags)
                            {
                                fields.insert(key.into(), Value::Byte(flag.into()));
                            }
                        }
                        8 => {
                            fields.insert("Chest".into(), Value::Byte(0));
                        }
                        9 => {
                            fields.insert(
                                "Tall".into(),
                                Value::Byte((piece.bb[4] - piece.bb[1] + 1 > 6).into()),
                            );
                        }
                        PORTAL => {
                            fields.insert("Mob".into(), Value::Byte(0));
                        }
                        FILLER => {
                            fields.insert("Steps".into(), Value::Int(piece.variant));
                        }
                        _ => {}
                    }
                    Value::Compound(fields)
                })
                .collect();
            return Some(Fields::from([
                ("id".into(), Value::String("Stronghold".into())),
                ("ChunkX".into(), Value::Int(cx)),
                ("ChunkZ".into(), Value::Int(cz)),
                ("BB".into(), bounds(overall)),
                ("Children".into(), Value::List(children)),
            ]));
        }
    }

    pub fn unexplored_eye_target(&self, x: i32, z: i32) -> (i32, i32, i32) {
        unexplored_eye_target(self.stronghold_starts(), x, z)
    }
}

pub(crate) fn unexplored_eye_target(starts: [(i32, i32); 3], x: i32, z: i32) -> (i32, i32, i32) {
    starts
        .map(|(cx, cz)| (cx * 16 + 8, 64, cz * 16 + 8))
        .into_iter()
        .min_by_key(|&(xx, _, zz)| {
            let dx = i128::from(xx) - i128::from(x);
            let dz = i128::from(zz) - i128::from(z);
            dx * dx + dz * dz
        })
        .expect("the DEFAULT ring always contains three starts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ring_start_contains_a_portal_and_attracts_an_eye() {
        let provider = Overworld::new(12345);
        for (cx, cz) in provider.stronghold_starts() {
            let expected = (cx * 16 + 8, 64, cz * 16 + 8);
            assert_eq!(
                provider.unexplored_eye_target(expected.0 - 31, expected.2 + 14),
                expected
            );
            let start = provider.stronghold_start_nbt(cx, cz).unwrap();
            let Value::List(children) = &start["Children"] else {
                panic!("expected pieces")
            };
            let ids: Vec<_> = children
                .iter()
                .map(|child| {
                    let Value::Compound(fields) = child else {
                        panic!("expected compound")
                    };
                    &fields["id"]
                })
                .collect();
            assert_eq!(ids[0], &Value::String("SHStart".into()));
            assert_eq!(
                ids.iter()
                    .filter(|id| ***id == Value::String("SHPR".into()))
                    .count(),
                1
            );
            assert_eq!(Some(start), provider.stronghold_start_nbt(cx, cz));
        }
    }
}
