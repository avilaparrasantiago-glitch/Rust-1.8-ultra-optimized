use super::Overworld;
use crate::arena::{Column, Section};
use std::{collections::BTreeMap, sync::OnceLock};

impl Overworld {
    pub fn spawn(&self) -> [i32; 3] {
        let mut r = super::JavaRandom::new(self.seed);
        let (mut x, mut z) = self
            .biomes
            .find_position(0, 0, 256, &mut r, |id| {
                matches!(id, 1 | 4 | 5 | 18 | 19 | 21 | 22)
            })
            .unwrap_or((0, 0));
        for attempt in 0..=1000 {
            let col = self.column(x >> 4, z >> 4);
            let block = |y: usize| {
                col.sections.get(&((y >> 4) as u8)).map_or(0, |s| {
                    s.state((y & 15) * 256 + (z & 15) as usize * 16 + (x & 15) as usize)
                })
            };
            let mut y = 63;
            while y < 255 && block(y + 1) != 0 {
                y += 1;
            }
            if block(y) >> 4 == 2 || attempt == 1000 {
                return [x, y as i32 + 1, z];
            }
            x += r.bounded(64) - r.bounded(64);
            z += r.bounded(64) - r.bounded(64);
        }
        unreachable!()
    }
    pub(crate) fn column(&self, cx: i32, cz: i32) -> Column {
        let (states, biomes) = self.primer(cx, cz);
        pack(&states, biomes)
    }
    pub(super) fn primer(&self, cx: i32, cz: i32) -> (Vec<u16>, [u8; 256]) {
        let ids = self.biomes.blocks(cx * 16, cz * 16, 16, 16);
        let biomes = self.biomes.quarter(cx * 4 - 2, cz * 4 - 2, 10, 10);
        self.primer_with_biomes(cx, cz, &ids, &biomes)
    }
    pub(super) fn primer_with_biomes(
        &self,
        cx: i32,
        cz: i32,
        ids: &[i32],
        quarter: &[i32],
    ) -> (Vec<u16>, [u8; 256]) {
        let mut states = self.terrain_with_biomes(cx, cz, quarter);
        self.replace_blocks_for_biome(cx, cz, ids, &mut states);
        for phase in [super::Carver::Caves, super::Carver::Ravine] {
            phase.carve(self.seed, cx, cz, &mut states, ids);
        }
        if self.structures {
            self.desert_temple(cx, cz, &mut states);
        }
        (states, std::array::from_fn(|i| ids[i] as u8))
    }
}
fn attenuate(level: u8, state: u16) -> u8 {
    match state >> 4 {
        0 | 111 => level,
        8 | 9 | 79 => level.saturating_sub(3),
        18 => level.saturating_sub(1),
        _ => 0,
    }
}

fn write_sky(blocks: &[u16], sky: &mut [u8; 256], plane: &mut [u8; 2048]) {
    for (layer, row) in blocks.rchunks_exact(256).zip(plane.rchunks_exact_mut(128)) {
        if sky.iter().all(|&level| level == 0) {
            break;
        }
        for ((pair, levels), byte) in layer
            .as_chunks::<2>()
            .0
            .iter()
            .zip(sky.as_chunks_mut::<2>().0)
            .zip(row)
        {
            levels[0] = attenuate(levels[0], pair[0]);
            levels[1] = attenuate(levels[1], pair[1]);
            *byte = levels[0] | (levels[1] << 4);
        }
    }
}

pub(super) fn pack(states: &[u16], biomes: [u8; 256]) -> Column {
    let mut sections = BTreeMap::new();
    let mut sky = [15u8; 256];
    for (sy, blocks) in states.as_chunks::<4096>().0.iter().enumerate().rev() {
        if blocks.iter().all(|v| *v == 0) {
            continue;
        }
        let mut seen = [0u64; 1024];
        let mut palette = Vec::new();
        for &state in blocks {
            let word = &mut seen[usize::from(state) >> 6];
            let mask = 1u64 << (state & 63);
            if *word & mask == 0 {
                palette.push(state);
                *word |= mask;
            }
        }
        palette.sort_unstable();
        let width = (usize::BITS - (palette.len() - 1).leading_zeros()) as usize;
        let mut indices = Vec::with_capacity(512 * width);
        if width != 0 {
            let mut slots = vec![0u16; usize::from(*palette.last().unwrap()) + 1];
            for (slot, &state) in palette.iter().enumerate() {
                slots[usize::from(state)] = slot as u16;
            }
            let mut word = 0u32;
            let mut bits = 0;
            for &state in blocks {
                word |= u32::from(slots[usize::from(state)]) << bits;
                bits += width;
                while bits >= 8 {
                    indices.push(word as u8);
                    word >>= 8;
                    bits -= 8;
                }
            }
        }
        let mut light = Box::new([[0u8; 2048]; 2]);
        write_sky(blocks, &mut sky, &mut light[1]);
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
        spawners: Vec::new(),
        skylight: true,
        sections,
        biomes,
        packet: OnceLock::new(),
        nbt: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "release performance measurement"]
    fn lit_packet_timing() {
        let terrain = Overworld::new(4862413858852300341);
        let mut samples = Vec::new();
        for x in -6..6 {
            for z in -6..6 {
                let (states, biomes) = terrain.primer(x, z);
                let start = std::time::Instant::now();
                let column = pack(&states, biomes);
                std::hint::black_box(column.packet((x, z)));
                samples.push(start.elapsed());
            }
        }
        samples.sort_unstable();
        println!(
            "144 newly packed lit packets (ms), p50 {:.3}, p95 {:.3}, max {:.3}",
            samples[71].as_secs_f64() * 1000.,
            samples[136].as_secs_f64() * 1000.,
            samples[143].as_secs_f64() * 1000.
        );
    }
}
