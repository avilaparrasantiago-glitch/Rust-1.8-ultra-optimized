use super::{
    JavaRandom, Overworld, decoration::DecorationWorld, dungeons::DungeonWorld, lakes::LakeWorld,
    population_seed,
};
use crate::anvil::structures::{Family, StructureFile};
use fastnbt::Value;
use flate2::read::GzDecoder;
use std::{
    collections::{BTreeMap, HashMap},
    io,
};
type Pos = [i32; 3];
struct FlatWorld {
    ocean: bool,
    blocks: HashMap<Pos, u16>,
    tiles: HashMap<Pos, Value>,
}
impl LakeWorld for FlatWorld {
    fn state(&self, p: Pos) -> u16 {
        self.blocks.get(&p).copied().unwrap_or_else(|| {
            if p[1] < if self.ocean { 32 } else { 64 } {
                16
            } else if self.ocean && p[1] < 63 {
                9 << 4
            } else {
                0
            }
        })
    }
    fn set_state(&mut self, p: Pos, s: u16) {
        if self.state(p) >> 4 != s >> 4 {
            self.tiles.remove(&p);
        }
        self.blocks.insert(p, s);
    }
    fn solid(&self, p: Pos) -> bool {
        !matches!(self.state(p) >> 4, 0 | 8..=11)
    }
    fn liquid(&self, p: Pos) -> bool {
        matches!(self.state(p) >> 4, 8..=11)
    }
    fn skylight(&self, _: Pos) -> u8 {
        15
    }
    fn mycelium_surface(&self, _: Pos) -> bool {
        false
    }
    fn can_freeze(&self, _: Pos) -> bool {
        false
    }
}
impl DungeonWorld for FlatWorld {
    fn full_block(&self, p: Pos) -> bool {
        !matches!(
            self.state(p) >> 4,
            0 | 8..=11 | 23 | 53 | 54 | 67 | 69 | 93 | 106 | 128 | 131 | 132 | 134
        )
    }
    fn set_tile(&mut self, p: Pos, value: Value) {
        self.tiles.insert(p, value);
    }
}
impl DecorationWorld for FlatWorld {
    fn top_solid_or_liquid(&self, _: i32, _: i32) -> i32 {
        64
    }
}
#[test]
fn official_jungle_hut_and_monument_population() -> io::Result<()> {
    for (seed, ocean, x, z, bytes) in [
        (
            12345,
            false,
            111,
            -211,
            &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-3/jungle-population.nbt.gz")[..],
        ),
        (
            12345,
            false,
            146,
            -509,
            &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-3/hut-population.nbt.gz")[..],
        ),
        (
            12345,
            true,
            -249,
            -508,
            &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-3/monument-population.nbt.gz")[..],
        ),
        (
            -9581,
            false,
            -239,
            -314,
            &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-3/jungle-9581-population.nbt.gz")[..],
        ),
        (
            -9581,
            false,
            67,
            -315,
            &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-3/hut-9581-population.nbt.gz")[..],
        ),
        (
            -9581,
            true,
            -149,
            -315,
            &include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-3/monument-9581-population.nbt.gz")[..],
        ),
    ] {
        let expected: HashMap<String, Value> =
            fastnbt::from_reader(GzDecoder::new(bytes)).map_err(io::Error::other)?;
        let generator = Overworld::new(seed);
        let dir = tempfile::tempdir()?;
        let family = if ocean {
            Family::Monument
        } else {
            Family::Temple
        };
        let mut index = StructureFile::load(dir.path(), family)?;
        generator.discover_features(family, (x, z), &mut index)?;
        let Some(Value::IntArray(bounds)) = index.get(x, z).and_then(|s| s.get("BB")).cloned()
        else {
            panic!("missing bounds");
        };
        let mut world = FlatWorld {
            ocean,
            blocks: HashMap::new(),
            tiles: HashMap::new(),
        };
        let mut entities = Vec::new();
        let mut streams = BTreeMap::new();
        for cx in ((bounds[0] - 8) >> 4)..=((bounds[3] - 8) >> 4) {
            for cz in ((bounds[2] - 8) >> 4)..=((bounds[5] - 8) >> 4) {
                let mut rng = JavaRandom::new(population_seed(seed, cx, cz));
                if ocean {
                    let delta =
                        generator.monument_pieces(&index, (cx, cz), &mut world, &mut rng)?;
                    delta.apply(&mut index)?;
                    entities.extend(delta.elders);
                } else {
                    let delta =
                        generator.scattered_pieces(&index, (cx, cz), &mut world, &mut rng)?;
                    delta.apply(&mut index)?;
                    entities.extend(delta.witches);
                }
                streams.insert(format!("{cx},{cz}"), Value::Int(rng.bits(32) as i32));
                index.save(dir.path())?;
                index = StructureFile::load(dir.path(), family)?;
            }
        }
        let Value::ByteArray(states) = &expected["States"] else {
            panic!()
        };
        let mut iter = states.chunks_exact(2);
        let mut differences = Vec::new();
        let mut count = 0;
        for y in 30..85 {
            for zz in bounds[2] - 8..=bounds[5] + 8 {
                for xx in bounds[0] - 8..=bounds[3] + 8 {
                    let pos = [xx, y, zz];
                    let word = iter.next().unwrap();
                    let reference = u16::from_be_bytes([word[0] as u8, word[1] as u8]);
                    let actual = world.state(pos);
                    if actual != reference {
                        count += 1;
                        if differences.len() < 10 {
                            differences.push((pos, actual, reference));
                        }
                    }
                }
            }
        }
        assert_eq!(count, 0, "start {x},{z}: {differences:?}");
        assert_eq!(
            Value::Compound(streams.into_iter().collect()),
            expected["Next"]
        );
        let Value::Compound(tiles) = &expected["Tiles"] else {
            panic!()
        };
        for (key, tile) in tiles {
            let Value::Compound(fields) = tile else {
                panic!()
            };
            let pos = ["x", "y", "z"].map(|k| match fields[k] {
                Value::Int(v) => v,
                _ => panic!(),
            });
            assert_eq!(world.tiles.get(&pos), Some(tile), "{key}");
        }
        let Value::List(reference) = &expected["Entities"] else {
            panic!()
        };
        let expected_positions: Vec<_> = reference
            .iter()
            .map(|t| {
                let Value::Compound(tag) = t else { panic!() };
                ["X", "Y", "Z"].map(|k| match tag[k] {
                    Value::Int(v) => v,
                    _ => panic!(),
                })
            })
            .collect();
        assert_eq!(entities, expected_positions);
        if !ocean && entities.len() == 1 {
            assert!(index.witch_spawn_at(entities[0]));
            assert!(!index.witch_spawn_at([entities[0][0], 90, entities[0][2]]));
            assert!(world.tiles.values().any(|v| matches!(v,Value::Compound(t) if t.get("Item")==Some(&Value::String("minecraft:red_mushroom".into())))));
        }
        if ocean {
            assert_eq!(entities.len(), 3);
            let mut rng = JavaRandom::new(0);
            let again = generator.monument_pieces(&index, (x, z), &mut world, &mut rng)?;
            assert!(again.elders.is_empty() && again.starts.is_empty());
        }
    }
    Ok(())
}
