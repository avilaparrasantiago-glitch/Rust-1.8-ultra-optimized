use super::{JavaRandom, Overworld, StructureGrid};
use std::sync::OnceLock;

pub(super) fn rectangles<const N: usize>(mut bytes: &'static [u8]) -> [&'static [u8]; N] {
    std::array::from_fn(|_| {
        let size = usize::from(u16::from_be_bytes([bytes[0], bytes[1]])) * 8;
        let (shape, rest) = bytes[2..].split_at(size);
        bytes = rest;
        shape
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn four_official_desert_pyramids_after_structure_placement() {
        let provider = Overworld::new(12345);
        let mut hash = Sha256::new();
        for (x, z) in [(96, -145), (140, -111), (-74, -75), (87, -75)] {
            for cz in z..=z + 1 {
                for cx in x..=x + 1 {
                    let column = provider.column(cx, cz);
                    for i in 0..65536 {
                        let state = column
                            .sections
                            .get(&((i >> 12) as u8))
                            .map_or(0, |s| s.state(i & 4095));
                        hash.update(state.to_be_bytes());
                    }
                }
            }
        }
        assert_eq!(
            format!("{:x}", hash.finalize()),
            "dfb2b83222dedc08e65d9fd8e82b0de2fe336b91bfaa8bdb8559565fabbbaac1"
        );
    }
}

impl Overworld {
    pub(super) fn desert_temple(&self, cx: i32, cz: i32, states: &mut [u16]) {
        // A pyramid covers 21x21 blocks from its start chunk's NW corner.
        for sx in cx - 1..=cx {
            for sz in cz - 1..=cz {
                if StructureGrid::ScatteredFeature.candidate(self.seed, sx, sz) != (sx, sz) {
                    continue;
                }
                if !matches!(
                    self.biomes.blocks(sx * 16 + 8, sz * 16 + 8, 1, 1)[0],
                    2 | 17
                ) {
                    continue;
                }
                let mut world = JavaRandom::new(self.seed);
                let mut map = JavaRandom::new(
                    self.seed
                        ^ world.long().wrapping_mul(i64::from(sx))
                        ^ world.long().wrapping_mul(i64::from(sz)),
                );
                map.bits(32);
                let facing = ((map.bounded(4) + 2) % 4) as usize;
                let (ox, oz) = ((sx - cx) * 16, (sz - cz) * 16);
                for z in oz.max(0)..=(oz + 20).min(15) {
                    for x in ox.max(0)..=(ox + 20).min(15) {
                        let i = (z * 16 + x) as usize;
                        for y in (2..=59).rev() {
                            let b = &mut states[y * 256 + i];
                            if !matches!(*b >> 4, 0 | 8..=11) {
                                break;
                            }
                            *b = 24 << 4;
                        }
                    }
                }
                static SHAPES: OnceLock<[&[u8]; 4]> = OnceLock::new();
                let shape =
                    SHAPES.get_or_init(|| rectangles(include_bytes!("desert-temple.bin")))[facing];
                for r in shape.as_chunks::<8>().0 {
                    let state = u16::from_be_bytes([r[6], r[7]]);
                    for y in (64 + i32::from(r[1] as i8))..=(64 + i32::from(r[4] as i8)) {
                        for z in (oz + i32::from(r[2])).max(0)..=(oz + i32::from(r[5])).min(15) {
                            for x in (ox + i32::from(r[0])).max(0)..=(ox + i32::from(r[3])).min(15)
                            {
                                states[(y * 256 + z * 16 + x) as usize] = state;
                            }
                        }
                    }
                }
            }
        }
    }
}
