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
        let mut states = self.set_blocks_in_chunk(cx, cz);
        let ids = self.biomes.blocks(cx * 16, cz * 16, 16, 16);
        self.replace_blocks_for_biome(cx, cz, &ids, &mut states);
        for phase in [super::Carver::Caves, super::Carver::Ravine] {
            phase.carve(self.seed, cx, cz, &mut states, &ids);
        }
        if self.structures {
            self.desert_temple(cx, cz, &mut states);
        }
        pack(&states, std::array::from_fn(|i| ids[i] as u8))
    }
}
pub(super) fn pack(states: &[u16], biomes: [u8; 256]) -> Column {
    let mut sections = BTreeMap::new();
    let mut sky = [15u8; 256];
    for (sy, blocks) in states.as_chunks::<4096>().0.iter().enumerate().rev() {
        if blocks.iter().all(|v| *v == 0) {
            continue;
        }
        let mut palette = blocks.to_vec();
        palette.sort_unstable();
        palette.dedup();
        let width = (usize::BITS - (palette.len() - 1).leading_zeros()) as usize;
        let mut indices = vec![0u8; 512 * width];
        let mut light = Box::new([[0u8; 2048]; 2]);
        for (i, &state) in blocks.iter().enumerate().rev() {
            let pos = i % 256;
            let opacity = match state >> 4 {
                0 | 111 => 0,
                8 | 9 | 79 => 3,
                18 => 1,
                _ => 15,
            };
            sky[pos] = sky[pos].saturating_sub(opacity);
            light[1][i / 2] |= sky[pos] << (4 * (i % 2));
            if width != 0 {
                let slot = palette.partition_point(|&v| v < state);
                let bit = i * width;
                for j in 0..width {
                    indices[(bit + j) / 8] |= (((slot >> j) & 1) as u8) << ((bit + j) % 8);
                }
            }
        }
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
