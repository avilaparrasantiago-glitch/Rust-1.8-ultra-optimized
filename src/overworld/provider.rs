use super::{Biomes, JavaRandom, Octaves, simplex::Simplex};

pub struct Overworld {
    pub(crate) structures: bool,
    pub(super) seed: i64,
    pub biomes: Biomes,
    lower: Octaves,
    upper: Octaves,
    selector: Octaves,
    surface: [Simplex; 4],
    depth: Octaves,
    swamp: Simplex,
    mesa: super::mesa::Mesa,
}
fn shape(id: i32) -> (f32, f32) {
    match id {
        0 | 10 => (-1., 0.1),
        1 | 2 | 12 | 35 | 129 => (0.125, 0.05),
        3 | 34 | 131 | 162 => (1., 0.5),
        5 | 30 | 32 | 160 | 161 => (0.2, 0.2),
        6 => (-0.2, 0.1),
        7 | 11 => (-0.5, 0.),
        13 | 17..=19 | 22 | 28 | 31 | 33 | 166 | 167 => (0.45, 0.3),
        14 => (0.2, 0.3),
        15 | 16 | 26 => (0., 0.025),
        20 => (0.8, 0.3),
        24 => (-1.8, 0.1),
        25 => (0.1, 0.8),
        36 | 38 | 39 => (1.5, 0.025),
        130 => (0.225, 0.25),
        132 => (0.1, 0.4),
        133 | 158 => (0.3, 0.4),
        134 => (-0.1, 0.3),
        140 => (0.425, 0.45000002),
        149 | 151 | 155 | 157 => (0.2, 0.4),
        156 => (0.55, 0.5),
        163 => (0.3625, 1.225),
        164 => (1.05, 1.2125001),
        _ => (0.1, 0.2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn density_matches_official_doubles() {
        let g = Overworld::new(12345);
        for (x, expected) in [
            (
                0,
                "3f1b4749409a38145302cee7713c4b202fdaab6c5a424a3034b644fcfc4966c1",
            ),
            (
                -7,
                "6d4ab3982e50560b03fc3ab6e43e9c136a0b1370473ccf2f0d239485ee7c444e",
            ),
        ] {
            let mut sum = Sha256::new();
            for v in g.density(x, 0) {
                sum.update(v.to_be_bytes());
            }
            assert_eq!(format!("{:x}", sum.finalize()), expected);
        }
    }
}
impl Overworld {
    /// Chunk coordinates of the three DEFAULT stronghold starts in generation order.
    pub fn stronghold_starts(&self) -> [(i32, i32); 3] {
        let mut r = JavaRandom::new(self.seed);
        let mut angle = r.double() * std::f64::consts::PI * 2.;
        std::array::from_fn(|_| {
            let radius = (1.25 + r.double()) * 32.;
            let cx = (angle.cos() * radius + 0.5).floor() as i32;
            let cz = (angle.sin() * radius + 0.5).floor() as i32;
            let selected = self
                .biomes
                .find_position(cx * 16 + 8, cz * 16 + 8, 112, &mut r, |id| shape(id).0 > 0.);
            angle += std::f64::consts::TAU / 3.;
            selected.map_or((cx, cz), |(x, z)| (x >> 4, z >> 4))
        })
    }
    pub fn new(seed: i64) -> Self {
        let mut rng = JavaRandom::new(seed);
        let lower = Octaves::from_random(&mut rng, 16);
        let upper = Octaves::from_random(&mut rng, 16);
        let selector = Octaves::from_random(&mut rng, 8);
        let surface = std::array::from_fn(|_| Simplex::new(&mut rng));
        let _scale = Octaves::from_random(&mut rng, 10);
        let depth = Octaves::from_random(&mut rng, 16);
        Self {
            seed,
            structures: true,
            biomes: Biomes::new(seed),
            lower,
            upper,
            selector,
            surface,
            depth,
            swamp: Simplex::new(&mut JavaRandom::new(2345)),
            mesa: super::mesa::Mesa::new(seed),
        }
    }
    fn density(&self, cx: i32, cz: i32) -> [f64; 825] {
        let origin = [cx * 4, 0, cz * 4];
        let biomes = self.biomes.quarter(origin[0] - 2, origin[2] - 2, 10, 10);
        let mut depth = [0.; 25];
        self.depth
            .fill(&mut depth, origin, [5, 1, 5], [200., 1., 200.]);
        let mut low = [0.; 825];
        let mut high = low;
        let mut mix = low;
        let scale = 684.412_f32;
        self.lower
            .fill(&mut low, origin, [5, 33, 5], [f64::from(scale); 3]);
        self.upper
            .fill(&mut high, origin, [5, 33, 5], [f64::from(scale); 3]);
        self.selector.fill(
            &mut mix,
            origin,
            [5, 33, 5],
            [scale / 80., scale / 160., scale / 80.].map(f64::from),
        );
        for x in 0..5 {
            for z in 0..5 {
                let center = shape(biomes[x + 2 + (z + 2) * 10]).0;
                let (mut base, mut variation, mut weights) = (0f32, 0f32, 0f32);
                for dx in 0..5 {
                    for dz in 0..5 {
                        let (b, v) = shape(biomes[x + dx + (z + dz) * 10]);
                        let r = (dx as i32 - 2).pow(2) + (dz as i32 - 2).pow(2);
                        let mut w = 10.0 / (r as f32 + 0.2).sqrt() / (b + 2.0);
                        if b > center {
                            w /= 2.0;
                        }
                        base += b * w;
                        variation += v * w;
                        weights += w;
                    }
                }
                let mut d = depth[x * 5 + z] / 8000.;
                if d < 0. {
                    d = -d * 0.3;
                }
                d = d * 3. - 2.;
                d = if d < 0. {
                    (d / 2.).max(-1.) / 1.4 / 2.
                } else {
                    d.min(1.) / 8.
                };
                let v = f64::from(variation / weights * 0.9 + 0.1);
                let b = f64::from((base / weights * 4. - 1.) / 8.);
                let level = 8.5 + (b + d * 0.2) * 8.5 / 8. * 4.;
                for y in 0..33 {
                    let i = (x * 5 + z) * 33 + y;
                    let mut dy = (y as f64 - level) * 12. * 128. / 256. / v;
                    if dy < 0. {
                        dy *= 4.;
                    }
                    let a = low[i] / 512.;
                    let b = high[i] / 512.;
                    let t = (mix[i] / 10. + 1.) / 2.;
                    let mut n = if t < 0. {
                        a
                    } else if t > 1. {
                        b
                    } else {
                        a + (b - a) * t
                    } - dy;
                    if y > 29 {
                        let t = f64::from((y - 29) as f32 / 3.);
                        n = n * (1. - t) - 10. * t;
                    }
                    low[i] = n;
                }
            }
        }
        low
    }
    pub fn set_blocks_in_chunk(&self, cx: i32, cz: i32) -> Vec<u16> {
        let field = self.density(cx, cz);
        let mut states = vec![0; 65536];
        for gx in 0..4 {
            for gz in 0..4 {
                for gy in 0..32 {
                    let indices = [
                        (gx * 5 + gz) * 33 + gy,
                        (gx * 5 + gz + 1) * 33 + gy,
                        ((gx + 1) * 5 + gz) * 33 + gy,
                        ((gx + 1) * 5 + gz + 1) * 33 + gy,
                    ];
                    let mut c = indices.map(|i| field[i]);
                    let step = indices.map(|i| (field[i + 1] - field[i]) * 0.125);
                    for iy in 0..8 {
                        let y = gy * 8 + iy;
                        let (mut a, mut b) = (c[0], c[1]);
                        let (da, db) = ((c[2] - a) * 0.25, (c[3] - b) * 0.25);
                        for ix in 0..4 {
                            let dz = (b - a) * 0.25;
                            let mut v = a - dz;
                            for iz in 0..4 {
                                v += dz;
                                states[y * 256 + (gz * 4 + iz) * 16 + gx * 4 + ix] = if v > 0. {
                                    16
                                } else if y < 63 {
                                    9 << 4
                                } else {
                                    0
                                };
                            }
                            a += da;
                            b += db;
                        }
                        for k in 0..4 {
                            c[k] += step[k];
                        }
                    }
                }
            }
        }
        states
    }
    pub fn replace_blocks_for_biome(&self, cx: i32, cz: i32, biomes: &[i32], states: &mut [u16]) {
        let mut rng = JavaRandom::new(
            i64::from(cx)
                .wrapping_mul(341873128712)
                .wrapping_add(i64::from(cz).wrapping_mul(132897987541)),
        );
        for z in 0..16 {
            for x in 0..16 {
                let id = biomes[z * 16 + x];
                let (wx, wz) = (cx * 16 + x as i32, cz * 16 + z as i32);
                let mut frequency = 1.0;
                let mut noise = 0.;
                for octave in &self.surface {
                    noise += octave.shifted(
                        f64::from(wx) * (0.0625 * frequency),
                        f64::from(wz) * (0.0625 * frequency),
                    ) * (0.55 / frequency);
                    frequency *= 0.5;
                }
                // BiomeGenBase receives transposed world coordinates and writes Z,X into the primer.
                let (bx, bz) = (cx * 16 + z as i32, cz * 16 + x as i32);
                if matches!(id,37..=39|165..=167) {
                    self.mesa.surface(states, id, [bx, bz], noise, &mut rng);
                    continue;
                }
                if matches!(id, 6 | 134) {
                    let wet = self
                        .swamp
                        .sample(f64::from(bx) * 0.25, f64::from(bz) * 0.25);
                    if wet > 0.0
                        && let Some(y) = (0..256).rev().find(|&y| states[y * 256 + z * 16 + x] != 0)
                        && y == 62
                        && states[y * 256 + z * 16 + x] != 9 << 4
                    {
                        states[y * 256 + z * 16 + x] = 9 << 4;
                        if wet < 0.12 {
                            states[(y + 1) * 256 + z * 16 + x] = 111 << 4;
                        }
                    }
                }
                let (grass, dirt) = match id {
                    2 | 17 | 16 | 26 | 130 => (12 << 4, 12 << 4),
                    14 | 15 => (110 << 4, 3 << 4),
                    25 => (16, 16),
                    140 => (80 << 4, 3 << 4),
                    131 | 162 if !(-1.0..=2.0).contains(&noise) => (13 << 4, 13 << 4),
                    3 | 20 | 131 | 162 if noise > 1. => (16, 16),
                    32 | 33 | 160 | 161 if noise > 1.75 => (49, 48),
                    32 | 33 | 160 | 161 if noise > -0.95 => (50, 48),
                    163 | 164 if noise > 1.75 => (16, 16),
                    163 | 164 if noise > -0.5 => (49, 48),
                    _ => (32, 48),
                };
                let (mut top, mut filler) = (grass, dirt);
                let thickness = (noise / 3. + 3. + rng.double() * 0.25) as i32;
                let mut remain = -1;
                for y in (0..256).rev() {
                    let value = &mut states[y * 256 + z * 16 + x];
                    let y = y as i32;
                    if y <= rng.bounded(5) {
                        *value = 7 << 4;
                        continue;
                    }
                    if *value == 0 {
                        remain = -1;
                        continue;
                    }
                    if *value != 16 {
                        continue;
                    }
                    if remain == -1 {
                        if thickness <= 0 {
                            top = 0;
                            filler = 16;
                        } else if (59..=64).contains(&y) {
                            top = grass;
                            filler = dirt;
                        }
                        if y < 63 && top == 0 {
                            top = if matches!(id, 10..=13 | 26 | 30 | 31 | 140 | 158) {
                                79 << 4
                            } else {
                                9 << 4
                            };
                        }
                        remain = thickness;
                        *value = if y >= 62 {
                            top
                        } else if y < 56 - thickness {
                            top = 0;
                            filler = 16;
                            13 << 4
                        } else {
                            filler
                        };
                    } else if remain > 0 {
                        remain -= 1;
                        *value = filler;
                        if remain == 0 && filler == 12 << 4 {
                            remain = rng.bounded(4) + (y - 63).max(0);
                            filler = 24 << 4;
                        }
                    }
                }
            }
        }
    }
}
