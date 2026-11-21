use super::{CellRandom, layer_seed};
use std::sync::Arc;

#[derive(Clone, Copy)]
enum Kind {
    Island,
    Fuzzy,
    Zoom,
    Land,
    RemoveOcean,
    Snow,
    Cool,
    Heat,
    Special,
    Mushroom,
    Deep,
    Biome,
    Edge,
    Noise,
    Hills,
    Rare,
    Shore,
    Smooth,
    River,
    Mix,
    Voronoi,
}
struct Layer {
    kind: Kind,
    seed: i64,
    parent: Option<Arc<Layer>>,
    other: Option<Arc<Layer>>,
}
pub struct Biomes {
    quarter: Arc<Layer>,
    blocks: Arc<Layer>,
}

impl Biomes {
    pub(super) fn find_position(
        &self,
        x: i32,
        z: i32,
        radius: i32,
        r: &mut super::JavaRandom,
        allowed: impl Fn(i32) -> bool,
    ) -> Option<(i32, i32)> {
        let (left, top) = ((x - radius) >> 2, (z - radius) >> 2);
        let width = (((x + radius) >> 2) - left + 1) as usize;
        let height = (((z + radius) >> 2) - top + 1) as usize;
        let mut found = None;
        let mut selections = 0;
        for (i, biome) in self
            .quarter(left, top, width, height)
            .into_iter()
            .enumerate()
        {
            if allowed(biome) && (found.is_none() || r.bounded(selections + 1) == 0) {
                found = Some((
                    (left + (i % width) as i32) * 4,
                    (top + (i / width) as i32) * 4,
                ));
                selections += 1;
            }
        }
        found
    }
    pub fn new(seed: i64) -> Self {
        use Kind::*;
        let make = |kind, salt, parent, other| {
            Arc::new(Layer {
                kind,
                seed: if salt == 0 { 0 } else { layer_seed(seed, salt) },
                parent,
                other,
            })
        };
        let chain = |mut p: Arc<Layer>, steps: &[(Kind, i64)]| {
            for &(kind, salt) in steps {
                p = make(kind, salt, Some(p), None);
            }
            p
        };
        let land = chain(
            make(Island, 1, None, None),
            &[
                (Fuzzy, 2000),
                (Land, 1),
                (Zoom, 2001),
                (Land, 2),
                (Land, 50),
                (Land, 70),
                (RemoveOcean, 2),
                (Snow, 2),
                (Land, 3),
                (Cool, 2),
                (Heat, 2),
                (Special, 3),
                (Zoom, 2002),
                (Zoom, 2003),
                (Land, 4),
                (Mushroom, 5),
                (Deep, 4),
            ],
        );
        let climate = chain(
            land.clone(),
            &[(Biome, 200), (Zoom, 1000), (Zoom, 1001), (Edge, 1000)],
        );
        let noise = chain(land, &[(Noise, 100)]);
        // GenLayerHills' secondary zooms never receive initWorldGenSeed in 1.8.
        let hill_noise = chain(noise.clone(), &[(Zoom, 0), (Zoom, 0)]);
        let hills = make(Hills, 1000, Some(climate), Some(hill_noise));
        let continent = chain(
            hills,
            &[
                (Rare, 1001),
                (Zoom, 1000),
                (Land, 3),
                (Zoom, 1001),
                (Shore, 1000),
                (Zoom, 1002),
                (Zoom, 1003),
                (Smooth, 1000),
            ],
        );
        let rivers = chain(
            noise,
            &[
                (Zoom, 1000),
                (Zoom, 1001),
                (Zoom, 1000),
                (Zoom, 1001),
                (Zoom, 1002),
                (Zoom, 1003),
                (River, 1),
                (Smooth, 1000),
            ],
        );
        let quarter = make(Mix, 100, Some(continent), Some(rivers));
        Self {
            blocks: make(Voronoi, 10, Some(quarter.clone()), None),
            quarter,
        }
    }
    pub fn quarter(&self, x: i32, z: i32, w: usize, h: usize) -> Vec<i32> {
        self.quarter.grid(x, z, w, h).values
    }
    pub fn blocks(&self, x: i32, z: i32, w: usize, h: usize) -> Vec<i32> {
        self.blocks.grid(x, z, w, h).values
    }
}

struct Grid {
    x: i32,
    z: i32,
    w: usize,
    values: Vec<i32>,
}
impl Grid {
    fn at(&self, x: i32, z: i32) -> i32 {
        self.values[(z - self.z) as usize * self.w + (x - self.x) as usize]
    }
    fn fill(x: i32, z: i32, w: usize, h: usize, mut f: impl FnMut(i32, i32) -> i32) -> Self {
        let mut values = Vec::with_capacity(w * h);
        for j in 0..h {
            for i in 0..w {
                values.push(f(x + i as i32, z + j as i32));
            }
        }
        Self { x, z, w, values }
    }
}
impl Layer {
    fn grid(&self, x: i32, z: i32, w: usize, h: usize) -> Grid {
        use Kind::*;
        let Some(parent) = &self.parent else {
            return Grid::fill(x, z, w, h, |i, j| {
                i32::from((i, j) == (0, 0) || CellRandom::new(self.seed, i, j).draw(10) == 0)
            });
        };
        if matches!(self.kind, Zoom | Fuzzy | Voronoi) {
            let voronoi = matches!(self.kind, Voronoi);
            let shift = if voronoi { 2 } else { 1 };
            let offset = if voronoi { 2 } else { 0 };
            let px = (x - offset) >> shift;
            let pz = (z - offset) >> shift;
            let pw = (((x - offset + w as i32 - 1) >> shift) - px + 2) as usize;
            let ph = (((z - offset + h as i32 - 1) >> shift) - pz + 2) as usize;
            let g = parent.grid(px, pz, pw, ph);
            return Grid::fill(x, z, w, h, |i, j| {
                let (u, v) = (i - offset, j - offset);
                let (a, b) = (u >> shift, v >> shift);
                let corners = [
                    g.at(a, b),
                    g.at(a + 1, b),
                    g.at(a, b + 1),
                    g.at(a + 1, b + 1),
                ];
                if voronoi {
                    return voronoi_cell(self.seed, u, v, corners);
                }
                let mut r = CellRandom::new(self.seed, a * 2, b * 2);
                let [nw, ne, sw, se] = corners;
                let west = [nw, sw][r.draw(2)];
                let north = [nw, ne][r.draw(2)];
                match (u & 1, v & 1) {
                    (0, 0) => nw,
                    (0, 1) => west,
                    (1, 0) => north,
                    _ => {
                        if !matches!(self.kind, Fuzzy)
                            && let Some(m) = mode(corners)
                        {
                            return m;
                        }
                        [nw, ne, sw, se][r.draw(4)]
                    }
                }
            });
        }
        let g = parent.grid(x - 1, z - 1, w + 2, h + 2);
        let other = self.other.as_ref().map(|p| p.grid(x, z, w, h));
        Grid::fill(x, z, w, h, |i, j| {
            let c = g.at(i, j);
            let sides = [
                g.at(i - 1, j),
                g.at(i + 1, j),
                g.at(i, j - 1),
                g.at(i, j + 1),
            ];
            let corners = [
                g.at(i - 1, j - 1),
                g.at(i + 1, j - 1),
                g.at(i - 1, j + 1),
                g.at(i + 1, j + 1),
            ];
            let mut r = CellRandom::new(self.seed, i, j);
            let second = || other.as_ref().map_or(0, |o| o.at(i, j));
            match self.kind {
                Land if c == 0 && corners.iter().any(|&n| n != 0) => {
                    let mut choice = 1;
                    for (n, v) in corners.into_iter().filter(|&v| v != 0).enumerate() {
                        if r.draw(n as i64 + 1) == 0 {
                            choice = v;
                        }
                    }
                    if r.draw(3) == 0 || choice == 4 {
                        choice
                    } else {
                        0
                    }
                }
                Land if c != 0 && corners.contains(&0) && r.draw(5) == 0 && c != 4 => 0,
                RemoveOcean if c == 0 && sides == [0; 4] && r.draw(2) == 0 => 1,
                Snow if c != 0 => match r.draw(6) {
                    0 => 4,
                    1 => 3,
                    _ => 1,
                },
                Cool if c == 1 && sides.iter().any(|v| matches!(v, 3 | 4)) => 2,
                Heat if c == 4 && sides.iter().any(|v| matches!(v, 1 | 2)) => 3,
                Special if c != 0 && r.draw(13) == 0 => c | ((1 + r.draw(15) as i32) << 8),
                Mushroom if c == 0 && corners == [0; 4] && r.draw(100) == 0 => 14,
                Deep if c == 0 && sides == [0; 4] => 24,
                Biome => climate(c, &mut r),
                Noise if c > 0 => 2 + r.draw(299999) as i32,
                Edge => edge(c, sides),
                Hills => hills(c, second(), sides, &mut r),
                Rare if c == 1 && r.draw(57) == 0 => 129,
                Shore => shore(c, sides),
                Smooth => match (sides[0] == sides[1], sides[2] == sides[3]) {
                    (true, true) => [sides[0], sides[2]][r.draw(2)],
                    (true, false) => sides[0],
                    (false, true) => sides[2],
                    _ => c,
                },
                River => {
                    let parity = |v| if v >= 2 { 2 + (v & 1) } else { v };
                    if sides.iter().all(|&n| parity(n) == parity(c)) {
                        -1
                    } else {
                        7
                    }
                }
                Mix if !matches!(c, 0 | 24) && second() == 7 => match c {
                    12 => 11,
                    14 | 15 => 15,
                    _ => 7,
                },
                _ => c,
            }
        })
    }
}

fn mode([a, b, c, d]: [i32; 4]) -> Option<i32> {
    if b == c && c == d {
        return Some(b);
    }
    for (v, x, y, z) in [(a, b, c, d), (b, c, a, d), (b, d, a, c), (c, d, a, b)] {
        if v == x && (y == v || z == v || y != z) {
            return Some(v);
        }
    }
    if (a == c && b != d) || (a == d && b != c) {
        return Some(a);
    }
    None
}
fn voronoi_cell(seed: i64, x: i32, z: i32, corners: [i32; 4]) -> i32 {
    let (a, b) = (x >> 2, z >> 2);
    let distances: [f64; 4] = std::array::from_fn(|n| {
        let (i, j) = ((n & 1) as i32, (n >> 1) as i32);
        let mut rng = CellRandom::new(seed, (a + i) << 2, (b + j) << 2);
        let mut delta = |k, local| {
            (rng.draw(1024) as f64 / 1024.0 - 0.5) * 3.6 + f64::from(k * 4) - f64::from(local)
        };
        let dx = delta(i, x & 3);
        let dz = delta(j, z & 3);
        dx * dx + dz * dz
    });
    for i in 0..3 {
        if (0..4).all(|j| i == j || distances[i] < distances[j]) {
            return corners[i];
        }
    }
    corners[3]
}
fn climate(c: i32, r: &mut CellRandom) -> i32 {
    let special = c & 0xf00 != 0;
    match c & !0xf00 {
        0 | 10 | 24 | 14 => c,
        1 if special => {
            if r.draw(3) == 0 {
                39
            } else {
                38
            }
        }
        1 => [2, 2, 2, 35, 35, 1][r.draw(6)],
        2 if special => 21,
        2 => [4, 29, 3, 1, 27, 6][r.draw(6)],
        3 if special => 32,
        3 => [4, 3, 5, 1][r.draw(4)],
        4 => [12, 12, 12, 30][r.draw(4)],
        _ => 14,
    }
}
fn category(v: i32) -> i32 {
    match v & 127 {
        2 | 17 => 2,
        3 | 20 | 34 => 3,
        4 | 18 | 27..=29 => 4,
        5 | 19 | 30..=33 => 5,
        12 | 13 => 12,
        21..=23 => 21,
        37..=39 => 37,
        0 | 10 | 24 => 0,
        14 | 15 => 14,
        7 | 11 => 7,
        35 | 36 => 35,
        16 | 26 => 16,
        n => n,
    }
}
fn similar(a: i32, b: i32) -> bool {
    if matches!(a, 38 | 39) {
        matches!(b, 38 | 39)
    } else {
        category(a) == category(b)
    }
}
fn edge(c: i32, s: [i32; 4]) -> i32 {
    match c {
        38 | 39 if !s.iter().all(|&v| similar(v, c)) => 37,
        32 if !s.iter().all(|&v| similar(v, c)) => 5,
        2 if s.contains(&12) => 34,
        6 if s.iter().any(|v| matches!(v, 2 | 30 | 12)) => 1,
        6 if s.contains(&21) => 23,
        _ => c,
    }
}
fn mutated(v: i32) -> Option<i32> {
    matches!(v,1..=6|12|21|23|27..=30|32..=39).then_some(v + 128)
}
fn hills(c: i32, noise: i32, s: [i32; 4], r: &mut CellRandom) -> i32 {
    let variant = (noise - 2) % 29;
    if variant == 1 && noise >= 2 && c != 0 {
        return mutated(c).unwrap_or(c);
    }
    if r.draw(3) != 0 && variant != 0 {
        return c;
    }
    let mut h = match c {
        2 => 17,
        4 => 18,
        27 => 28,
        29 => 1,
        5 => 19,
        32 => 33,
        30 => 31,
        1 => {
            if r.draw(3) == 0 {
                18
            } else {
                4
            }
        }
        12 => 13,
        21 => 22,
        0 => 24,
        3 => 34,
        35 => 36,
        38 | 39 => 37,
        24 if r.draw(3) == 0 => {
            if r.draw(2) == 0 {
                1
            } else {
                4
            }
        }
        _ => c,
    };
    if variant == 0 && h != c {
        h = mutated(h).unwrap_or(c);
    }
    if s.iter().filter(|&&v| similar(v, c)).count() >= 3 {
        h
    } else {
        c
    }
}
fn shore(c: i32, s: [i32; 4]) -> i32 {
    let sea = |v| matches!(v, 0 | 10 | 24);
    let coast = s.into_iter().any(sea);
    match c {
        14 => {
            if s.contains(&0) {
                15
            } else {
                c
            }
        }
        _ if category(c) == 21 => {
            if !s
                .iter()
                .all(|&v| category(v) == 21 || matches!(v, 4 | 5) || sea(v))
            {
                23
            } else if coast {
                16
            } else {
                c
            }
        }
        3 | 34 | 20 if coast => 25,
        12 | 13 | 140 | 30 | 31 | 158 if coast => 26,
        37 | 38 => {
            if !coast && s.iter().any(|&v| category(v) != 37) {
                2
            } else {
                c
            }
        }
        _ if coast && !matches!(c, 0 | 24 | 7 | 6) => 16,
        _ => c,
    }
}
