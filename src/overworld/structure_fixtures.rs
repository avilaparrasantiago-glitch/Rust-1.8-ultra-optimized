use super::{JavaRandom, dungeons::book};
use fastnbt::Value;
use std::collections::{BTreeMap, HashMap};

type Entry = (&'static str, i16, i32, i32, i32);
const MINE: &[Entry] = &[
    ("iron_ingot", 0, 1, 5, 10),
    ("gold_ingot", 0, 1, 3, 5),
    ("redstone", 0, 4, 9, 5),
    ("dye", 4, 4, 9, 5),
    ("diamond", 0, 1, 2, 3),
    ("coal", 0, 3, 8, 10),
    ("bread", 0, 1, 3, 15),
    ("iron_pickaxe", 0, 1, 1, 1),
    ("rail", 0, 4, 8, 1),
    ("melon_seeds", 0, 2, 4, 10),
    ("pumpkin_seeds", 0, 2, 4, 10),
    ("saddle", 0, 1, 1, 3),
    ("iron_horse_armor", 0, 1, 1, 1),
    ("enchanted_book", 0, 1, 1, 1),
];
const LIBRARY: &[Entry] = &[
    ("book", 0, 1, 3, 20),
    ("paper", 0, 2, 7, 20),
    ("map", 0, 1, 1, 1),
    ("compass", 0, 1, 1, 1),
    ("enchanted_book", 0, 1, 5, 2),
];

fn inventory(rng: &mut JavaRandom, entries: &[Entry], enchantment: Value, rolls: i32) -> Value {
    let total = entries.iter().map(|entry| entry.4).sum();
    let mut slots = BTreeMap::new();
    for _ in 0..rolls {
        let mut ticket = rng.bounded(total);
        let &(name, damage, min, max, _) = entries
            .iter()
            .find(|entry| {
                ticket -= entry.4;
                ticket < 0
            })
            .expect("positive loot weights");
        let count = min + rng.bounded(max - min + 1);
        let split = name == "enchanted_book" && count > 1;
        for _ in 0..if split { count } else { 1 } {
            let slot = rng.bounded(27) as i8;
            let mut item = HashMap::from([
                ("id".into(), Value::String(format!("minecraft:{name}"))),
                ("Damage".into(), Value::Short(damage)),
                (
                    "Count".into(),
                    Value::Byte(if split { 1 } else { count as i8 }),
                ),
                ("Slot".into(), Value::Byte(slot)),
            ]);
            if name == "enchanted_book" {
                item.insert("tag".into(), enchantment.clone());
            }
            slots.insert(slot, Value::Compound(item));
        }
    }
    Value::List(slots.into_values().collect())
}

pub fn minecart_items(rng: &mut JavaRandom) -> Value {
    minecart_rail_and_items(rng).1
}
pub fn minecart_rail_and_items(rng: &mut JavaRandom) -> (u16, Value) {
    prepare_cart(rng)(rng)
}

pub(super) fn prepare_cart(
    rng: &mut JavaRandom,
) -> impl FnOnce(&mut JavaRandom) -> (u16, Value) + use<> {
    let enchantment = book(rng);
    let rolls = 3 + rng.bounded(4);
    move |rng| {
        let rail = rng.bits(1) as u16;
        (rail, inventory(rng, MINE, enchantment, rolls))
    }
}
pub fn library_items(rng: &mut JavaRandom) -> Value {
    let enchantment = book(rng);
    let rolls = 1 + rng.bounded(4);
    inventory(rng, LIBRARY, enchantment, rolls)
}

pub(super) fn prepare_stronghold_chest(
    kind: usize,
    rng: &mut JavaRandom,
) -> impl FnOnce(&mut JavaRandom) -> Value + use<> {
    const CROSS: &[Entry] = &[
        ("iron_ingot", 0, 1, 5, 10),
        ("gold_ingot", 0, 1, 3, 5),
        ("redstone", 0, 4, 9, 5),
        ("coal", 0, 3, 8, 10),
        ("bread", 0, 1, 3, 15),
        ("apple", 0, 1, 3, 15),
        ("iron_pickaxe", 0, 1, 1, 1),
        ("enchanted_book", 0, 1, 1, 1),
    ];
    const HALL: &[Entry] = &[
        ("ender_pearl", 0, 1, 1, 10),
        ("diamond", 0, 1, 3, 3),
        ("iron_ingot", 0, 1, 5, 10),
        ("gold_ingot", 0, 1, 3, 5),
        ("redstone", 0, 4, 9, 5),
        ("bread", 0, 1, 3, 15),
        ("apple", 0, 1, 3, 15),
        ("iron_pickaxe", 0, 1, 1, 5),
        ("iron_sword", 0, 1, 1, 5),
        ("iron_chestplate", 0, 1, 1, 5),
        ("iron_helmet", 0, 1, 1, 5),
        ("iron_leggings", 0, 1, 1, 5),
        ("iron_boots", 0, 1, 1, 5),
        ("golden_apple", 0, 1, 1, 1),
        ("saddle", 0, 1, 1, 1),
        ("iron_horse_armor", 0, 1, 1, 1),
        ("golden_horse_armor", 0, 1, 1, 1),
        ("diamond_horse_armor", 0, 1, 1, 1),
        ("enchanted_book", 0, 1, 1, 1),
    ];
    let enchanted = book(rng);
    let (pool, rolls) = match kind {
        8 => (HALL, 2 + rng.bounded(2)),
        9 => (LIBRARY, 1 + rng.bounded(4)),
        _ => (CROSS, 1 + rng.bounded(4)),
    };
    move |rng| inventory(rng, pool, enchanted, rolls)
}

pub fn portal_eyes(rng: &mut JavaRandom) -> [bool; 12] {
    std::array::from_fn(|_| rng.float() > 0.9)
}

pub enum SpawnerMob {
    Silverfish,
    CaveSpider,
}
pub fn spawner(pos: [i32; 3], mob: SpawnerMob) -> Value {
    let entity = match mob {
        SpawnerMob::Silverfish => "Silverfish",
        SpawnerMob::CaveSpider => "CaveSpider",
    };
    let mut fields = HashMap::from([
        ("id".into(), Value::String("MobSpawner".into())),
        ("EntityId".into(), Value::String(entity.into())),
        ("x".into(), Value::Int(pos[0])),
        ("y".into(), Value::Int(pos[1])),
        ("z".into(), Value::Int(pos[2])),
    ]);
    for (key, value) in [
        ("Delay", 20),
        ("MinSpawnDelay", 200),
        ("MaxSpawnDelay", 800),
        ("SpawnCount", 4),
        ("MaxNearbyEntities", 6),
        ("RequiredPlayerRange", 16),
        ("SpawnRange", 4),
    ] {
        fields.insert(key.into(), Value::Short(value));
    }
    Value::Compound(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventories_and_next_draw_match_server() {
        use std::io::Read;
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-2/structure-loot.nbt.gz")[..])
            .read_to_end(&mut bytes)
            .unwrap();
        let cases: HashMap<String, Value> = fastnbt::from_bytes(&bytes).unwrap();
        for (family, generate) in [
            ("Mine", minecart_items as fn(&mut JavaRandom) -> Value),
            ("Library", library_items),
        ] {
            let Value::List(samples) = &cases[family] else {
                panic!("expected samples")
            };
            for (index, sample) in samples.iter().enumerate() {
                let Value::Compound(sample) = sample else {
                    panic!("expected sample")
                };
                let mut random = JavaRandom::new(12345 + index as i64);
                assert_eq!(generate(&mut random), sample["Items"], "{family} {index}");
                assert_eq!(
                    Value::Int(random.bits(32) as i32),
                    sample["Next"],
                    "{family} {index}"
                );
            }
        }
    }
    #[test]
    fn java_seed_zero_uses_twelve_floats_for_the_frames() {
        assert_eq!(
            portal_eyes(&mut JavaRandom::new(0)),
            [
                false, false, false, false, false, false, false, false, false, false, false, false
            ]
        );
    }
    #[test]
    fn loot_is_slotted_and_spawners_use_legacy_ids() {
        for (mob, name) in [
            (SpawnerMob::Silverfish, "Silverfish"),
            (SpawnerMob::CaveSpider, "CaveSpider"),
        ] {
            let Value::Compound(tile) = spawner([-3, 22, 512], mob) else {
                panic!()
            };
            assert_eq!(tile["EntityId"], Value::String(name.into()));
            assert_eq!(tile["SpawnCount"], Value::Short(4));
        }
        for generate in [minecart_items, library_items] {
            for seed in 0..32 {
                let Value::List(items) = generate(&mut JavaRandom::new(seed)) else {
                    panic!()
                };
                assert!(!items.is_empty());
                let mut last = -1;
                for item in items {
                    let Value::Compound(fields) = item else {
                        panic!()
                    };
                    let Value::Byte(slot) = fields["Slot"] else {
                        panic!()
                    };
                    assert!(slot > last && slot < 27);
                    last = slot;
                }
            }
        }
    }
}
