use super::{JavaRandom, Overworld, StructureGrid, stronghold::oriented};
use fastnbt::{IntArray, Value};
use std::collections::HashMap;
mod placement;
pub(super) use placement::{PopulationDelta, populate_villages};

type Box3 = [i32; 6];
type Fields = HashMap<String, Value>;
const IDS: [&str; 12] = [
    "ViSH", "ViST", "ViBH", "ViSmH", "ViPH", "ViDF", "ViF", "ViS", "ViTRH", "ViL", "ViSR",
    "ViStart",
];
const SIZES: [[i32; 3]; 10] = [
    [5, 6, 5],
    [5, 12, 9],
    [9, 9, 6],
    [4, 6, 5],
    [9, 7, 11],
    [13, 4, 9],
    [7, 4, 9],
    [10, 6, 7],
    [9, 7, 12],
    [3, 4, 2],
];
const WEIGHTS: [i32; 9] = [4, 20, 20, 3, 15, 3, 3, 15, 8];
const LIMITS: [(i32, i32); 9] = [
    (2, 4),
    (0, 1),
    (0, 2),
    (2, 5),
    (0, 2),
    (1, 4),
    (2, 4),
    (0, 1),
    (0, 3),
];
const LAMP: usize = 9;
const ROAD: usize = 10;
const WELL: usize = 11;

#[derive(Clone)]
struct Piece {
    bb: Box3,
    dir: i32,
    depth: i32,
    kind: usize,
    extra: Fields,
}

struct Village<'a> {
    world: &'a Overworld,
    rng: JavaRandom,
    pieces: Vec<Piece>,
    roads: Vec<usize>,
    remaining: [i32; 9],
    last: usize,
    desert: bool,
}

fn span(bb: Box3) -> i32 {
    (bb[3] - bb[0]).max(bb[5] - bb[2]) + 1
}
fn bounds(bb: Box3) -> Value {
    Value::IntArray(IntArray::new(bb.to_vec()))
}

impl Village<'_> {
    fn free(&self, bb: Box3) -> bool {
        self.pieces
            .iter()
            .all(|p| (0..3).any(|i| bb[i] > p.bb[i + 3] || p.bb[i] > bb[i + 3]))
    }
    fn in_range(&self, x: i32, z: i32) -> bool {
        let root = self.pieces[0].bb;
        (x - root[0]).abs() <= 112 && (z - root[2]).abs() <= 112
    }
    fn viable(&self, bb: Box3) -> bool {
        let x = (bb[0] + bb[3]) / 2;
        let z = (bb[2] + bb[5]) / 2;
        let r = (span(bb) - 1) / 2 + 4;
        let (a, b) = ((x - r) >> 2, (z - r) >> 2);
        self.world
            .biomes
            .quarter(
                a,
                b,
                (((x + r) >> 2) - a + 1) as usize,
                (((z + r) >> 2) - b + 1) as usize,
            )
            .iter()
            .all(|id| matches!(id, 1 | 2 | 35))
    }
    fn road(&mut self, at: [i32; 3], dir: i32, depth: i32) {
        if depth > 3 || !self.in_range(at[0], at[2]) {
            return;
        }
        let mut length = 7 * (3 + self.rng.bounded(3));
        while length >= 7 {
            let bb = oriented(at, dir, [0, 0, 0, 3, 3, length]);
            if self.free(bb) {
                if bb[1] > 10 && self.viable(bb) {
                    self.roads.push(self.pieces.len());
                    self.pieces.push(Piece {
                        bb,
                        dir,
                        depth,
                        kind: ROAD,
                        extra: HashMap::from([("Length".into(), Value::Int(length))]),
                    });
                }
                return;
            }
            length -= 7;
        }
    }
    fn piece(&mut self, at: [i32; 3], dir: i32, kind: usize, depth: i32) -> Option<Piece> {
        let [w, h, l] = SIZES[kind];
        let bb = oriented(at, dir, [0, 0, 0, w, h, l]);
        if !self.free(bb) {
            return None;
        }
        let mut extra = Fields::new();
        match kind {
            0 => {
                extra.insert("Terrace".into(), Value::Byte(self.rng.bits(1) as i8));
            }
            3 => {
                extra.insert("C".into(), Value::Byte(self.rng.bits(1) as i8));
                extra.insert("T".into(), Value::Int(self.rng.bounded(3)));
            }
            5 | 6 => {
                for key in ["CA", "CB", "CC", "CD"]
                    .into_iter()
                    .take(if kind == 5 { 4 } else { 2 })
                {
                    let crop = match self.rng.bounded(5) {
                        0 => 141,
                        1 => 142,
                        _ => 59,
                    };
                    extra.insert(key.into(), Value::Int(crop));
                }
            }
            7 => {
                extra.insert("Chest".into(), Value::Byte(0));
            }
            _ => {}
        }
        Some(Piece {
            bb,
            dir,
            depth,
            kind,
            extra,
        })
    }
    fn house(&mut self, at: [i32; 3], dir: i32, depth: i32) -> Option<i32> {
        if depth > 50 || !self.in_range(at[0], at[2]) {
            return None;
        }
        let total: i32 = (0..9)
            .filter(|&i| self.remaining[i] > 0)
            .map(|i| WEIGHTS[i])
            .sum();
        if total == 0 {
            return None;
        }
        let mut chosen = None;
        'attempt: for _ in 0..5 {
            let mut ticket = self.rng.bounded(total);
            for (i, &weight) in WEIGHTS.iter().enumerate() {
                if self.remaining[i] == 0 {
                    continue;
                }
                ticket -= weight;
                if ticket >= 0 {
                    continue;
                }
                if self.last == i && total > weight {
                    break;
                }
                if let Some(p) = self.piece(at, dir, i, depth + 1) {
                    self.remaining[i] -= 1;
                    self.last = i;
                    chosen = Some(p);
                    break 'attempt;
                }
            }
        }
        let p = chosen.or_else(|| self.piece(at, dir, LAMP, depth + 1))?;
        if !self.viable(p.bb) {
            return None;
        }
        let length = span(p.bb);
        self.pieces.push(p);
        Some(length)
    }
    fn extend(&mut self, idx: usize) {
        let p = &self.pieces[idx];
        let [x, y, z, x1, _, z1] = p.bb;
        let (dir, depth, length) = (p.dir, p.depth, span(p.bb));
        let axial = dir % 2 == 0;
        let mut built = false;
        for right in [false, true] {
            let mut offset = self.rng.bounded(5);
            while offset < length - 8 {
                let (at, facing) = match (axial, right) {
                    (true, false) => ([x - 1, y, z + offset], 1),
                    (true, true) => ([x1 + 1, y, z + offset], 3),
                    (false, false) => ([x + offset, y, z - 1], 2),
                    (false, true) => ([x + offset, y, z1 + 1], 0),
                };
                if let Some(size) = self.house(at, facing, depth) {
                    offset += size;
                    built = true;
                }
                offset += 2 + self.rng.bounded(5);
            }
        }
        for right in [false, true] {
            if !built || self.rng.bounded(3) == 0 {
                continue;
            }
            let (at, facing) = if axial {
                (
                    [
                        if right { x1 + 1 } else { x - 1 },
                        y,
                        if dir == 2 { z } else { z1 - 2 },
                    ],
                    if right { 3 } else { 1 },
                )
            } else {
                (
                    [
                        if dir == 1 { x } else { x1 - 2 },
                        y,
                        if right { z1 + 1 } else { z - 1 },
                    ],
                    if right { 0 } else { 2 },
                )
            };
            self.road(at, facing, depth);
        }
    }
}

impl Overworld {
    pub fn village_start_nbt(&self, cx: i32, cz: i32) -> Option<Fields> {
        if !StructureGrid::Village.starts_at(self.seed, &self.biomes, cx, cz) {
            return None;
        }
        let mut seed = JavaRandom::new(self.seed);
        let mut rng = JavaRandom::new(
            self.seed ^ seed.long().wrapping_mul(cx.into()) ^ seed.long().wrapping_mul(cz.into()),
        );
        rng.bits(32);
        let remaining = LIMITS.map(|(min, max)| min + rng.bounded(max - min + 1));
        let (x, z) = (cx * 16 + 2, cz * 16 + 2);
        let dir = (rng.bounded(4) + 2) % 4;
        let root = Piece {
            bb: [x, 64, z, x + 5, 78, z + 5],
            dir,
            depth: 0,
            kind: WELL,
            extra: Fields::new(),
        };
        let mut village = Village {
            world: self,
            rng,
            pieces: vec![root],
            roads: vec![],
            remaining,
            last: usize::MAX,
            desert: matches!(self.biomes.blocks(x, z, 1, 1)[0], 2 | 17),
        };
        for (at, dir) in [
            ([x - 1, 74, z + 1], 1),
            ([x + 6, 74, z + 1], 3),
            ([x + 1, 74, z - 1], 2),
            ([x + 1, 74, z + 6], 0),
        ] {
            village.road(at, dir, 0);
        }
        while !village.roads.is_empty() {
            let i = village.rng.bounded(village.roads.len() as i32) as usize;
            let idx = village.roads.remove(i);
            village.extend(idx);
        }
        let valid = village.pieces.iter().filter(|p| p.kind != ROAD).count() > 2;
        let mut overall = village.pieces[0].bb;
        let children = village
            .pieces
            .into_iter()
            .map(|mut p| {
                for i in 0..3 {
                    overall[i] = overall[i].min(p.bb[i]);
                    overall[i + 3] = overall[i + 3].max(p.bb[i + 3]);
                }
                p.extra.extend([
                    ("id".into(), Value::String(IDS[p.kind].into())),
                    ("BB".into(), bounds(p.bb)),
                    ("O".into(), Value::Int(p.dir)),
                    ("GD".into(), Value::Int(p.depth)),
                    ("HPos".into(), Value::Int(-1)),
                    ("VCount".into(), Value::Int(0)),
                    ("Desert".into(), Value::Byte(village.desert as i8)),
                ]);
                Value::Compound(p.extra)
            })
            .collect();
        Some(HashMap::from([
            ("id".into(), Value::String("Village".into())),
            ("ChunkX".into(), Value::Int(cx)),
            ("ChunkZ".into(), Value::Int(cz)),
            ("Valid".into(), Value::Byte(valid as i8)),
            ("Children".into(), Value::List(children)),
            ("BB".into(), bounds(overall)),
        ]))
    }
}
