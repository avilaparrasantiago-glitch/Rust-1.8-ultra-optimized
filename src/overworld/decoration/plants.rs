use crate::overworld::{JavaRandom, lakes::LakeWorld};
type Pos = [i32; 3];

pub trait PlantWorld: LakeWorld {
    /// World.getLightFromNeighbors, used by BlockMushroom.canBlockStay.
    fn combined_light(&self, p: Pos) -> u8;
    fn full_block(&self, p: Pos) -> bool;
}

#[derive(Debug, Copy, Clone)]
pub enum Plant {
    Flower(u16),
    Grass(u16),
    DeadBush,
    Mushroom(bool),
    Reed,
    Pumpkin,
    Cactus,
    Lily,
}

fn spread(r: &mut JavaRandom, bound: i32) -> i32 {
    r.bounded(bound) - r.bounded(bound)
}
fn horizontal([x, y, z]: Pos) -> [Pos; 4] {
    [[x - 1, y, z], [x + 1, y, z], [x, y, z - 1], [x, y, z + 1]]
}

impl Plant {
    pub fn generate(self, w: &mut impl PlantWorld, r: &mut JavaRandom, [cx, mut cy, cz]: Pos) {
        if matches!(self, Self::Grass(_) | Self::DeadBush) {
            while cy > 0 && matches!(w.state([cx, cy, cz]) >> 4, 0 | 18 | 161) {
                cy -= 1;
            }
        }
        let attempts = match self {
            Self::Grass(_) => 128,
            Self::DeadBush => 4,
            Self::Reed => 20,
            Self::Cactus | Self::Lily => 10,
            _ => 64,
        };
        for _ in 0..attempts {
            let [x, y, z] = if matches!(self, Self::Reed) {
                [cx + spread(r, 4), cy, cz + spread(r, 4)]
            } else {
                [cx + spread(r, 8), cy + spread(r, 4), cz + spread(r, 8)]
            };
            let p = [x, y, z];
            if w.state(p) != 0 {
                continue;
            }
            let below = [x, y - 1, z];
            let ground = w.state(below);
            let id = ground >> 4;
            let state = match self {
                Self::Flower(state) if matches!(id, 2 | 3 | 60) => state,
                Self::Grass(meta) if matches!(id, 2 | 3 | 60) => (31 << 4) | meta,
                Self::DeadBush if matches!(id, 12 | 159 | 172 | 3) => 32 << 4,
                Self::Mushroom(red)
                    if (0..256).contains(&y)
                        && (id == 110
                            || ground == (3 << 4 | 2)
                            || (w.combined_light(p) < 13 && w.full_block(below))) =>
                {
                    (39 + u16::from(red)) << 4
                }
                Self::Lily if id == 9 => 111 << 4,
                Self::Pumpkin if id == 2 => {
                    // EnumFacing.Plane.HORIZONTAL is north, east, south, west.
                    (86 << 4) | [2, 3, 0, 1][r.bounded(4) as usize]
                }
                Self::Reed => {
                    if !horizontal(below)
                        .into_iter()
                        .any(|n| matches!(w.state(n) >> 4, 8 | 9))
                    {
                        continue;
                    }
                    let bound = r.bounded(3) + 1;
                    let height = 2 + r.bounded(bound);
                    if matches!(id, 2 | 3 | 12 | 83) {
                        for dy in 0..height {
                            w.set_state([x, y + dy, z], 83 << 4);
                        }
                    }
                    continue;
                }
                Self::Cactus => {
                    let bound = r.bounded(3) + 1;
                    let height = 1 + r.bounded(bound);
                    if matches!(id, 12 | 81) && !horizontal(p).into_iter().any(|n| w.solid(n)) {
                        for dy in 0..height {
                            w.set_state([x, y + dy, z], 81 << 4);
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            w.set_state(p, state);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::lakes::LakeWorld;
    use std::collections::BTreeMap;

    struct Plot {
        floor: u16,
        writes: BTreeMap<Pos, u16>,
    }
    impl LakeWorld for Plot {
        fn state(&self, [x, y, z]: Pos) -> u16 {
            self.writes.get(&[x, y, z]).copied().unwrap_or(if y > 64 {
                0
            } else if self.floor == 83 {
                if x % 3 == 0 { 9 << 4 } else { 12 << 4 }
            } else {
                self.floor << 4
            })
        }
        fn set_state(&mut self, p: Pos, s: u16) {
            self.writes.insert(p, s);
        }
        fn solid(&self, p: Pos) -> bool {
            matches!(self.state(p) >> 4, 2 | 12 | 81 | 86)
        }
        fn liquid(&self, p: Pos) -> bool {
            self.state(p) >> 4 == 9
        }
        fn skylight(&self, _: Pos) -> u8 {
            0
        }
        fn can_freeze(&self, _: Pos) -> bool {
            false
        }
        fn mycelium_surface(&self, _: Pos) -> bool {
            false
        }
    }
    impl PlantWorld for Plot {
        fn combined_light(&self, _: Pos) -> u8 {
            0
        }
        fn full_block(&self, p: Pos) -> bool {
            matches!(self.state(p) >> 4, 2 | 12 | 86)
        }
    }
    #[test]
    fn official_plant_generators_on_negative_seam() {
        // Individual 1.8.9 WorldGen calls at (-1,65,-1). FNV-1a on
        // (id << 4 | meta) in X/Z/Y order, followed by Random.nextLong().
        for (patch, floor, hash, next) in [
            (
                Plant::Flower(37 << 4),
                2,
                0x64179ac3b1e46b6f,
                7433358876451833299,
            ),
            (
                Plant::Flower(38 << 4),
                2,
                0x74b2e5ca6d6a18cf,
                7433358876451833299,
            ),
            (Plant::Grass(1), 2, 0x905355f13627bbf1, -8426857815866525481),
            (
                Plant::DeadBush,
                12,
                0xee9c1e2349fa7c6f,
                -3500614978637125381,
            ),
            (
                Plant::Mushroom(false),
                2,
                0xc2808e3d9aed776f,
                7433358876451833299,
            ),
            (
                Plant::Mushroom(true),
                2,
                0xe5ade75c102eff8f,
                7433358876451833299,
            ),
            (Plant::Reed, 83, 0xf750998b9cfbdf5f, -5970305719089840402),
            (Plant::Pumpkin, 2, 0x9c3a2935c126be78, -4523862087580514004),
            (Plant::Cactus, 12, 0xc1b99427326eb45f, -7435625448598974047),
            (Plant::Lily, 9, 0xa5fc02c8a7687a0f, 2877948439254078926),
        ] {
            let mut world = Plot {
                floor,
                writes: BTreeMap::new(),
            };
            let mut r = JavaRandom::new(12345);
            patch.generate(&mut world, &mut r, [-1, 65, -1]);
            let mut h = 0xcbf29ce484222325u64;
            for x in -9..=7 {
                for z in -9..=7 {
                    for y in 60..=72 {
                        h = (h ^ u64::from(world.state([x, y, z]))).wrapping_mul(0x100000001b3);
                    }
                }
            }
            assert_eq!(h, hash, "{patch:?}");
            assert_eq!(r.long(), next, "{patch:?}");
        }
    }
}
