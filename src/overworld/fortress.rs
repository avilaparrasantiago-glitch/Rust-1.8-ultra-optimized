use super::JavaRandom;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Start,
    Bridge,
    Crossing,
    Room,
    Stairs,
    Throne,
    Entrance,
    Corridor,
    Junction,
    Right,
    Left,
    Stairwell,
    Balcony,
    Farm,
    End,
}
use Kind::*;

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Start => "NeStart",
            Bridge => "NeBS",
            Crossing => "NeBCr",
            Room => "NeRC",
            Stairs => "NeSR",
            Throne => "NeMT",
            Entrance => "NeCE",
            Corridor => "NeSC",
            Junction => "NeSCSC",
            Right => "NeSCRT",
            Left => "NeSCLT",
            Stairwell => "NeCCS",
            Balcony => "NeCTB",
            Farm => "NeCSR",
            End => "NeBEF",
        }
    }
    fn footprint(self) -> [i32; 5] {
        // local X and Y offsets, width, height and length
        match self {
            Start | Crossing => [-8, -3, 19, 10, 19],
            Bridge => [-1, -3, 5, 10, 19],
            Room => [-2, 0, 7, 9, 7],
            Stairs => [-2, 0, 7, 11, 7],
            Throne => [-2, 0, 7, 8, 9],
            Entrance | Farm => [-5, -3, 13, 14, 13],
            End => [-1, -3, 5, 10, 8],
            Balcony => [-3, 0, 9, 7, 9],
            Stairwell => [-1, -7, 5, 14, 10],
            Corridor | Junction | Right | Left => [-1, 0, 5, 7, 5],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub kind: Kind,
    pub facing: u8,
    pub depth: u8,
    pub bounds: [i32; 6],
    pub chest: bool,
    pub rubble_seed: i32,
}
#[derive(Clone, Copy)]
struct Choice {
    kind: Kind,
    weight: i32,
    max: u8,
    repeat: bool,
    used: u8,
}
impl Choice {
    fn new((kind, weight, max, repeat): (Kind, i32, u8, bool)) -> Self {
        Self {
            kind,
            weight,
            max,
            repeat,
            used: 0,
        }
    }
}
struct Layout {
    pieces: Vec<Piece>,
    pending: Vec<usize>,
    pools: [Vec<Choice>; 2],
    last: Option<Kind>,
    rng: JavaRandom,
}

impl Layout {
    fn fit(&mut self, kind: Kind, [x, y, z]: [i32; 3], facing: u8, depth: u8) -> Option<Piece> {
        let [offset, dy, w, h, l] = kind.footprint();
        let [x1, z1, x2, z2] = match facing {
            0 => [x + offset, z, x + offset + w - 1, z + l - 1],
            1 => [x - l + 1, z + offset, x, z + offset + w - 1],
            2 => [x + offset, z - l + 1, x + offset + w - 1, z],
            _ => [x, z + offset, x + l - 1, z + offset + w - 1],
        };
        let bounds = [x1, y + dy, z1, x2, y + dy + h - 1, z2];
        if bounds[1] <= 10
            || self.pieces.iter().any(|p| {
                (0..3).all(|axis| {
                    p.bounds[axis] <= bounds[axis + 3] && p.bounds[axis + 3] >= bounds[axis]
                })
            })
        {
            return None;
        }
        Some(Piece {
            kind,
            facing,
            depth,
            bounds,
            chest: matches!(kind, Left | Right) && self.rng.bounded(3) == 0,
            rubble_seed: if kind == End {
                self.rng.bits(32) as i32
            } else {
                0
            },
        })
    }
    fn choose(&mut self, pos: [i32; 3], facing: u8, depth: u8, inside: bool) -> Option<Piece> {
        let pool = usize::from(inside);
        let choices = &self.pools[pool];
        if depth <= 30 && choices.iter().any(|c| c.max != 0 && c.used < c.max) {
            let total = choices.iter().map(|c| c.weight).sum();
            for _ in 0..5 {
                let mut draw = self.rng.bounded(total);
                for idx in 0..self.pools[pool].len() {
                    let c = self.pools[pool][idx];
                    draw -= c.weight;
                    if draw >= 0 {
                        continue;
                    }
                    if !c.repeat && self.last == Some(c.kind) {
                        break;
                    }
                    if let Some(piece) = self.fit(c.kind, pos, facing, depth) {
                        self.last = Some(c.kind);
                        self.pools[pool][idx].used += 1;
                        if c.max != 0 && c.used + 1 == c.max {
                            self.pools[pool].remove(idx);
                        }
                        return Some(piece);
                    }
                }
            }
        }
        self.fit(End, pos, facing, depth)
    }
    fn connect(&mut self, p: Piece, side: i32, lateral: i32, vertical: i32, inside: bool) {
        let [x, y, z, xx, _, zz] = p.bounds;
        let (px, pz, facing) = match (side, p.facing) {
            (0, 0) => (x + lateral, zz + 1, 0),
            (0, 1) => (x - 1, z + lateral, 1),
            (0, 2) => (x + lateral, z - 1, 2),
            (0, _) => (xx + 1, z + lateral, 3),
            (-1, 0 | 2) => (x - 1, z + lateral, 1),
            (-1, _) => (x + lateral, z - 1, 2),
            (_, 0 | 2) => (xx + 1, z + lateral, 3),
            (_, _) => (x + lateral, zz + 1, 0),
        };
        let pos = [px, y + vertical, pz];
        let start = self.pieces[0];
        if px.abs_diff(start.bounds[0]) > 112 || pz.abs_diff(start.bounds[2]) > 112 {
            self.fit(End, pos, facing, p.depth);
        } else if let Some(piece) = self.choose(pos, facing, p.depth + 1, inside) {
            self.pending.push(self.pieces.len());
            self.pieces.push(piece);
        }
    }
    fn extend(&mut self, p: Piece) {
        match p.kind {
            Start | Crossing => {
                for side in [0, -1, 1] {
                    self.connect(p, side, 8, 3, false)
                }
            }
            Room => {
                for side in [0, -1, 1] {
                    self.connect(p, side, 2, 0, false)
                }
            }
            Bridge => self.connect(p, 0, 1, 3, false),
            Stairs => self.connect(p, 1, 2, 6, false),
            Entrance => self.connect(p, 0, 5, 3, true),
            Farm => {
                for y in [3, 11] {
                    self.connect(p, 0, 5, y, true)
                }
            }
            Corridor | Stairwell => self.connect(p, 0, 1, 0, true),
            Junction => {
                for side in [0, -1, 1] {
                    self.connect(p, side, 1, 0, true)
                }
            }
            Left => self.connect(p, -1, 1, 0, true),
            Right => self.connect(p, 1, 1, 0, true),
            Balcony => {
                for side in [-1, 1] {
                    let inside = self.rng.bounded(8) > 0;
                    self.connect(
                        p,
                        side,
                        if matches!(p.facing, 1 | 2) { 5 } else { 1 },
                        0,
                        inside,
                    );
                }
            }
            End | Throne => {}
        }
    }
}

pub fn fortress_pieces(seed: i64, cx: i32, cz: i32) -> Vec<Piece> {
    let (rx, rz) = (cx >> 4, cz >> 4);
    let Some((x, z, rng)) = candidate(seed, rx, rz) else {
        return Vec::new();
    };
    if (x, z) != (cx, cz) {
        return Vec::new();
    }
    assemble(cx, cz, rng)
}

fn candidate(seed: i64, rx: i32, rz: i32) -> Option<(i32, i32, JavaRandom)> {
    let mut rng = JavaRandom::new(seed ^ i64::from(rx ^ (rz << 4)));
    rng.bits(32);
    if rng.bounded(3) != 0 {
        return None;
    }
    Some((
        (rx << 4) + 4 + rng.bounded(8),
        (rz << 4) + 4 + rng.bounded(8),
        rng,
    ))
}

fn assemble(cx: i32, cz: i32, mut rng: JavaRandom) -> Vec<Piece> {
    let (x, z) = (cx * 16 + 2, cz * 16 + 2);
    let start = Piece {
        kind: Start,
        facing: ((rng.bounded(4) + 2) % 4) as u8,
        depth: 0,
        bounds: [x, 64, z, x + 18, 73, z + 18],
        chest: false,
        rubble_seed: 0,
    };
    let mut layout = Layout {
        pieces: vec![start],
        pending: Vec::new(),
        last: None,
        rng,
        pools: [
            [
                (Bridge, 30, 0, true),
                (Crossing, 10, 4, false),
                (Room, 10, 4, false),
                (Stairs, 10, 3, false),
                (Throne, 5, 2, false),
                (Entrance, 5, 1, false),
            ]
            .into_iter()
            .map(Choice::new)
            .collect(),
            [
                (Corridor, 25, 0, true),
                (Junction, 15, 5, false),
                (Right, 5, 10, false),
                (Left, 5, 10, false),
                (Stairwell, 10, 3, true),
                (Balcony, 7, 2, false),
                (Farm, 5, 2, false),
            ]
            .into_iter()
            .map(Choice::new)
            .collect(),
        ],
    };
    layout.extend(start);
    while !layout.pending.is_empty() {
        let selected = layout.rng.bounded(layout.pending.len() as i32) as usize;
        let index = layout.pending.remove(selected);
        layout.extend(layout.pieces[index]);
    }
    let (min, max) = layout.pieces.iter().fold((64, 73), |(a, b), p| {
        (a.min(p.bounds[1]), b.max(p.bounds[4]))
    });
    let range = 23 - (max - min + 1);
    let dy = 48 - min
        + if range > 1 {
            layout.rng.bounded(range)
        } else {
            0
        };
    for p in &mut layout.pieces {
        p.bounds[1] += dy;
        p.bounds[4] += dy;
    }
    layout.pieces
}

fn shapes() -> &'static [&'static [u8]; 52] {
    static TABLE: std::sync::OnceLock<[&[u8]; 52]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| super::temple::rectangles(include_bytes!("fortress-shapes.bin")))
}

impl Piece {
    fn absolute(&self, x: i32, y: i32, z: i32) -> [i32; 3] {
        let [a, b, c, d, _, f] = self.bounds;
        match self.facing {
            0 => [a + x, b + y, c + z],
            1 => [d - z, b + y, c + x],
            2 => [a + x, b + y, f - z],
            _ => [a + z, b + y, c + x],
        }
    }
    fn paint(&self, cx: i32, cz: i32, states: &mut [u16]) {
        let [x, y, z, xx, _, zz] = self.bounds;
        if cx < x >> 4 || cx > xx >> 4 || cz < z >> 4 || cz > zz >> 4 {
            return;
        }
        let (bx, bz) = (cx * 16, cz * 16);
        if self.kind == End {
            let mut rng = JavaRandom::new(i64::from(self.rubble_seed));
            let mut row = |ix, iy, bound| {
                let stop = rng.bounded(bound);
                for iz in 0..=stop {
                    let [px, py, pz] = self.absolute(ix, iy, iz);
                    if px >> 4 == cx && pz >> 4 == cz {
                        states[(py * 256 + (pz - bz) * 16 + px - bx) as usize] = 112 << 4;
                    }
                }
            };
            for ix in 0..5 {
                for iy in 3..5 {
                    row(ix, iy, 8);
                }
            }
            row(0, 5, 8);
            row(4, 5, 8);
            for ix in 0..5 {
                row(ix, 2, 5);
            }
            for ix in 0..5 {
                for iy in 0..2 {
                    row(ix, iy, 3);
                }
            }
            return;
        }
        let index = match self.kind {
            Bridge => 0,
            Start | Crossing => 1,
            Room => 2,
            Stairs => 3,
            Throne => 4,
            Entrance => 5,
            Corridor => 6,
            Junction => 7,
            Right => 8,
            Left => 9,
            Stairwell => 10,
            Balcony => 11,
            Farm => 12,
            End => unreachable!(),
        };
        for rect in shapes()[index * 4 + usize::from(self.facing)]
            .as_chunks::<8>()
            .0
        {
            let state = u16::from_be_bytes([rect[6], rect[7]]);
            let (low, high) = (y + i32::from(rect[1] as i8), y + i32::from(rect[4] as i8));
            for pz in (z + i32::from(rect[2])).max(bz)..=(z + i32::from(rect[5])).min(bz + 15) {
                for px in (x + i32::from(rect[0])).max(bx)..=(x + i32::from(rect[3])).min(bx + 15) {
                    let offset = ((pz - bz) * 16 + px - bx) as usize;
                    if rect[1] == 255 {
                        for py in (2..=low).rev() {
                            let b = &mut states[py as usize * 256 + offset];
                            if !matches!(*b >> 4, 0 | 8..=11) {
                                break;
                            }
                            *b = state;
                        }
                    } else {
                        for py in low..=high {
                            states[py as usize * 256 + offset] = state;
                        }
                    }
                }
            }
        }
        if self.chest {
            let [px, py, pz] = self.absolute(if self.kind == Left { 3 } else { 1 }, 2, 3);
            if px >> 4 == cx && pz >> 4 == cz {
                let north_wall = shapes()[index * 4 + usize::from(self.facing)]
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .any(|r| {
                        let [lx, ly, lz] = [px - x, py - y, pz - 1 - z];
                        (i32::from(r[0])..=i32::from(r[3])).contains(&lx)
                            && (i32::from(r[1] as i8)..=i32::from(r[4] as i8)).contains(&ly)
                            && (i32::from(r[2])..=i32::from(r[5])).contains(&lz)
                            && u16::from_be_bytes([r[6], r[7]]) >> 4 == 112
                    });
                states[(py * 256 + (pz - bz) * 16 + px - bx) as usize] =
                    54 << 4 | if north_wall { 3 } else { 2 };
            }
        }
    }
}

fn nearby_layouts(seed: i64, cx: i32, cz: i32, mut visit: impl FnMut(Vec<Piece>)) {
    for rx in ((cx - 10) >> 4)..=((cx + 10) >> 4) {
        for rz in ((cz - 10) >> 4)..=((cz + 10) >> 4) {
            if let Some((x, z, rng)) = candidate(seed, rx, rz)
                && x.abs_diff(cx) <= 10
                && z.abs_diff(cz) <= 10
            {
                visit(assemble(x, z, rng));
            }
        }
    }
}

// Structure population owns the 16x16 window starting eight blocks into the chunk.
pub(crate) fn population_features(
    seed: i64,
    cx: i32,
    cz: i32,
    mut visit: impl FnMut([i32; 3], bool),
) {
    nearby_layouts(seed, cx, cz, |pieces| {
        for p in pieces {
            let point = if p.chest {
                p.absolute(if p.kind == Left { 3 } else { 1 }, 2, 3)
            } else if p.kind == Entrance {
                p.absolute(6, 5, 6)
            } else {
                continue;
            };
            if ((point[0] - 8) >> 4, (point[2] - 8) >> 4) == (cx, cz) {
                visit(point, p.chest);
            }
        }
    });
}
pub(super) fn paint_column(
    seed: i64,
    cx: i32,
    cz: i32,
    states: &mut [u16],
) -> Vec<crate::arena::BlockPos> {
    let mut spawners = Vec::new();
    nearby_layouts(seed, cx, cz, |pieces| {
        for p in pieces {
            p.paint(cx, cz, states);
            if p.kind == Throne {
                let [x, y, z] = p.absolute(3, 5, 5);
                if (x >> 4, z >> 4) == (cx, cz) {
                    spawners.push((x, y as u8, z));
                }
            }
        }
    });
    spawners
}

/// Inclusive vertical and horizontal bounds used by ChunkProviderHell's monster list.
/// A position inside a piece qualifies on any floor. Gaps inside the enclosing
/// fortress qualify only when the block directly underneath is nether brick.
pub struct FortressSpawnArea {
    column: (i32, i32),
    pieces: Vec<[i32; 6]>,
    enclosures: Vec<[i32; 6]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnListEntry {
    pub entity: &'static str,
    pub weight: u8,
    pub min_group: u8,
    pub max_group: u8,
}
const fn spawn(entity: &'static str, weight: u8, min_group: u8, max_group: u8) -> SpawnListEntry {
    SpawnListEntry {
        entity,
        weight,
        min_group,
        max_group,
    }
}
// Nether skeletons choose their wither subtype during entity initialization.
const MONSTERS: [SpawnListEntry; 4] = [
    spawn("Blaze", 10, 2, 3),
    spawn("PigZombie", 5, 4, 4),
    spawn("Skeleton", 10, 4, 4),
    spawn("LavaSlime", 3, 4, 4),
];

impl FortressSpawnArea {
    pub fn for_column(seed: i64, cx: i32, cz: i32) -> Self {
        let mut area = Self {
            column: (cx, cz),
            pieces: Vec::new(),
            enclosures: Vec::new(),
        };
        let intersects =
            |b: [i32; 6]| cx >= b[0] >> 4 && cx <= b[3] >> 4 && cz >= b[2] >> 4 && cz <= b[5] >> 4;
        nearby_layouts(seed, cx, cz, |pieces| {
            let mut enclosure = pieces[0].bounds;
            for piece in pieces {
                let b = piece.bounds;
                for axis in 0..3 {
                    enclosure[axis] = enclosure[axis].min(b[axis]);
                    enclosure[axis + 3] = enclosure[axis + 3].max(b[axis + 3]);
                }
                if intersects(b) {
                    area.pieces.push(b);
                }
            }
            if intersects(enclosure) {
                area.enclosures.push(enclosure);
            }
        });
        area
    }

    pub fn monsters_at(&self, pos: [i32; 3], floor_state: u16) -> &'static [SpawnListEntry] {
        if self.uses_fortress_monsters(pos, floor_state) {
            &MONSTERS
        } else {
            &[]
        }
    }
    pub fn uses_fortress_monsters(&self, [x, y, z]: [i32; 3], floor_state: u16) -> bool {
        if (x >> 4, z >> 4) != self.column {
            return false;
        }
        let contains = |b: &[i32; 6]| {
            x >= b[0] && x <= b[3] && y >= b[1] && y <= b[4] && z >= b[2] && z <= b[5]
        };
        self.pieces.iter().any(contains)
            || (floor_state >> 4 == 112 && self.enclosures.iter().any(contains))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn fortress_spawn_bounds_from_official_piece_boxes() {
        for (cx, cz, expected) in [
            (
                20,
                -54,
                "1aebc1b8ca43c9112484761a9f83e306c4d008ba235e478c51fc2cde5bb661b3",
            ),
            (
                -25,
                -41,
                "df0325bbcac3eb3493117a95c8d8941579198d8cf216d6d64b96712fd2a159ec",
            ),
        ] {
            let mut hash = Sha256::new();
            let mut row = [0u8; 16];
            for z in cz - 9..=cz + 9 {
                for x in cx - 9..=cx + 9 {
                    let area = FortressSpawnArea::for_column(12345, x, z);
                    for y in 0..128 {
                        for dz in 0..16 {
                            for (dx, byte) in row.iter_mut().enumerate() {
                                let pos = [16 * x + dx as i32, y, 16 * z + dz];
                                *byte = u8::from(area.uses_fortress_monsters(pos, 112 << 4))
                                    | (u8::from(area.uses_fortress_monsters(pos, 87 << 4)) << 1);
                            }
                            hash.update(row);
                        }
                    }
                }
            }
            assert_eq!(format!("{:x}", hash.finalize()), expected);
        }
    }
    #[test]
    fn official_fortress_blocks_in_negative_region() {
        let provider =
            crate::overworld::DimensionTerrain::new(12345, crate::overworld::Dimension::Nether);
        for (x, z, expected) in [
            (
                17,
                -57,
                "d4c0137095e88235be1733d7cb58d177a42a2fbeb91bb76a8d1805d809d494bd",
            ),
            (
                -27,
                -48,
                "12368f5986ec8648a3ad78d7c667b6b8b38baf9c417b56f4748846eb3ba63fb0",
            ),
        ] {
            let mut hash = Sha256::new();
            for cz in z..z + 8 {
                for cx in x..x + 8 {
                    let mut blocks = provider.provide(cx, cz);
                    paint_column(12345, cx, cz, &mut blocks);
                    for s in &blocks[..32768] {
                        hash.update(s.to_be_bytes());
                    }
                }
            }
            assert_eq!(format!("{:x}", hash.finalize()), expected);
        }
    }
    #[test]
    fn official_fortress_piece_layouts() {
        let mut hash = Sha256::new();
        for z in -64..64 {
            for x in -64..64 {
                for p in fortress_pieces(12345, x, z) {
                    let [a, b, c, d, e, f] = p.bounds;
                    hash.update(format!(
                        "{x} {z} {} {} {} {a} {b} {c} {d} {e} {f} {} {}\n",
                        p.kind.id(),
                        p.facing,
                        p.depth,
                        i32::from(p.chest),
                        p.rubble_seed
                    ));
                }
            }
        }
        assert_eq!(
            format!("{:x}", hash.finalize()),
            "19c2945f20432773ed7ea543d64a3607a78bf519bfae82fa64781026a7fb0bd0"
        );
    }
}
