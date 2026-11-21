use super::JavaRandom;

/// Result of WorldGenSpikes after checking the complete footprint on the existing terrain.
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

    pub fn place(&mut self, base: [i32; 3], block: impl Fn([i32; 3]) -> u16) -> Option<EndSpike> {
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

fn footprint(x: i32, z: i32, radius: i32) -> impl Iterator<Item = (i32, i32)> {
    (-radius..=radius).flat_map(move |dx| {
        (-radius..=radius)
            .filter(move |dz| dx * dx + dz * dz <= radius * radius + 1)
            .map(move |dz| (x + dx, z + dz))
    })
}

impl EndSpike {
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
