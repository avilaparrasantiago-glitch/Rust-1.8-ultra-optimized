use super::{Bounds, dry_boundary};
use crate::overworld::{
    JavaRandom,
    dungeons::DungeonWorld,
    structure_fixtures::{SpawnerMob, prepare_cart, spawner},
};
use fastnbt::Value;

pub struct Corridor {
    pub bounds: Bounds,
    pub facing: i32,
    pub sections: i32,
    pub rails: bool,
    pub spiders: bool,
    pub spawned: bool,
}

pub enum CorridorFeature {
    Cart { pos: [i32; 3], items: Value },
}

impl Corridor {
    pub fn place(
        &mut self,
        world: &mut impl DungeonWorld,
        owner: [i32; 2],
        rng: &mut JavaRandom,
        mut emit: impl FnMut(CorridorFeature),
    ) -> bool {
        if !dry_boundary(world, self.bounds, owner) {
            return false;
        }
        let [west, bottom, north, east, _, south] = self.bounds;
        let pos = |x, y, z| {
            let (xx, zz) = match self.facing {
                0 => (west + x, north + z),
                1 => (east - z, north + x),
                2 => (west + x, south - z),
                _ => (west + z, north + x),
            };
            [xx, bottom + y, zz]
        };
        let inside = |[x, _, z]: [i32; 3]| ((x - 8) >> 4) == owner[0] && ((z - 8) >> 4) == owner[1];
        let put = |world: &mut _, x, y, z, state| {
            let p = pos(x, y, z);
            if inside(p) {
                crate::overworld::lakes::LakeWorld::set_state(world, p, state);
            }
        };
        let length = self.sections * 5;
        for y in 0..2 {
            for x in 0..3 {
                for z in 0..length {
                    put(world, x, y, z, 0);
                }
            }
        }
        for x in 0..3 {
            for z in 0..length {
                if rng.float() <= 0.8 {
                    put(world, x, 2, z, 0);
                }
            }
        }
        if self.spiders {
            for y in 0..2 {
                for x in 0..3 {
                    for z in 0..length {
                        if rng.float() <= 0.6 {
                            put(world, x, y, z, 30 << 4);
                        }
                    }
                }
            }
        }
        for center in (2..length).step_by(5) {
            for x in [0, 2] {
                for y in [0, 1] {
                    put(world, x, y, center, 85 << 4);
                }
            }
            let split = rng.bounded(4) == 0;
            for x in 0..3 {
                if !split || x != 1 {
                    put(world, x, 2, center, 5 << 4);
                }
            }
            for offset in [-1, 1, -2, 2] {
                for x in [0, 2] {
                    if rng.float()
                        < if offset == 1 || offset == -1 {
                            0.1
                        } else {
                            0.05
                        }
                    {
                        put(world, x, 2, center + offset, 30 << 4);
                    }
                }
            }
            for offset in [-1, 1] {
                if rng.float() < 0.05 {
                    put(world, 1, 2, center + offset, 50 * 16 + 1);
                }
            }
            for (x, offset) in [(2, -1), (0, 1)] {
                if rng.bounded(100) == 0 {
                    let finish = prepare_cart(rng);
                    let location = pos(x, 0, center + offset);
                    if inside(location) && world.state(location) >> 4 == 0 {
                        let (rail, items) = finish(rng);
                        put(
                            world,
                            x,
                            0,
                            center + offset,
                            66 * 16 + (rail ^ ((self.facing & 1) as u16)),
                        );
                        emit(CorridorFeature::Cart {
                            pos: location,
                            items,
                        });
                    }
                }
            }
            if self.spiders && !self.spawned {
                let location = pos(1, 0, center - 1 + rng.bounded(3));
                if inside(location) {
                    self.spawned = true;
                    world.set_state(location, 52 << 4);
                    world.set_tile(location, spawner(location, SpawnerMob::CaveSpider));
                }
            }
        }
        for x in 0..3 {
            for z in 0..length {
                let location = pos(x, -1, z);
                if inside(location) && world.state(location) >> 4 == 0 {
                    world.set_state(location, 5 << 4);
                }
            }
        }
        if self.rails {
            for z in 0..length {
                let floor = pos(1, -1, z);
                if inside(floor) && world.full_block(floor) && rng.float() < 0.7 {
                    put(world, 1, 0, z, 66 * 16 + (self.facing & 1) as u16);
                }
            }
        }
        true
    }
}
