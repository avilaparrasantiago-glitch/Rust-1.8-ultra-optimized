use super::{Generator, Surface};
use std::ops::Range;

impl Generator {
    // A winding tunnel follows a noise contour. Its floor changes continuously
    // in world coordinates, including at the seam between negative chunks.
    pub(super) fn cave_span(&self, x: i64, z: i64) -> Range<u8> {
        let distance = self.noise(x, z, 31, 20).abs();
        let center = (22. + 13. * self.noise(x, z, 47, 21)).round() as u8;
        let radius = (4. - 14. * distance).floor() as i8;
        if radius < 0 {
            return 0..0;
        }
        center - radius as u8..center + radius as u8 + 1
    }

    pub(super) fn populate(
        &self,
        cx: i32,
        cz: i32,
        surface: &[Surface; 256],
        layers: &mut Vec<[u8; 256]>,
    ) {
        let ox = i64::from(cx) * 16;
        let oz = i64::from(cz) * 16;
        // Replay candidate anchors from the halo, clipping writes to our column.
        // Stable ordering also makes overlapping features independent of load order.
        for (spacing, kind) in [(8, 0), (8, 1), (48, 2)] {
            for gz in (oz - 2).div_euclid(spacing)..=(oz + 17).div_euclid(spacing) {
                for gx in (ox - 2).div_euclid(spacing)..=(ox + 17).div_euclid(spacing) {
                    let hash = self.hash(gx, gz, 30 + kind);
                    let x = gx * spacing + (hash & 65535) as i64 % spacing;
                    let z = gz * spacing + ((hash >> 16) & 65535) as i64 % spacing;
                    if x < ox - 2 || x > ox + 17 || z < oz - 2 || z > oz + 17 {
                        continue;
                    }
                    let Some(feature) = self.feature(x, z, kind, (hash >> 32) as u32) else {
                        continue;
                    };
                    // A canopy anchored above a steep chunk edge can exceed the
                    // highest local surface. Keep its full height on both sides.
                    if let Feature::Oak(base) = feature {
                        let required = (base + 7).next_multiple_of(16);
                        if layers.len() < required {
                            layers.resize(required, [0; 256]);
                        }
                    }
                    for dz in -2i64..=2 {
                        for dx in -2i64..=2 {
                            let (lx, lz) = (x + dx - ox, z + dz - oz);
                            if !(0..16).contains(&lx) || !(0..16).contains(&lz) {
                                continue;
                            }
                            let i = (lz * 16 + lx) as usize;
                            let ground = usize::from(surface[i].height);
                            feature.place(layers, i, ground, dx, dz);
                        }
                    }
                }
            }
        }
    }

    fn feature(&self, x: i64, z: i64, kind: u64, roll: u32) -> Option<Feature> {
        if kind == 0 {
            let (code, limit) = match roll % 12 {
                0 => (11, 13),
                1 => (10, 28),
                2..=5 => (9, 48),
                _ => (8, 59),
            };
            return Some(Feature::Vein {
                code,
                y: 3 + (roll >> 8) as usize % limit,
            });
        }
        if x.abs() <= 5 && z.abs() <= 5 {
            return None;
        }
        let anchor = self.surface(x, z);
        let base = usize::from(anchor.height);
        match (kind, anchor.biome) {
            (1, 4) => Some(Feature::Oak(base)),
            (1, 1 | 3) if roll.is_multiple_of(7) => Some(Feature::Oak(base)),
            (1, 2) if roll.is_multiple_of(3) => Some(Feature::Cactus(base)),
            (2, 1 | 2)
                if roll.is_multiple_of(3)
                    && (-2..=2).all(|dx| {
                        (-2..=2).all(|dz| {
                            self.surface(x + dx, z + dz).height.abs_diff(anchor.height) <= 1
                        })
                    }) =>
            {
                Some(Feature::Ruin {
                    code: if anchor.biome == 2 { 7 } else { 15 },
                    base,
                })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Feature {
    Vein { code: u8, y: usize },
    Oak(usize),
    Cactus(usize),
    Ruin { code: u8, base: usize },
}

impl Feature {
    fn place(self, layers: &mut [[u8; 256]], i: usize, ground: usize, dx: i64, dz: i64) {
        match self {
            Self::Vein { code, y: vein_y } => {
                for y in vein_y - 1..=vein_y + 1 {
                    if dx * dx + dz * dz + 2 * y.abs_diff(vein_y) as i64 <= 4
                        && let Some(row) = layers.get_mut(y)
                        && row[i] == 1
                    {
                        row[i] = code;
                    }
                }
            }
            Self::Oak(base) => {
                for y in base + 1..=base + 6 {
                    if y <= ground {
                        continue;
                    }
                    let trunk = dx == 0 && dz == 0 && y <= base + 4;
                    let leaves =
                        y >= base + 3 && dx.abs() + dz.abs() <= if y == base + 6 { 1 } else { 3 };
                    if let Some(row) = layers.get_mut(y) {
                        if trunk {
                            row[i] = 12;
                        } else if leaves && row[i] == 0 {
                            row[i] = 13;
                        }
                    }
                }
            }
            Self::Cactus(base) if dx == 0 && dz == 0 => {
                for row in &mut layers[base + 1..=base + 2] {
                    row[i] = 14;
                }
            }
            Self::Ruin { code, base } => {
                for (y, row) in layers
                    .iter_mut()
                    .enumerate()
                    .take(base + 5)
                    .skip(ground + 1)
                {
                    if y <= base + 1 || y == base + 4 || (dx.abs() == 2 && dz.abs() == 2) {
                        row[i] = code;
                    } else {
                        row[i] = 0;
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Template;

    #[test]
    fn canopy_vein_and_ruin_continue_across_negative_chunk_edges() {
        let g = Generator(9581);
        for kind in 0..=2 {
            let spacing = if kind == 2 { 48 } else { 8 };
            let mut verified = false;
            'search: for gx in -96..0 {
                for gz in -32..32 {
                    let h = g.hash(gx, gz, 30 + kind);
                    let x = gx * spacing + (h & 65535) as i64 % spacing;
                    let z = gz * spacing + ((h >> 16) & 65535) as i64 % spacing;
                    if x.rem_euclid(16) != 15 {
                        continue;
                    }
                    let Some(feature) = g.feature(x, z, kind, (h >> 32) as u32) else {
                        continue;
                    };
                    let (y, expected) = match feature {
                        Feature::Vein { y, code } => (y, super::super::STATES[code as usize]),
                        Feature::Oak(base) => (base + 5, 18 << 4),
                        Feature::Ruin { code, base } => {
                            (base + 4, super::super::STATES[code as usize])
                        }
                        Feature::Cactus(_) => continue,
                    };
                    if kind == 0
                        && (0..=1).any(|d| {
                            g.cave_span(x + d, z).contains(&(y as u8))
                                || y + 5 >= usize::from(g.surface(x + d, z).height)
                        })
                    {
                        continue;
                    }
                    let cx = x.div_euclid(16) as i32;
                    let cz = z.div_euclid(16) as i32;
                    let world = Template {
                        spawn_hint: None,
                        columns: [cx + 1, cx]
                            .into_iter()
                            .map(|i| ((i, cz), g.column(i, cz)))
                            .collect(),
                    };
                    for dx in [0, 1] {
                        assert_eq!(world.block((x as i32 + dx, y as u8, z as i32)), expected);
                    }
                    verified = true;
                    break 'search;
                }
            }
            assert!(verified, "missing cross-chunk feature kind {kind}");
        }
    }
}
