use super::JavaRandom;

/// World queries used by WorldGenLakes. Coordinates are absolute; writes
/// have the semantics of World.setBlockState with update flag 2.
pub trait LakeWorld {
    fn state(&self, pos: [i32; 3]) -> u16;
    fn set_state(&mut self, pos: [i32; 3], state: u16);
    fn solid(&self, pos: [i32; 3]) -> bool;
    fn liquid(&self, pos: [i32; 3]) -> bool;
    fn skylight(&self, pos: [i32; 3]) -> u8;
    fn mycelium_surface(&self, pos: [i32; 3]) -> bool;
    fn can_freeze(&self, pos: [i32; 3]) -> bool;
}

#[derive(Clone, Copy, PartialEq)]
pub enum Lake {
    Water,
    Lava,
}

fn cavity(random: &mut JavaRandom) -> [bool; 2048] {
    let mut cells = [false; 2048];
    for _ in 0..random.bounded(4) + 4 {
        let width = random.double() * 6.0 + 3.0;
        let height = random.double() * 4.0 + 2.0;
        let depth = random.double() * 6.0 + 3.0;
        let cx = random.double() * (16.0 - width - 2.0) + 1.0 + width / 2.0;
        let cy = random.double() * (8.0 - height - 4.0) + 2.0 + height / 2.0;
        let cz = random.double() * (16.0 - depth - 2.0) + 1.0 + depth / 2.0;
        for x in 1..15 {
            for z in 1..15 {
                for y in 1..7 {
                    let dx = (x as f64 - cx) / (width / 2.0);
                    let dy = (y as f64 - cy) / (height / 2.0);
                    let dz = (z as f64 - cz) / (depth / 2.0);
                    cells[(x * 16 + z) * 8 + y] |= dx * dx + dy * dy + dz * dz < 1.0;
                }
            }
        }
    }
    cells
}

impl Lake {
    pub fn generate(
        self,
        world: &mut impl LakeWorld,
        random: &mut JavaRandom,
        [x, mut y, z]: [i32; 3],
    ) -> bool {
        let x = x.wrapping_sub(8);
        let z = z.wrapping_sub(8);
        while y > 5 && world.state([x, y, z]) >> 4 == 0 {
            y -= 1;
        }
        if y <= 4 {
            return false;
        }
        let base = y - 4;
        let cells = cavity(random);
        let fluid = match self {
            Self::Water => 9,
            Self::Lava => 11,
        };
        let pos = |i: usize| {
            [
                x.wrapping_add((i / 128) as i32),
                base + (i % 8) as i32,
                z.wrapping_add((i / 8 % 16) as i32),
            ]
        };
        let edge = |i: usize| {
            !cells[i]
                && (i / 128 > 0 && cells[i - 128]
                    || i / 128 < 15 && cells[i + 128]
                    || !(i / 8).is_multiple_of(16) && cells[i - 8]
                    || i / 8 % 16 < 15 && cells[i + 8]
                    || !i.is_multiple_of(8) && cells[i - 1]
                    || i % 8 < 7 && cells[i + 1])
        };
        for i in (0..2048).filter(|&i| edge(i)) {
            let p = pos(i);
            if if i % 8 >= 4 {
                world.liquid(p)
            } else {
                !world.solid(p) && world.state(p) >> 4 != fluid
            } {
                return false;
            }
        }
        for (i, _) in cells.iter().enumerate().filter(|(_, filled)| **filled) {
            world.set_state(pos(i), if i % 8 >= 4 { 0 } else { fluid << 4 });
        }
        for (i, _) in cells
            .iter()
            .enumerate()
            .filter(|(i, filled)| i % 8 >= 4 && **filled)
        {
            let below = pos(i - 1);
            if world.state(below) >> 4 == 3 && world.skylight(pos(i)) > 0 {
                let surface = if world.mycelium_surface(below) {
                    110
                } else {
                    2
                };
                world.set_state(below, surface << 4);
            }
        }
        match self {
            Self::Lava => {
                for i in (0..2048).filter(|&i| edge(i)) {
                    if (i % 8 < 4 || random.bounded(2) != 0) && world.solid(pos(i)) {
                        world.set_state(pos(i), 1 << 4);
                    }
                }
            }
            Self::Water => {
                for i in (4..2048).step_by(8) {
                    if world.can_freeze(pos(i)) {
                        world.set_state(pos(i), 79 << 4);
                    }
                }
            }
        }
        true
    }
}

/// Lake stage of DEFAULT populate. The caller supplies the biome sampled
/// at chunk origin + (16, 16) and the stream left by the structure stage.
pub fn populate_lakes(
    world: &mut impl LakeWorld,
    random: &mut JavaRandom,
    chunk: [i32; 2],
    biome: u8,
    village_generated: bool,
) {
    if village_generated {
        return;
    }
    let [x, z] = chunk.map(|n| n.wrapping_mul(16).wrapping_add(8));
    if !matches!(biome, 2 | 17) && random.bounded(4) == 0 {
        let pos = [
            x.wrapping_add(random.bounded(16)),
            random.bounded(256),
            z.wrapping_add(random.bounded(16)),
        ];
        Lake::Water.generate(world, random, pos);
    }
    if random.bounded(8) == 0 {
        let lx = x.wrapping_add(random.bounded(16));
        let bound = random.bounded(248) + 8;
        let ly = random.bounded(bound);
        let lz = z.wrapping_add(random.bounded(16));
        if ly < 63 || random.bounded(10) == 0 {
            Lake::Lava.generate(world, random, [lx, ly, lz]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn official_population_stream_at_decorator() {
        // aoa.populate with structures disabled and a plains-like biome on
        // solid stone. The official dungeons find no valid doorway here.
        for (cx, writes, next) in [
            (-4, 0, -8862264520117747935),
            (-3, 252, 4950228458832826241),
            (-2, 0, -6000057283539324977),
            (-1, 0, 5249152773775838858),
            (0, 0, 4005512702520059925),
            (1, 240, 8593410665479499776),
            (2, 247, -4678052140113582367),
            (3, 0, -7555655258807791757),
            (4, 0, -3781482440598896324),
        ] {
            let mut world = Stone::default();
            let mut random = JavaRandom::new(crate::overworld::population_seed(12345, cx, -1));
            super::super::dungeons::populate_features(&mut world, &mut random, [cx, -1], 1, false);
            assert_eq!(world.0.len(), writes);

            assert_eq!(random.long(), next);
        }
    }
    #[derive(Default)]
    struct Stone(BTreeMap<[i32; 3], u16>);
    impl super::super::dungeons::DungeonWorld for Stone {
        fn full_block(&self, p: [i32; 3]) -> bool {
            self.solid(p)
        }
        fn set_tile(&mut self, _: [i32; 3], _: fastnbt::Value) {
            panic!("unexpected dungeon in solid stone")
        }
    }
    impl LakeWorld for Stone {
        fn state(&self, p: [i32; 3]) -> u16 {
            *self.0.get(&p).unwrap_or(&16)
        }
        fn set_state(&mut self, p: [i32; 3], state: u16) {
            self.0.insert(p, state);
        }
        fn solid(&self, p: [i32; 3]) -> bool {
            self.state(p) == 16
        }
        fn liquid(&self, p: [i32; 3]) -> bool {
            matches!(self.state(p) >> 4, 9 | 11)
        }
        fn skylight(&self, _: [i32; 3]) -> u8 {
            0
        }
        fn mycelium_surface(&self, _: [i32; 3]) -> bool {
            false
        }
        fn can_freeze(&self, _: [i32; 3]) -> bool {
            false
        }
    }

    #[test]
    fn official_lakes_across_four_chunks() {
        // Dedicated server 1.8.9, apc.b on solid stone. FNV-1a over
        // numerical block states in X/Z/Y order, then Random.nextLong().
        for (fluid, hash, next) in [
            (Lake::Water, 0xff3bd4908e8d5395, -8961857808696917006),
            (Lake::Lava, 0xbefdcdd648725555, -3257221601120504661),
        ] {
            let mut world = Stone::default();
            let mut random = JavaRandom::new(12345);
            assert!(fluid.generate(&mut world, &mut random, [0, 32, 0]));
            let mut actual = 0xcbf29ce484222325u64;
            for x in -8..8 {
                for z in -8..8 {
                    for y in 28..36 {
                        actual = (actual ^ u64::from(world.state([x, y, z])))
                            .wrapping_mul(0x100000001b3);
                    }
                }
            }
            assert_eq!(actual, hash);
            assert_eq!(random.long(), next);
        }
    }
}
