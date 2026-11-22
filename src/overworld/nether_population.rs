use super::{DimensionTerrain, JavaRandom, fortress};
use fastnbt::Value;
use std::collections::BTreeMap;

type Pos = [i32; 3];
const ROCK: u16 = 87 << 4;
const SIDES: [Pos; 4] = [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]];
fn offset([x, y, z]: Pos, [dx, dy, dz]: Pos) -> Pos {
    [x + dx, y + dy, z + dz]
}
pub(super) fn index([x, y, z]: Pos) -> usize {
    (y * 256 + (z & 15) * 16 + (x & 15)) as usize
}
pub(super) fn column([x, _, z]: Pos) -> (i32, i32) {
    (x >> 4, z >> 4)
}

#[derive(Default)]
pub(super) struct Decoration {
    pub blocks: BTreeMap<Pos, u16>,
    pub chests: BTreeMap<Pos, Value>,
    pub ticks: BTreeMap<Pos, i32>,
}
struct Region<'a> {
    provider: &'a DimensionTerrain,
    primers: BTreeMap<(i32, i32), Vec<u16>>,
    out: Decoration,
}
impl Region<'_> {
    fn get(&mut self, p: Pos) -> u16 {
        if !(0..256).contains(&p[1]) {
            return 0;
        }
        if let Some(&s) = self.out.blocks.get(&p) {
            return s;
        }
        let (x, z) = column(p);
        self.primers.entry((x, z)).or_insert_with(|| {
            let mut blocks = self.provider.provide(x, z);
            fortress::paint_column(self.provider.seed, x, z, &mut blocks);
            blocks
        })[index(p)]
    }
    fn spring(&mut self, p: Pos, hidden: bool) {
        if self.get(offset(p, [0, 1, 0])) != ROCK || !matches!(self.get(p), 0 | ROCK) {
            return;
        }
        let mut rock = 0;
        let mut air = 0;
        for d in SIDES.into_iter().chain([[0, -1, 0]]) {
            let s = self.get(offset(p, d));
            rock += i32::from(s == ROCK);
            air += i32::from(s == 0);
        }
        if rock == 5 || (!hidden && rock == 4 && air == 1) {
            self.lava(p);
        }
    }
    fn lava(&mut self, p: Pos) {
        self.out.blocks.insert(p, 11 << 4);
        let down = offset(p, [0, -1, 0]);
        if self.get(down) == 0 {
            self.out.blocks.insert(down, (10 << 4) | 8);
            self.out.ticks.insert(down, 10);
        } else {
            for d in SIDES {
                let q = offset(p, d);
                if self.get(q) == 0 {
                    self.out.blocks.insert(q, (10 << 4) | 1);
                    self.out.ticks.insert(q, 10);
                }
            }
        }
    }
    // Mushrooms need block light below 13. Nether emitters have level 15,
    // so only unobstructed paths of at most two steps can prevent placement.
    fn mushroom_light_ok(&mut self, p: Pos) -> bool {
        let neighbors = SIDES.into_iter().chain([[0, 1, 0], [0, -1, 0]]);
        for d in neighbors.clone() {
            let q = offset(p, d);
            match self.get(q) >> 4 {
                10 | 11 | 51 | 89 => return false,
                0 | 39 | 40 | 113 | 115
                    if neighbors
                        .clone()
                        .any(|s| matches!(self.get(offset(q, s)) >> 4, 10 | 11 | 51 | 89)) =>
                {
                    return false;
                }
                _ => {}
            }
        }
        true
    }
    fn scatter(&mut self, r: &mut JavaRandom, p: Pos, id: u16) {
        for _ in 0..64 {
            let q = offset(
                p,
                [
                    r.bounded(8) - r.bounded(8),
                    r.bounded(4) - r.bounded(4),
                    r.bounded(8) - r.bounded(8),
                ],
            );
            if !(1..255).contains(&q[1]) || self.get(q) != 0 {
                continue;
            }
            let floor = self.get(offset(q, [0, -1, 0]));
            if (id == 51 && floor == ROCK)
                || (id != 51
                    && matches!(floor >> 4, 7 | 13 | 87 | 112 | 153)
                    && self.mushroom_light_ok(q))
            {
                self.out.blocks.insert(q, id << 4);
            }
        }
    }
    fn glow(&mut self, r: &mut JavaRandom, p: Pos) {
        if self.get(p) != 0 || self.get(offset(p, [0, 1, 0])) != ROCK {
            return;
        }
        self.out.blocks.insert(p, 89 << 4);
        for _ in 0..1500 {
            let q = offset(
                p,
                [
                    r.bounded(8) - r.bounded(8),
                    -r.bounded(12),
                    r.bounded(8) - r.bounded(8),
                ],
            );
            if q[1] < 0 || self.get(q) != 0 {
                continue;
            }
            if SIDES
                .into_iter()
                .chain([[0, 1, 0], [0, -1, 0]])
                .filter(|&d| self.get(offset(q, d)) == 89 << 4)
                .take(2)
                .count()
                == 1
            {
                self.out.blocks.insert(q, 89 << 4);
            }
        }
    }
    fn quartz(&mut self, r: &mut JavaRandom, [x, y, z]: Pos) {
        let angle = r.bits(24) as f32 / 16777216. * std::f32::consts::PI;
        // MathHelper uses a 65536-entry float sine table.
        let sin = |i: i32| ((f64::from(i & 65535)) * std::f64::consts::TAU / 65536.).sin() as f32;
        let dx = sin((angle * 10430.378) as i32) * 14. / 8.;
        let dz = sin((angle * 10430.378 + 16384.) as i32) * 14. / 8.;
        let a = [
            f64::from((x + 8) as f32 + dx),
            f64::from(y + r.bounded(3) - 2),
            f64::from((z + 8) as f32 + dz),
        ];
        let b = [
            f64::from((x + 8) as f32 - dx),
            f64::from(y + r.bounded(3) - 2),
            f64::from((z + 8) as f32 - dz),
        ];
        for i in 0..14 {
            let t = i as f32 / 14.;
            let [cx, cy, cz] = std::array::from_fn(|j| a[j] + (b[j] - a[j]) * f64::from(t));
            let radius = (f64::from(sin((std::f32::consts::PI * t * 10430.378) as i32) + 1.)
                * r.double()
                * 14.
                / 16.
                + 1.)
                / 2.;
            for x in (cx - radius).floor() as i32..=(cx + radius).floor() as i32 {
                for y in (cy - radius).floor() as i32..=(cy + radius).floor() as i32 {
                    for z in (cz - radius).floor() as i32..=(cz + radius).floor() as i32 {
                        let d = [
                            (f64::from(x) + 0.5 - cx) / radius,
                            (f64::from(y) + 0.5 - cy) / radius,
                            (f64::from(z) + 0.5 - cz) / radius,
                        ];
                        if d.iter().map(|v| v * v).sum::<f64>() < 1. && self.get([x, y, z]) == ROCK
                        {
                            self.out.blocks.insert([x, y, z], 153 << 4);
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
    #[test]
    fn hidden_spring_requires_five_rock_neighbors() {
        let provider = DimensionTerrain::new(12345, super::super::Dimension::Nether);
        let p = [-8, 48, -8];
        let down = offset(p, [0, -1, 0]);
        let mut region = Region {
            provider: &provider,
            primers: BTreeMap::from([(column(p), vec![ROCK; 65536])]),
            out: Decoration::default(),
        };
        region.primers.get_mut(&column(p)).unwrap()[index(down)] = 0;
        region.spring(p, true);
        assert!(region.out.blocks.is_empty());
        region.spring(p, false);
        assert_eq!(region.get(p), 11 << 4);
        assert_eq!(region.get(down), (10 << 4) | 8);
        assert_eq!(region.out.ticks.get(&down), Some(&10));
        region.out = Decoration::default();
        region.primers.get_mut(&column(p)).unwrap()[index(down)] = ROCK;
        region.spring(p, true);
        assert_eq!(region.get(p), 11 << 4);
        assert!(region.out.ticks.is_empty());
    }
}
fn chest(r: &mut JavaRandom, [x, y, z]: Pos) -> Value {
    let table = [
        ("diamond", 1, 3, 5),
        ("iron_ingot", 1, 5, 5),
        ("gold_ingot", 1, 3, 15),
        ("golden_sword", 1, 1, 5),
        ("golden_chestplate", 1, 1, 5),
        ("flint_and_steel", 1, 1, 5),
        ("nether_wart", 3, 7, 5),
        ("saddle", 1, 1, 10),
        ("golden_horse_armor", 1, 1, 8),
        ("iron_horse_armor", 1, 1, 5),
        ("diamond_horse_armor", 1, 1, 3),
        ("obsidian", 2, 4, 2),
    ];
    let mut slots = BTreeMap::new();
    for _ in 0..2 + r.bounded(4) {
        let mut draw = r.bounded(table.iter().map(|e| e.3).sum());
        let &(id, min, max, _) = table
            .iter()
            .find(|e| {
                draw -= e.3;
                draw < 0
            })
            .unwrap();
        let count = min + r.bounded(max - min + 1);
        let slot = r.bounded(27) as i8;
        slots.insert(
            slot,
            Value::Compound(
                [
                    ("Slot".into(), Value::Byte(slot)),
                    ("id".into(), Value::String(format!("minecraft:{id}"))),
                    ("Count".into(), Value::Byte(count as i8)),
                    ("Damage".into(), Value::Short(0)),
                ]
                .into(),
            ),
        );
    }
    Value::Compound(
        [
            ("id".into(), Value::String("Chest".into())),
            ("x".into(), Value::Int(x)),
            ("y".into(), Value::Int(y)),
            ("z".into(), Value::Int(z)),
            ("Items".into(), Value::List(slots.into_values().collect())),
        ]
        .into(),
    )
}

pub(super) fn populate(provider: &DimensionTerrain, cx: i32, cz: i32) -> Decoration {
    let (mut primer, mut r) = provider.surface_and_caves(cx, cz);
    fortress::paint_column(provider.seed, cx, cz, &mut primer);
    let mut region = Region {
        provider,
        primers: BTreeMap::from([((cx, cz), primer)]),
        out: Decoration::default(),
    };
    fortress::population_features(provider.seed, cx, cz, |p, is_chest| {
        if is_chest {
            region.out.chests.insert(p, chest(&mut r, p));
        } else {
            region.lava(p);
        }
    });
    let pos = |r: &mut JavaRandom, shift, low, height| {
        [
            cx * 16 + shift + r.bounded(16),
            low + r.bounded(height),
            cz * 16 + shift + r.bounded(16),
        ]
    };
    for _ in 0..8 {
        let p = pos(&mut r, 8, 4, 120);
        region.spring(p, false);
    }
    let mut i = 0;
    loop {
        let n = r.bounded(10) + 1;
        if i > r.bounded(n) {
            break;
        }
        let p = pos(&mut r, 8, 4, 120);
        region.scatter(&mut r, p, 51);
        i += 1;
    }
    let mut i = 0;
    loop {
        let n = r.bounded(10) + 1;
        if i >= r.bounded(n) {
            break;
        }
        let p = pos(&mut r, 8, 4, 120);
        region.glow(&mut r, p);
        i += 1;
    }
    for _ in 0..10 {
        let p = pos(&mut r, 8, 0, 128);
        region.glow(&mut r, p);
    }
    for id in [39, 40] {
        if r.bits(1) != 0 {
            let p = pos(&mut r, 8, 0, 128);
            region.scatter(&mut r, p, id);
        }
    }
    for _ in 0..16 {
        let p = pos(&mut r, 0, 10, 108);
        region.quartz(&mut r, p);
    }
    for _ in 0..16 {
        let p = pos(&mut r, 0, 10, 108);
        region.spring(p, true);
    }
    region.out
}
