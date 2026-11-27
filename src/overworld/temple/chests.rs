use super::super::{
    JavaRandom, Overworld, StructureGrid,
    dungeons::{DungeonWorld, book, fill_chest},
};
use crate::anvil::structures::StructureFile;
use fastnbt::Value;

pub(super) const LOOT: &[(&str, i32, i32, i32)] = &[
    ("minecraft:diamond", 1, 3, 3),
    ("minecraft:iron_ingot", 1, 5, 10),
    ("minecraft:gold_ingot", 2, 7, 15),
    ("minecraft:emerald", 1, 3, 2),
    ("minecraft:bone", 4, 6, 20),
    ("minecraft:rotten_flesh", 3, 7, 16),
    ("minecraft:saddle", 1, 1, 3),
    ("minecraft:iron_horse_armor", 1, 1, 1),
    ("minecraft:golden_horse_armor", 1, 1, 1),
    ("minecraft:diamond_horse_armor", 1, 1, 1),
    ("minecraft:enchanted_book", 1, 1, 1),
];

impl Overworld {
    pub(in crate::overworld) fn pyramid_chests(
        &self,
        index: &StructureFile,
        (cx, cz): (i32, i32),
        world: &mut impl DungeonWorld,
        rng: &mut JavaRandom,
    ) -> Vec<[i32; 3]> {
        let mut placed = Vec::new();
        for sx in cx..=cx + 1 {
            for sz in cz..=cz + 1 {
                if StructureGrid::ScatteredFeature.candidate(self.seed, sx, sz) != (sx, sz) {
                    continue;
                }
                let Some(start) = index.get(sx, sz) else {
                    continue;
                };
                let Some(Value::List(children)) = start.get("Children") else {
                    continue;
                };
                let Some(Value::Compound(piece)) = children.first() else {
                    continue;
                };
                if !matches!(piece.get("id"),Some(Value::String(id)) if id=="TeDP") {
                    continue;
                }
                let Some(Value::Int(dir)) = piece.get("O") else {
                    continue;
                };
                // EnumFacing.Plane.HORIZONTAL iterates north, east, south, west.
                for (flag, x, z) in [(2, 10, 8), (3, 12, 10), (0, 10, 12), (1, 8, 10)] {
                    if piece.get(&format!("hasPlacedChest{flag}")) == Some(&Value::Byte(1)) {
                        continue;
                    }
                    let enchantments = book(rng);
                    let rolls = 2 + rng.bounded(5);
                    let (dx, dz) = match dir {
                        0 => (x, z),
                        1 => (20 - z, x),
                        2 => (x, 20 - z),
                        _ => (z, x),
                    };
                    let pos = [sx * 16 + dx, 53, sz * 16 + dz];
                    if (pos[0] - 8) >> 4 != cx || (pos[2] - 8) >> 4 != cz {
                        continue;
                    }
                    world.set_tile(pos, fill_chest(rng, pos, LOOT, enchantments, rolls));
                    placed.push([sx, sz, flag]);
                }
            }
        }
        placed
    }
}
