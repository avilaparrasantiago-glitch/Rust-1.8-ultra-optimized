use super::JavaRandom;
use std::collections::HashMap;

/// Result of WorldGenSpikes after checking the complete footprint on the existing terrain.
#[derive(Debug)]
pub struct EndSpike {
    base: [i32; 3],
    height: i32,
    radius: i32,
    pub crystal_yaw: f32,
}

pub struct SpikeGenerator(JavaRandom);

impl SpikeGenerator {
    pub fn new(seed: i64) -> Self {
        Self(JavaRandom::new(seed))
    }

    // The End runs BiomeDecorator.generateOres before deciding whether to place
    // a spike. None of the veins can replace end stone, but their random draws
    // still affect the spike's position, shape and the following chunks.
    fn advance_ores(&mut self) {
        for (count, range, size, lapis) in [
            (10, 256, 33, false),
            (8, 256, 33, false),
            (30, 80, 33, false),
            (20, 128, 17, false),
            (20, 64, 9, false),
            (2, 32, 9, false),
            (8, 16, 8, false),
            (1, 16, 8, false),
            (1, 16, 7, true),
        ] {
            for _ in 0..count {
                self.0.bounded(16);
                self.0.bounded(range);
                if lapis {
                    self.0.bounded(range);
                }
                self.0.bounded(16);
                self.0.bits(24);
                self.0.bounded(3);
                self.0.bounded(3);
                for _ in 0..size {
                    self.0.double();
                }
            }
        }
    }

    pub fn place(
        &mut self,
        base: [i32; 3],
        mut block: impl FnMut([i32; 3]) -> u16,
    ) -> Option<EndSpike> {
        let [x, y, z] = base;
        if block(base) != 0 || block([x, y - 1, z]) >> 4 != 121 {
            return None;
        }
        let height = self.0.bounded(32) + 6;
        let radius = self.0.bounded(4) + 1;
        if footprint(x, z, radius).any(|(dx, dz)| block([dx, y - 1, dz]) >> 4 != 121) {
            return None;
        }
        Some(EndSpike {
            base,
            height,
            radius,
            crystal_yaw: self.0.bits(24) as f32 / 16777216. * 360.,
        })
    }
}

/// Populate the 1.8 central island in a fixed row order, independently of
/// client arrival and of the order in which generation workers serve columns.
pub(super) fn island_spikes(provider: &super::DimensionTerrain, seed: i64) -> Vec<EndSpike> {
    let mut generator = SpikeGenerator::new(seed);
    let mut columns = HashMap::new();
    let mut placed = HashMap::new();
    let mut spikes = Vec::new();
    for cz in -8..8 {
        for cx in -8..8 {
            generator.advance_ores();
            if generator.0.bounded(5) == 0 {
                let x = cx * 16 + 8 + generator.0.bounded(16);
                let z = cz * 16 + 8 + generator.0.bounded(16);
                let mut block = |[x, y, z]: [i32; 3]| -> u16 {
                    if !(0..256).contains(&y) {
                        return 0;
                    }
                    if let Some(&state) = placed.get(&[x, y, z]) {
                        return state;
                    }
                    let col = columns
                        .entry((x >> 4, z >> 4))
                        .or_insert_with(|| provider.terrain(x >> 4, z >> 4));
                    col[(y * 256 + (z & 15) * 16 + (x & 15)) as usize]
                };
                let y = (0..256)
                    .rev()
                    .find(|&y| block([x, y, z]) != 0)
                    .map_or(0, |y| y + 1);
                if let Some(spike) = generator.place([x, y, z], block) {
                    placed.extend(spike.obsidian().map(|pos| (pos, 49 << 4)));
                    placed.insert(spike.bedrock(), 7 << 4);
                    spikes.push(spike);
                }
            }
            if (cx, cz) == (0, 0) {
                generator.0.bits(24);
            }
        }
    }
    spikes
}

fn footprint(x: i32, z: i32, radius: i32) -> impl Iterator<Item = (i32, i32)> {
    (-radius..=radius).flat_map(move |dx| {
        (-radius..=radius)
            .filter(move |dz| dx * dx + dz * dz <= radius * radius + 1)
            .map(move |dz| (x + dx, z + dz))
    })
}

impl EndSpike {
    pub(super) fn write_column(&self, cx: i32, cz: i32, states: &mut [u16]) {
        let [x, y, z] = self.base;
        if cx < ((x - self.radius) >> 4)
            || cx > ((x + self.radius) >> 4)
            || cz < ((z - self.radius) >> 4)
            || cz > ((z + self.radius) >> 4)
        {
            return;
        }
        for (px, pz) in
            footprint(x, z, self.radius).filter(|(px, pz)| px >> 4 == cx && pz >> 4 == cz)
        {
            let i = ((pz & 15) * 16 + (px & 15)) as usize;
            for level in y..(y + self.height).min(256) {
                states[level as usize * 256 + i] = 49 << 4;
            }
        }
        if x >> 4 == cx && z >> 4 == cz && y + self.height < 256 {
            states[((y + self.height) * 256 + (z & 15) * 16 + (x & 15)) as usize] = 7 << 4;
        }
    }
    pub fn obsidian(&self) -> impl Iterator<Item = [i32; 3]> {
        let [x, y, z] = self.base;
        (y..(y + self.height).min(256)).flat_map(move |level| {
            footprint(x, z, self.radius).map(move |(px, pz)| [px, level, pz])
        })
    }

    pub fn bedrock(&self) -> [i32; 3] {
        let [x, y, z] = self.base;
        [x, y + self.height, z]
    }

    pub fn crystal(&self) -> [f64; 3] {
        let [x, y, z] = self.bedrock();
        [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::fmt::Write;

    #[test]
    fn official_decorated_end_island() {
        use crate::overworld::{Dimension, DimensionTerrain};
        let provider = DimensionTerrain::new(12345, Dimension::End);
        let crystals: Vec<_> = provider
            .spikes()
            .iter()
            .map(|s| (s.bedrock(), s.crystal_yaw.to_bits()))
            .collect();
        assert_eq!(
            crystals,
            [
                ([3, 92, -81], 1124695000),
                ([33, 85, -59], 1129689128),
                ([44, 78, -63], 1124414538),
                ([27, 92, -46], 1135075237),
                ([31, 83, -37], 1133002269),
                ([89, 77, -33], 1134375544),
                ([-54, 71, -18], 1125671590),
                ([-69, 65, -1], 1135242253),
                ([44, 96, 2], 1133786460),
                ([67, 89, -4], 1111064291),
                ([1, 94, 10], 1128774768),
                ([-45, 95, 32], 1133823178),
                ([-12, 74, 30], 1133246598),
                ([42, 97, 37], 1129408808),
                ([-103, 88, 45], 1135549699),
                ([-77, 88, 40], 1132796201),
                ([58, 72, 61], 1131115534),
            ]
        );
        let mut hash = Sha256::new();
        for cz in -8..8 {
            for cx in -8..8 {
                let mut states = provider.provide(cx, cz);
                for spike in provider.spikes() {
                    spike.write_column(cx, cz, &mut states);
                }
                hash.update(
                    states
                        .into_iter()
                        .map(|s| (s >> 4) as u8)
                        .collect::<Vec<_>>(),
                );
            }
        }
        assert_eq!(
            format!("{:x}", hash.finalize()),
            "38c6ef0c9cc838cbd5180fc4d8adac189baa4b854b33eb07105e7ae27536001b"
        );
    }

    #[test]
    fn official_spike_across_chunk_edges_and_rejected_footprints() {
        for (hole, expected) in [
            (
                200,
                "9c346b4b83dc2e8ed832d6e0198d5bbf69bd0f90235d684fcb024a740adeb6c9",
            ),
            (
                17,
                "f19f49ca872ca40aa654e33cc81805b0c4d264254d8998c047b6a7019d3e1b3d",
            ),
            (
                15,
                "523b45d42961f8197dfa4c16ac2be1e663e79a7fbe1d0024f9179be63b875256",
            ),
        ] {
            let mut generator = SpikeGenerator::new(12345);
            let result = generator.place([15, 63, -16], |[x, y, _]| {
                if y == 62 && x != hole { 121 << 4 } else { 0 }
            });
            let mut trace = String::new();
            let status = if let Some(spike) = result {
                for [x, y, z] in spike.obsidian() {
                    writeln!(trace, "{x} {y} {z} 49").unwrap();
                }
                let [x, y, z] = spike.crystal();
                writeln!(
                    trace,
                    "crystal {} {y} {} {}",
                    x * 2.,
                    z * 2.,
                    spike.crystal_yaw.to_bits()
                )
                .unwrap();
                let [x, y, z] = spike.bedrock();
                writeln!(trace, "{x} {y} {z} 7").unwrap();
                "placed"
            } else {
                "failed"
            };
            writeln!(trace, "{status} {}", generator.0.bits(32) as i32).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(trace)), expected);
        }
    }
}
