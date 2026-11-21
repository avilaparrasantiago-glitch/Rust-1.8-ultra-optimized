use super::{JavaRandom, Octaves};
#[derive(Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Dimension {
    Nether,
    End,
}
pub struct DimensionTerrain {
    populated: bool,
    spikes: std::sync::OnceLock<Vec<super::EndSpike>>,
    seed: i64,
    surface: Option<(Octaves, Octaves)>,
    kind: Dimension,
    lower: Octaves,
    upper: Octaves,
    selector: Octaves,
}
impl DimensionTerrain {
    pub fn provide(&self, cx: i32, cz: i32) -> Vec<u16> {
        let mut blocks = self.terrain(cx, cz);
        if let Some((patches, thickness)) = &self.surface {
            let mut sand = [0.; 256];
            let mut gravel = sand;
            let mut depth = sand;
            patches.fill(
                &mut sand,
                [cx * 16, cz * 16, 0],
                [16, 16, 1],
                [0.03125, 0.03125, 1.],
            );
            patches.fill(
                &mut gravel,
                [cx * 16, 109, cz * 16],
                [16, 1, 16],
                [0.03125, 1., 0.03125],
            );
            thickness.fill(&mut depth, [cx * 16, cz * 16, 0], [16, 16, 1], [0.0625; 3]);
            let mut r = JavaRandom::new(
                i64::from(cx)
                    .wrapping_mul(341873128712)
                    .wrapping_add(i64::from(cz).wrapping_mul(132897987541)),
            );
            for z in 0..16 {
                for x in 0..16 {
                    let i = x + 16 * z;
                    let noise_index = z + x * 16;
                    let sandy = sand[noise_index] + r.double() * 0.2 > 0.;
                    let rocky = gravel[noise_index] + r.double() * 0.2 > 0.;
                    let thick = (depth[noise_index] / 3. + 3. + r.double() * 0.25) as i32;
                    let (mut remaining, mut top, mut fill) = (-1, 87 << 4, 87 << 4);
                    for y in (0..128).rev() {
                        let b = &mut blocks[y * 256 + i];
                        let y = y as i32;
                        if y >= 127 - r.bounded(5) || y <= r.bounded(5) {
                            *b = 7 << 4;
                            continue;
                        }
                        if *b == 0 {
                            remaining = -1;
                            continue;
                        }
                        if *b != 87 << 4 {
                            continue;
                        }
                        if remaining == -1 {
                            if thick <= 0 {
                                top = 0;
                                fill = 87 << 4;
                            } else if (60..=65).contains(&y) {
                                top = if sandy {
                                    88 << 4
                                } else if rocky {
                                    13 << 4
                                } else {
                                    87 << 4
                                };
                                fill = if sandy { 88 << 4 } else { 87 << 4 };
                            }
                            if y < 64 && top == 0 {
                                top = 11 << 4;
                            }
                            remaining = thick;
                            *b = if y >= 63 { top } else { fill };
                        } else if remaining > 0 {
                            remaining -= 1;
                            *b = fill;
                        }
                    }
                }
            }
            super::Carver::Nether.carve(self.seed, cx, cz, &mut blocks, &[]);
        }
        blocks
    }
    pub(crate) fn column(&self, cx: i32, cz: i32) -> crate::arena::Column {
        let mut blocks = self.provide(cx, cz);
        if self.kind == Dimension::Nether && self.populated {
            super::fortress::paint_column(self.seed, cx, cz, &mut blocks);
        }
        for spike in self.spikes() {
            spike.write_column(cx, cz, &mut blocks);
        }
        let mut col = super::column::pack(
            &blocks,
            [if matches!(self.kind, Dimension::End) {
                9
            } else {
                8
            }; 256],
        );
        col.skylight = false;
        // A ground-up 0x21 with mask zero means unload to a protocol 47 client.
        if col.sections.is_empty() {
            col.sections.insert(
                0,
                crate::arena::Section {
                    palette: vec![0],
                    indices: Vec::new(),
                    light: None,
                },
            );
        }
        col
    }
    pub fn new(seed: i64, kind: Dimension) -> Self {
        let mut r = JavaRandom::new(seed);
        Self {
            populated: true,
            spikes: Default::default(),
            seed,
            kind,
            lower: Octaves::from_random(&mut r, 16),
            upper: Octaves::from_random(&mut r, 16),
            selector: Octaves::from_random(&mut r, 8),
            surface: matches!(kind, Dimension::Nether).then(|| {
                (
                    Octaves::from_random(&mut r, 4),
                    Octaves::from_random(&mut r, 4),
                )
            }),
        }
    }
    pub fn spikes(&self) -> &[super::EndSpike] {
        if self.kind == Dimension::End {
            self.spikes
                .get_or_init(|| super::spikes::island_spikes(self, self.seed))
        } else {
            &[]
        }
    }
    pub(crate) fn use_undecorated_terrain(&mut self) {
        self.populated = false;
        self.spikes.get_or_init(Vec::new);
    }
    /// Unpopulated primer in section wire order, before surface replacement or carving.
    pub fn terrain(&self, cx: i32, cz: i32) -> Vec<u16> {
        let end = matches!(self.kind, Dimension::End);
        let (horizontal, vertical, scale_x, scale_y) = if end {
            (2, 32, 1368.824, 684.412)
        } else {
            (4, 16, 684.412, 2053.236)
        };
        let nx = horizontal + 1;
        let ny = vertical + 1;
        let origin = [cx * horizontal as i32, 0, cz * horizontal as i32];
        let size = [nx, ny, nx];
        let mut field = vec![0.; nx * nx * ny];
        let mut high = field.clone();
        let mut blend = field.clone();
        self.lower
            .fill(&mut field, origin, size, [scale_x, scale_y, scale_x]);
        self.upper
            .fill(&mut high, origin, size, [scale_x, scale_y, scale_x]);
        self.selector.fill(
            &mut blend,
            origin,
            size,
            [
                scale_x / 80.,
                scale_y / if end { 160. } else { 60. },
                scale_x / 80.,
            ],
        );
        for x in 0..nx {
            for z in 0..nx {
                let dx = (origin[0] + x as i32) as f32;
                let dz = (origin[2] + z as i32) as f32;
                let island = f64::from((100. - (dx * dx + dz * dz).sqrt() * 8.).clamp(-100., 80.));
                for y in 0..ny {
                    let i = (x * nx + z) * ny + y;
                    let a = field[i] / 512.;
                    let b = high[i] / 512.;
                    let t = (blend[i] / 10. + 1.) / 2.;
                    let mut v = if t < 0. {
                        a
                    } else if t > 1. {
                        b
                    } else {
                        a + (b - a) * t
                    };
                    if end {
                        v = v - 8. + island;
                        if y > 14 {
                            let t = f64::from((y - 14) as f32 / 64.);
                            v = v * (1. - t) - 3000. * t;
                        }
                        if y < 8 {
                            let t = f64::from((8 - y) as f32 / 7.);
                            v = v * (1. - t) - 30. * t;
                        }
                    } else {
                        let mut bias =
                            (y as f64 * std::f64::consts::PI * 6. / ny as f64).cos() * 2.;
                        let distance = y.min(ny - 1 - y);
                        if distance < 4 {
                            bias -= ((4 - distance) as f64).powi(3) * 10.;
                        }
                        v -= bias;
                        if y > ny - 4 {
                            let t = f64::from((y - (ny - 4)) as f32 / 3.);
                            v = v * (1. - t) - 10. * t;
                        }
                    }
                    field[i] = v;
                }
            }
        }
        let (sx, sy) = (16 / horizontal, 128 / vertical);
        let mut blocks = vec![0; 65536];
        for x in 0..horizontal {
            for z in 0..horizontal {
                for y in 0..vertical {
                    let starts = [
                        (x * nx + z) * ny + y,
                        (x * nx + z + 1) * ny + y,
                        ((x + 1) * nx + z) * ny + y,
                        ((x + 1) * nx + z + 1) * ny + y,
                    ];
                    let mut edges = starts.map(|i| field[i]);
                    let dy = starts.map(|i| (field[i + 1] - field[i]) / sy as f64);
                    for iy in 0..sy {
                        let (mut a, mut b) = (edges[0], edges[1]);
                        let da = (edges[2] - a) / sx as f64;
                        let db = (edges[3] - b) / sx as f64;
                        for ix in 0..sx {
                            let mut v = a;
                            let dz = (b - a) / sx as f64;
                            for iz in 0..sx {
                                let height = y * sy + iy;
                                blocks[height * 256 + (z * sx + iz) * 16 + x * sx + ix] = if v > 0.
                                {
                                    if end { 121 << 4 } else { 87 << 4 }
                                } else if !end && height < 32 {
                                    11 << 4
                                } else {
                                    0
                                };
                                v += dz;
                            }
                            a += da;
                            b += db;
                        }
                        for k in 0..4 {
                            edges[k] += dy[k];
                        }
                    }
                }
            }
        }
        blocks
    }
}
