use crate::anvil::structures::{Family, StructureFile};
use crate::overworld::{JavaRandom, Overworld, StructureGrid};
use fastnbt::{IntArray, Value};
use std::{collections::HashMap, io};

impl Overworld {
    pub(super) fn feature_facing(&self, x: i32, z: i32, scattered: bool) -> i32 {
        let mut seed = JavaRandom::new(self.seed);
        let mut piece = JavaRandom::new(
            self.seed
                ^ seed.long().wrapping_mul(i64::from(x))
                ^ seed.long().wrapping_mul(i64::from(z)),
        );
        if scattered {
            piece.bits(32);
        }
        (piece.bounded(4) + 2) % 4
    }

    // MapGenBase visits potential starts up to eight chunks from the primer.
    // There is at most one candidate per 32x32 cell.
    pub(in crate::overworld) fn discover_features(
        &self,
        family: Family,
        (cx, cz): (i32, i32),
        file: &mut StructureFile,
    ) -> io::Result<bool> {
        let grid = match family {
            Family::Temple => StructureGrid::ScatteredFeature,
            Family::Monument => StructureGrid::OceanMonument,
            _ => return Err(io::Error::other("expected Temple or Monument")),
        };
        let mut changed = false;
        for rx in (cx - 8).div_euclid(32)..=(cx + 8).div_euclid(32) {
            for rz in (cz - 8).div_euclid(32)..=(cz + 8).div_euclid(32) {
                let (x, z) = grid.candidate(self.seed, rx * 32, rz * 32);
                if (x - cx).abs() > 8
                    || (z - cz).abs() > 8
                    || file.get(x, z).is_some()
                    || !grid.starts_at(self.seed, &self.biomes, x, z)
                {
                    continue;
                }
                let scattered = family == Family::Temple;
                let facing = self.feature_facing(x, z, scattered);
                let mut piece = HashMap::from([
                    ("O".to_owned(), Value::Int(facing)),
                    ("GD".into(), Value::Int(0)),
                ]);
                let (name, id, bounds) = if scattered {
                    let biome = self.biomes.blocks(x * 16 + 8, z * 16 + 8, 1, 1)[0];
                    let (id, w, h, d, flags): (_, _, _, _, &[&str]) = match biome {
                        2 | 17 => (
                            "TeDP",
                            21,
                            15,
                            21,
                            &[
                                "hasPlacedChest0",
                                "hasPlacedChest1",
                                "hasPlacedChest2",
                                "hasPlacedChest3",
                            ],
                        ),
                        21 | 22 => (
                            "TeJP",
                            12,
                            10,
                            15,
                            &[
                                "placedMainChest",
                                "placedHiddenChest",
                                "placedTrap1",
                                "placedTrap2",
                            ],
                        ),
                        6 => ("TeSH", 7, 7, 9, &["Witch"]),
                        _ => continue,
                    };
                    for (key, value) in [("Width", w), ("Height", h), ("Depth", d), ("HPos", -1)] {
                        piece.insert(key.into(), Value::Int(value));
                    }
                    for &key in flags {
                        piece.insert(key.into(), Value::Byte(0));
                    }
                    let (dx, dz) = if facing % 2 == 0 { (w, d) } else { (d, w) };
                    (
                        "Temple",
                        id,
                        [
                            x * 16,
                            64,
                            z * 16,
                            x * 16 + dx - 1,
                            64 + h - 1,
                            z * 16 + dz - 1,
                        ],
                    )
                } else {
                    (
                        "Monument",
                        "OMB",
                        [x * 16 - 21, 39, z * 16 - 21, x * 16 + 36, 61, z * 16 + 36],
                    )
                };
                let bb = Value::IntArray(IntArray::new(bounds.to_vec()));
                piece.insert("BB".into(), bb.clone());
                piece.insert("id".into(), Value::String(id.into()));
                let mut start = HashMap::from([
                    ("ChunkX".into(), Value::Int(x)),
                    ("ChunkZ".into(), Value::Int(z)),
                    ("BB".into(), bb),
                    ("id".into(), Value::String(name.into())),
                    ("Children".into(), Value::List(vec![Value::Compound(piece)])),
                ]);
                if !scattered {
                    start.insert("Processed".into(), Value::List(Vec::new()));
                }
                file.insert(start)?;
                changed = true;
            }
        }
        Ok(changed)
    }
}
