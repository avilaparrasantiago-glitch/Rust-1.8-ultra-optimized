//! Independently verifiable stages of DEFAULT generation in Java Edition 1.8.

mod biomes;
mod carvers;
pub mod decoration;
pub mod dungeons;
pub mod lakes;
pub mod ores;
mod column;
mod mesa;
pub use carvers::Carver;
mod dimensions;
pub mod fortress;
mod nether_population;
mod temple;
pub mod mineshaft;
pub mod stronghold;
pub mod structure_fixtures;

#[cfg(test)]
mod reference_layout {
    use super::*;
    use fastnbt::Value;
    use std::{collections::HashMap, io::Read};

    fn compare(actual: HashMap<String, Value>, gzip: &[u8]) {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(gzip).read_to_end(&mut raw).unwrap();
        let expected: HashMap<String, Value> = fastnbt::from_bytes(&raw).unwrap();
        let (Value::List(actual_children), Value::List(expected_children)) =
            (&actual["Children"], &expected["Children"]) else { panic!("missing children") };
        for (index, (piece, reference)) in actual_children.iter().zip(expected_children).enumerate() {
            assert_eq!(piece, reference, "piece {index}");
        }
        assert_eq!(actual_children.len(), expected_children.len());
        assert_eq!(actual, expected);
    }

    #[test]
    fn official_mine_layout() {
        for (cx, cz, expected) in [
            (-31, -32, &include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/Mineshaft.nbt.gz")[..]),
            (-24, 4, include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/mineshaft--24-4.nbt.gz")),
            (-22, -4, include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/mineshaft--22--4.nbt.gz")),
            (-11, 30, include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/mineshaft--11-30.nbt.gz")),
            (22, -4, include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/mineshaft-22--4.nbt.gz")),
            (28, 29, include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/mineshaft-28-29.nbt.gz")),
        ] {
            compare(mineshaft::start_nbt(12345, cx, cz).unwrap(), expected);
        }
    }

    #[test]
    fn official_stronghold_layout() {
        let overworld = Overworld::new(12345);
        for (cx, cz, expected) in [
            (-44, 59, &include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/Stronghold.nbt.gz")[..]),
            (-16, -38, include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/stronghold--16--38.nbt.gz")),
            (61, 18, include_bytes!("../FONTS EXTRAS/vectors/T2.4b-2/stronghold-61-18.nbt.gz")),
        ] {
            compare(overworld.stronghold_start_nbt(cx, cz).unwrap(), expected);
        }
    }
}
mod spikes;
pub use spikes::{EndSpike, SpikeGenerator};
mod noise;
pub use dimensions::{Dimension, DimensionTerrain};
mod provider;
pub(crate) mod populated;
mod simplex;
pub use biomes::Biomes;
pub use noise::Octaves;
pub use provider::Overworld;

fn mix(seed: i64, salt: i64) -> i64 {
    seed.wrapping_mul(
        seed.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407),
    )
    .wrapping_add(salt)
}

fn layer_seed(seed: i64, salt: i64) -> i64 {
    let base = (0..3).fold(salt, |s, _| mix(s, salt));
    (0..3).fold(seed, |s, _| mix(s, base))
}

struct CellRandom {
    world: i64,
    state: i64,
}

impl CellRandom {
    fn new(world: i64, x: i32, z: i32) -> Self {
        let state = [x, z, x, z]
            .into_iter()
            .fold(world, |s, n| mix(s, i64::from(n)));
        Self { world, state }
    }

    fn draw(&mut self, bound: i64) -> usize {
        let value = (self.state >> 24).rem_euclid(bound);
        self.state = mix(self.state, self.world);
        value as usize
    }
}

pub struct LandLayers {
    island_seed: i64,
    zoom_seed: i64,
}

impl LandLayers {
    pub fn new(seed: i64) -> Self {
        Self {
            island_seed: layer_seed(seed, 1),
            zoom_seed: layer_seed(seed, 2000),
        }
    }

    pub fn island(&self, x: i32, z: i32) -> u8 {
        u8::from((x, z) == (0, 0) || CellRandom::new(self.island_seed, x, z).draw(10) == 0)
    }

    pub fn fuzzy_zoom(&self, x: i32, z: i32) -> u8 {
        let (px, pz) = (x >> 1, z >> 1);
        let nw = self.island(px, pz);
        if x & 1 == 0 && z & 1 == 0 {
            return nw;
        }
        let mut rng = CellRandom::new(self.zoom_seed, px << 1, pz << 1);
        let sw = self.island(px, pz + 1);
        let west = [nw, sw][rng.draw(2)];
        if x & 1 == 0 {
            return west;
        }
        let ne = self.island(px + 1, pz);
        let north = [nw, ne][rng.draw(2)];
        if z & 1 == 0 {
            return north;
        }
        [nw, ne, sw, self.island(px + 1, pz + 1)][rng.draw(4)]
    }
}

pub struct JavaRandom(u64);

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        Self((seed as u64 ^ 0x5deece66d) & ((1 << 48) - 1))
    }

    fn bits(&mut self, count: u32) -> u32 {
        self.0 = self.0.wrapping_mul(0x5deece66d).wrapping_add(11) & ((1 << 48) - 1);
        (self.0 >> (48 - count)) as u32
    }

    fn long(&mut self) -> i64 {
        let upper = i64::from(self.bits(32)) << 32;
        upper.wrapping_add(i64::from(self.bits(32) as i32))
    }

    fn double(&mut self) -> f64 {
        let high = u64::from(self.bits(26)) << 27;
        (high | u64::from(self.bits(27))) as f64 / (1u64 << 53) as f64
    }

    fn bounded(&mut self, bound: i32) -> i32 {
        loop {
            let bits = self.bits(31) as i32;
            if bound & (bound - 1) == 0 {
                return ((i64::from(bits) * i64::from(bound)) >> 31) as i32;
            }
            let value = bits % bound;
            if (bits - value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }
}

/// Seed passed to Random.setSeed at the start of ChunkProviderGenerate.populate.
pub fn population_seed(world: i64, chunk_x: i32, chunk_z: i32) -> i64 {
    let mut random = JavaRandom::new(world);
    // Java divides signed longs toward zero before doubling them again.
    let mut odd = || (random.long() / 2 * 2).wrapping_add(1);
    let x_seed = odd();
    let z_seed = odd();
    i64::from(chunk_x)
        .wrapping_mul(x_seed)
        .wrapping_add(i64::from(chunk_z).wrapping_mul(z_seed))
        ^ world
}

/// MapGenMineshaft's DEFAULT start predicate, before constructing any pieces.
pub fn mineshaft_start(seed: i64, x: i32, z: i32) -> bool {
    mineshaft_random(seed, x, z).is_some()
}

fn mineshaft_random(seed: i64, x: i32, z: i32) -> Option<JavaRandom> {
    let mut world = JavaRandom::new(seed);
    let chunk_seed =
        seed ^ world.long().wrapping_mul(i64::from(x)) ^ world.long().wrapping_mul(i64::from(z));
    let mut chunk = JavaRandom::new(chunk_seed);
    // MapGenStructure consumes one unbounded integer before calling the predicate.
    chunk.bits(32);
    (chunk.double() < 0.004 && chunk.bounded(80) < x.wrapping_abs().max(z.wrapping_abs())).then_some(chunk)
}

pub enum StructureGrid {
    Village,
    ScatteredFeature,
    OceanMonument,
}

/// MapGenNetherBridge tests one candidate in roughly one third of its 16-chunk cells.
pub fn nether_fortress_start(seed: i64, x: i32, z: i32) -> bool {
    let (rx, rz) = (x >> 4, z >> 4);
    let mut rng = JavaRandom::new(seed ^ i64::from(rx ^ (rz << 4)));
    rng.bits(32);
    rng.bounded(3) == 0
        && x == (rx << 4) + 4 + rng.bounded(8)
        && z == (rz << 4) + 4 + rng.bounded(8)
}

impl StructureGrid {
    /// Tests both the regional candidate and the biome filters used before piece assembly.
    pub fn starts_at(&self, seed: i64, biomes: &Biomes, cx: i32, cz: i32) -> bool {
        if self.candidate(seed, cx, cz) != (cx, cz) {
            return false;
        }
        let (x, z) = (cx * 16 + 8, cz * 16 + 8);
        match self {
            Self::Village => matches!(biomes.quarter(x >> 2, z >> 2, 1, 1)[0], 1 | 2 | 35),
            Self::ScatteredFeature => matches!(biomes.blocks(x, z, 1, 1)[0], 2 | 6 | 17 | 21 | 22),
            Self::OceanMonument => {
                let (left, top) = ((x - 29) >> 2, (z - 29) >> 2);
                let width = (((x + 29) >> 2) - left + 1) as usize;
                let height = (((z + 29) >> 2) - top + 1) as usize;
                biomes.blocks(x, z, 1, 1)[0] == 24
                    && biomes
                        .quarter(left, top, width, height)
                        .iter()
                        .all(|id| matches!(id, 0 | 7 | 10 | 11 | 24))
            }
        }
    }

    /// Candidate chunk in the 32-chunk cell containing the input chunk.
    /// Biome viability is a subsequent stage.
    pub fn candidate(&self, seed: i64, chunk_x: i32, chunk_z: i32) -> (i32, i32) {
        let (x, z) = (chunk_x.div_euclid(32), chunk_z.div_euclid(32));
        let salt = match self {
            Self::Village => 10387312,
            Self::ScatteredFeature => 14357617,
            Self::OceanMonument => 10387313,
        };
        let mut rng = JavaRandom::new(
            seed.wrapping_add(i64::from(x).wrapping_mul(341873128712))
                .wrapping_add(i64::from(z).wrapping_mul(132897987541))
                .wrapping_add(salt),
        );
        let mut offset = || match self {
            Self::OceanMonument => (rng.bounded(27) + rng.bounded(27)) / 2,
            _ => rng.bounded(24),
        };
        (32 * x + offset(), 32 * z + offset())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn official_population_seeds() {
        // setSeed and the first nextInt(4) in 1.8.9 aoa.populate, structures off.
        for (world, x, z, seed, first) in [
            (12345, -1, 0, -6674089274190693130, 3),
            (-9581, 0, -1, -7679664831557963206, 3),
            (12345, -129, 128, -4601265542202375946, 0),
            (1, 200000, -300000, 7291686953485037985, 2),
        ] {
            let actual = population_seed(world, x, z);
            assert_eq!(actual, seed);
            assert_eq!(JavaRandom::new(actual).bounded(4), first);
        }
    }

    #[test]
    fn official_viable_structure_starts() {
        use StructureGrid::*;
        for (seed, expected) in [
            (
                12345,
                "e15df292c4b264340cb4b54038b90a26586b05833b7373d47ce238b92bf34a17",
            ),
            (
                -9581,
                "4679a8b9be270904a09e20cc78720a23f5306406e5c3245357ddd0bf4357a3ca",
            ),
        ] {
            let biomes = Biomes::new(seed);
            let mut hash = Sha256::new();
            for cz in -128..128 {
                for cx in -128..128 {
                    let mut mask = u8::from(nether_fortress_start(seed, cx, cz)) << 3;
                    for (i, kind) in [Village, ScatteredFeature, OceanMonument]
                        .iter()
                        .enumerate()
                    {
                        mask |= u8::from(kind.starts_at(seed, &biomes, cx, cz)) << i;
                    }
                    hash.update([mask]);
                }
            }
            assert_eq!(format!("{:x}", hash.finalize()), expected);
        }
    }

    #[test]
    fn cubiomes_mineshaft_starts() {
        let mut starts = Vec::new();
        for x in -32..32 {
            for z in -32..32 {
                if mineshaft_start(12345, x, z) {
                    starts.push((x, z));
                }
            }
        }
        assert_eq!(
            starts,
            [
                (-31, -32),
                (-24, 4),
                (-22, -4),
                (-11, 30),
                (22, -4),
                (28, 29)
            ]
        );
    }

    #[test]
    fn cubiomes_structure_candidates() {
        use StructureGrid::*;
        for (cell, expected) in [
            ((0, 0), [(7, 23), (17, 5), (3, 19)]),
            ((-1, -2), [(-25, -42), (-32, -54), (-20, -56)]),
            ((24, -37), [(789, -1167), (783, -1180), (788, -1167)]),
        ] {
            for (grid, candidate) in [Village, ScatteredFeature, OceanMonument]
                .iter()
                .zip(expected)
            {
                for edge in [0, 31] {
                    assert_eq!(
                        grid.candidate(12345, cell.0 * 32 + edge, cell.1 * 32 + edge),
                        candidate
                    );
                }
            }
        }
    }

    #[test]
    fn official_land_layers() {
        for (seed, island_hash, fuzzy_hash) in [
            (
                12345,
                "0e81d80c3ea6f386b246ddd9d6ea2286257c6ee837fbc3746e5dfc9f61380b55",
                "c6e30ecf59fb1cf9ec4a468d7cc743c6ed5d6709049eed5f3294555c7492c89c",
            ),
            (
                -9581,
                "208e8c38b3d077cd48970f7777a289461e2d8d1c616dab1705f84cd6aefcfb5c",
                "fa7227131d50bc7cfba4a7b175d4c655168e536a8b2a6beeefd3f5f678d5d7bf",
            ),
            (
                281474976723001,
                "8bcbfdc5f10e1cb5feec66252b5b2f1dcfcb9efd2bcbd96582bb8f60290badb8",
                "74f60418bac77ead5ad9534188b24665a2153e24af8c516bc40ec5a921b325a0",
            ),
        ] {
            let layers = LandLayers::new(seed);
            let mut island = Sha256::new();
            let mut fuzzy = Sha256::new();
            for cz in -1..=1 {
                for cx in -1..=1 {
                    for z in 0..16 {
                        for x in 0..16 {
                            island.update([layers.island(cx * 16 + x, cz * 16 + z)]);
                            fuzzy.update([layers.fuzzy_zoom(cx * 16 + x, cz * 16 + z)]);
                        }
                    }
                }
            }
            assert_eq!(format!("{:x}", island.finalize()), island_hash);
            assert_eq!(format!("{:x}", fuzzy.finalize()), fuzzy_hash);
        }
    }
}
