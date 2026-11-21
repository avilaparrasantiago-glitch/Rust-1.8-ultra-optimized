use crate::arena::{Column, Section, Template};
use rayon::prelude::*;
use std::{collections::BTreeMap, sync::OnceLock};

const SEA: u8 = 62;
mod population;
// One nibble per block during section packing; entries are protocol 47 states.
const STATES: [u16; 16] = [
    0,
    1 << 4,
    3 << 4,
    2 << 4,
    12 << 4,
    9 << 4,
    7 << 4,
    24 << 4,
    16 << 4,
    15 << 4,
    14 << 4,
    56 << 4,
    17 << 4,
    18 << 4,
    81 << 4,
    4 << 4,
];

#[derive(Clone, Copy)]
struct Surface {
    height: u8,
    biome: u8,
}

pub(crate) struct Generator(pub(crate) u64);

impl Generator {
    fn hash(&self, x: i64, z: i64, layer: u64) -> u64 {
        let mut v = self.0
            ^ layer.wrapping_mul(0x632be59bd9b4e019)
            ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (z as u64).wrapping_mul(0xd6e8feb86659fd93);
        v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
        v ^ (v >> 31)
    }

    fn corner(&self, x: i64, z: i64, layer: u64) -> f64 {
        (self.hash(x, z, layer) >> 11) as f64 / ((1u64 << 52) as f64) - 1.
    }

    fn noise(&self, x: i64, z: i64, scale: i64, layer: u64) -> f64 {
        let fade = |v: i64| {
            let t = v.rem_euclid(scale) as f64 / scale as f64;
            t * t * (3. - 2. * t)
        };
        let (a, b) = (x.div_euclid(scale), z.div_euclid(scale));
        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let (u, v) = (fade(x), fade(z));
        lerp(
            lerp(self.corner(a, b, layer), self.corner(a + 1, b, layer), u),
            lerp(
                self.corner(a, b + 1, layer),
                self.corner(a + 1, b + 1, layer),
                u,
            ),
            v,
        )
    }

    fn surface(&self, x: i64, z: i64) -> Surface {
        let continent = self.noise(x, z, 224, 1);
        let hills = self.noise(x, z, 96, 2).max(0.);
        let detail = self.noise(x, z, 23, 3);
        let land = 68. + 30. * continent + 34. * hills * hills + 3. * detail;
        let channel = (self.noise(x, z, 144, 4) + 0.16 * self.noise(x, z, 49, 5)).abs();
        let bank = (channel / 0.18).min(1.);
        let bed = land.min(57. + detail);
        let carved = bed + (land - bed) * bank * bank * (3. - 2. * bank);
        // Guarantee a dry starting point, including seeds with an ocean at the origin.
        let shore = 67. - 0.35 * (x as f64).hypot(z as f64);
        let height = carved.max(shore).round() as u8;
        let biome = if height < SEA {
            if land >= f64::from(SEA) { 7 } else { 0 }
        } else if height <= SEA + 2 {
            16
        } else if hills > 0.55 && height > 83 {
            3
        } else {
            let climate = self.noise(x, z, 151, 6);
            if climate > 0.42 {
                2
            } else if climate < -0.12 {
                4
            } else {
                1
            }
        };
        Surface { height, biome }
    }

    pub(crate) fn column(&self, cx: i32, cz: i32) -> Column {
        let surface: [Surface; 256] = std::array::from_fn(|i| {
            self.surface(
                i64::from(cx) * 16 + (i % 16) as i64,
                i64::from(cz) * 16 + (i / 16) as i64,
            )
        });
        let highest = surface.iter().fold(SEA, |h, s| h.max(s.height));
        // Reserve headroom for features anchored just outside this chunk.
        let mut layers = vec![[0u8; 256]; usize::from(highest + 9).next_multiple_of(16)];
        for (i, s) in surface.iter().enumerate() {
            let sandy = matches!(s.biome, 0 | 2 | 7 | 16);
            let x = i64::from(cx) * 16 + (i % 16) as i64;
            let z = i64::from(cz) * 16 + (i / 16) as i64;
            let caves = self.cave_span(x, z);
            for (y, layer) in layers
                .iter_mut()
                .enumerate()
                .take(usize::from(s.height.max(SEA)) + 1)
            {
                let y = y as u8;
                layer[i] = if y == 0 {
                    6
                } else if y > s.height {
                    5
                } else if y == s.height {
                    if sandy { 4 } else { 3 }
                } else if s.height - y <= 3 {
                    if sandy { 4 } else { 2 }
                } else if sandy && s.height - y <= 5 {
                    7
                } else if caves.contains(&y) {
                    0
                } else {
                    1
                };
            }
        }
        self.populate(cx, cz, &surface, &mut layers);
        let mut sections = BTreeMap::new();
        let mut sky = [15u8; 256];
        for (sy, slice) in layers.as_chunks::<16>().0.iter().enumerate().rev() {
            let blocks = slice.as_slice().as_flattened();
            if blocks.iter().all(|&v| v == 0) && sections.is_empty() {
                continue;
            }
            let mut light = Box::new([[0u8; 2048]; 2]);
            for (y, row) in slice.iter().enumerate().rev() {
                for (i, &code) in row.iter().enumerate() {
                    sky[i] = match code {
                        0 => sky[i],
                        5 => sky[i].saturating_sub(3),
                        13 => sky[i].saturating_sub(1),
                        _ => 0,
                    };
                    light[1][y * 128 + i / 2] |= sky[i] << (4 * (i % 2));
                }
            }
            let (palette, indices) = if blocks.iter().all(|&b| b == blocks[0]) {
                (vec![STATES[blocks[0] as usize]], Vec::new())
            } else {
                let packed = blocks
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&[a, b]| a | b << 4)
                    .collect();
                (STATES.to_vec(), packed)
            };
            sections.insert(
                sy as u8,
                Section {
                    palette,
                    indices,
                    light: Some(light),
                },
            );
        }
        Column {
            skylight: true,
            sections,
            biomes: surface.map(|s| s.biome),
            packet: OnceLock::new(),
            nbt: Vec::new(),
        }
    }
}

impl Template {
    pub(crate) fn dimension(
        g: &crate::overworld::DimensionTerrain,
        radius: i32,
        dim: crate::overworld::Dimension,
    ) -> Self {
        let end = dim == crate::overworld::Dimension::End;
        let center = if end { 100 >> 4 } else { 0 };
        let mut template = Self {
            spawn_hint: None,
            columns: (center - radius..=center + radius)
                .into_par_iter()
                .flat_map_iter(|x| (-radius..=radius).map(move |z| ((x, z), g.column(x, z))))
                .collect(),
        };
        template.spawn_hint = if end {
            Some([100, 49, 0])
        } else {
            (0..16)
                .flat_map(|z| (0..16).map(move |x| (x, z)))
                .find_map(|(x, z)| {
                    (32..120u8)
                        .rev()
                        .find(|&y| {
                            matches!(template.block((x, y, z)) >> 4, 87 | 88 | 13)
                                && template.block((x, y + 1, z)) == 0
                                && template.block((x, y + 2, z)) == 0
                        })
                        .map(|y| [x, i32::from(y) + 1, z])
                })
        };
        template
    }
    pub fn overworld(seed: i64, radius: i32) -> Self {
        Self::overworld_revision(seed, radius, 2)
    }
    pub(crate) fn overworld_revision(seed: i64, radius: i32, revision: u32) -> Self {
        let mut provider = crate::overworld::Overworld::new(seed);
        provider.structures = revision >= 2;
        let spawn = provider.spawn();
        let (sx, sz) = (spawn[0] >> 4, spawn[2] >> 4);
        let columns = (sx - radius..=sx + radius)
            .into_par_iter()
            .flat_map_iter(|cx| {
                let provider = &provider;
                (sz - radius..=sz + radius).map(move |cz| ((cx, cz), provider.column(cx, cz)))
            })
            .collect();
        Self {
            spawn_hint: Some(spawn),
            columns,
        }
    }
    pub fn normal(seed: i64, radius: i32) -> Self {
        let generator = Generator(seed as u64);
        let columns = (-radius..=radius)
            .into_par_iter()
            .flat_map_iter(|cx| {
                let generator = &generator;
                (-radius..=radius).map(move |cz| ((cx, cz), generator.column(cx, cz)))
            })
            .collect();
        Self {
            spawn_hint: None,
            columns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_blocks_follow_surface_across_negative_chunk_seams() {
        let g = Generator(9581);
        for (cx, cz) in [(-1, -1), (0, -1), (1, -1), (-2, -1), (-1, 0)] {
            let col = g.column(cx, cz);
            for z in 0..16 {
                for x in 0..16 {
                    let wx = i64::from(cx) * 16 + x as i64;
                    let wz = i64::from(cz) * 16 + z as i64;
                    let s = g.surface(wx, wz);
                    let idx = z * 16 + x;
                    assert_eq!(col.biomes[idx], s.biome);
                    let top = col.sections[&(s.height / 16)]
                        .state(usize::from(s.height % 16) * 256 + idx);
                    assert!(matches!(top, 32 | 192));
                    assert!(s.height.abs_diff(g.surface(wx, wz - 1).height) <= 5);
                    assert!(s.height.abs_diff(g.surface(wx - 1, wz).height) <= 5);
                }
            }
        }
    }

    #[test]
    fn underground_population_and_dry_spawn() {
        let world = Template::normal(9581, 1);
        let mut cave = false;
        let mut ore = false;
        for x in -16..16 {
            for z in -16..16 {
                for y in 3..32 {
                    let id = world.block((x, y, z)) >> 4;
                    cave |= id == 0;
                    ore |= matches!(id, 14 | 15 | 16 | 56);
                }
            }
        }
        assert!(cave && ore);
        let [_, y, _] = world.spawn().unwrap();
        let height = Generator(9581).surface(0, 0).height;
        assert_eq!(y, f64::from(height + 1));
        for level in [height + 1, height + 2] {
            assert_eq!(world.block((0, level, 0)), 0);
        }
    }

    #[test]
    fn seed_and_load_order() {
        let positions = [(-1, 1), (0, -1), (1, 0)];
        let generate = |order: &[(i32, i32)]| Template {
            spawn_hint: None,
            columns: order
                .iter()
                .map(|&(x, z)| ((x, z), Generator((-42i64) as u64).column(x, z)))
                .collect(),
        };
        let first = generate(&positions);
        let mut reverse = positions;
        reverse.reverse();
        let second = generate(&reverse);
        for pos in positions {
            assert_eq!(
                first.packet(pos).unwrap().body(),
                second.packet(pos).unwrap().body()
            );
        }
        let small = Template::normal(-42, 1);
        let large = Template::normal(-42, 2);
        let other = Template::normal(91, 1);
        assert_eq!(
            small.packet((-1, 1)).unwrap().body(),
            large.packet((-1, 1)).unwrap().body()
        );
        assert_ne!(
            small.packet((-1, 1)).unwrap().body(),
            other.packet((-1, 1)).unwrap().body()
        );
        let g = Generator(9581);
        let mut found = [false; 17];
        for z in (-768..768).step_by(13) {
            for x in (-768..768).step_by(13) {
                found[g.surface(x, z).biome as usize] = true;
            }
        }
        for biome in [0, 1, 2, 3, 4, 7, 16] {
            assert!(found[biome], "missing biome {biome}");
        }
    }
}
