mod acacia;
mod big_oak;
pub mod biomes;
mod forest;
mod huge;
mod ice;
mod jungle;
mod mushrooms;
pub mod plants;
mod roofed;
use super::{JavaRandom, lakes::LakeWorld, ores::generate_ores};

pub trait DecorationWorld: LakeWorld {
    fn replace_stone(&mut self, pos: [i32; 3], ore: u16) {
        if self.state(pos) >> 4 == 1 {
            self.set_state(pos, ore);
        }
    }

    /// Y returned by World.getTopSolidOrLiquidBlock.
    fn top_solid_or_liquid(&self, x: i32, z: i32) -> i32;
}

/// The opening passes of BiomeDecorator.genDecorations. The biome is the
/// owner of the decorator, including when a mutated biome delegates.
pub fn decorate_ground(
    world: &mut impl DecorationWorld,
    random: &mut JavaRandom,
    chunk: [i32; 2],
    decorator_biome: u8,
) {
    generate_ores(random, chunk, |pos, ore| {
        world.replace_stone(pos, ore);
    });
    let [x, z] = chunk.map(|c| c.wrapping_mul(16).wrapping_add(8));
    for (block, count, size, depth) in [(12, 3, 7, 2), (82, 1, 4, 1), (13, 1, 6, 2)] {
        if decorator_biome == 6 && block != 82 {
            continue;
        }
        for _ in 0..count {
            let cx = x.wrapping_add(random.bounded(16));
            let cz = z.wrapping_add(random.bounded(16));
            let cy = world.top_solid_or_liquid(cx, cz);
            if !matches!(world.state([cx, cy, cz]) >> 4, 8 | 9) {
                continue;
            }
            let radius = random.bounded(size - 2) + 2;
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    if dx * dx + dz * dz > radius * radius {
                        continue;
                    }
                    for y in cy - depth..=cy + depth {
                        let pos = [cx.wrapping_add(dx), y, cz.wrapping_add(dz)];
                        let existing = world.state(pos) >> 4;
                        if existing == 3 || existing == if block == 82 { 82 } else { 2 } {
                            world.set_state(pos, block << 4);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Riverbed(BTreeMap<[i32; 3], u16>);
    impl LakeWorld for Riverbed {
        fn state(&self, p: [i32; 3]) -> u16 {
            self.0
                .get(&p)
                .copied()
                .unwrap_or(if p[1] >= 62 { 9 << 4 } else { 3 << 4 })
        }
        fn set_state(&mut self, p: [i32; 3], s: u16) {
            self.0.insert(p, s);
        }
        fn solid(&self, p: [i32; 3]) -> bool {
            self.state(p) >> 4 != 9
        }
        fn liquid(&self, p: [i32; 3]) -> bool {
            self.state(p) >> 4 == 9
        }
        fn can_freeze(&self, _: [i32; 3]) -> bool {
            false
        }
        fn skylight(&self, _: [i32; 3]) -> u8 {
            0
        }
        fn mycelium_surface(&self, _: [i32; 3]) -> bool {
            false
        }
    }
    impl DecorationWorld for Riverbed {
        fn top_solid_or_liquid(&self, _: i32, _: i32) -> i32 {
            62
        }
    }
    #[test]
    fn official_sand_clay_gravel_after_ores() {
        let mut w = Riverbed::default();
        let mut r = JavaRandom::new(12345);
        decorate_ground(&mut w, &mut r, [-1, -1], 1);
        let mut hash = 0xcbf29ce484222325u64;
        for x in -16..16 {
            for z in -16..16 {
                for y in 59..64 {
                    hash = (hash ^ u64::from(w.state([x, y, z]))).wrapping_mul(0x100000001b3);
                }
            }
        }
        assert_eq!(hash, 0x7eae2c56ff192375);
        assert_eq!(r.long(), -5668873818259790122);
    }
}
