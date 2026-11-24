use super::{JavaRandom, carvers::trig};
use std::f32::consts::PI;

/// WorldGenMinable passes each candidate to the world's stone predicate
/// before writing. The callback performs that check and the flag-2 write.
pub(super) fn vein(
    random: &mut JavaRandom,
    [x, y, z]: [i32; 3],
    size: i32,
    state: u16,
    replace_stone: &mut impl FnMut([i32; 3], u16),
) {
    let angle = random.float() * PI;
    let x = x.wrapping_add(8) as f32;
    let z = z.wrapping_add(8) as f32;
    let dx = trig(angle, false) * size as f32 / 8.;
    let dz = trig(angle, true) * size as f32 / 8.;
    let a = [
        f64::from(x + dx),
        f64::from(y + random.bounded(3) - 2),
        f64::from(z + dz),
    ];
    let b = [
        f64::from(x - dx),
        f64::from(y + random.bounded(3) - 2),
        f64::from(z - dz),
    ];
    for i in 0..size {
        let t = i as f32 / size as f32;
        let [cx, cy, cz] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * f64::from(t));
        let scale = random.double() * f64::from(size) / 16.;
        let radius = (f64::from(trig(PI * t, false) + 1.) * scale + 1.) / 2.;
        // Test voxel centers. Keep the lower endpoint inclusive at a tangent.
        let bounds = |c: f64| (c - radius - 0.5).ceil() as i32..=(c + radius - 0.5).floor() as i32;
        let squared = |coord: i32, center: f64| {
            let d = (f64::from(coord) + 0.5 - center) / radius;
            d * d
        };
        for x in bounds(cx) {
            let xx = squared(x, cx);
            if xx >= 1. {
                continue;
            }
            for y in bounds(cy) {
                let xy = xx + squared(y, cy);
                if xy >= 1. {
                    continue;
                }
                for z in bounds(cz) {
                    if xy + squared(z, cz) < 1. {
                        replace_stone([x, y, z], state);
                    }
                }
            }
        }
    }
}

/// DEFAULT BiomeDecorator.generateOres, including the triangular lapis distribution.
/// The callback must replace only block ID 1 (including its metadata variants).
pub fn generate_ores(
    random: &mut JavaRandom,
    chunk: [i32; 2],
    mut replace_stone: impl FnMut([i32; 3], u16),
) {
    let [x, z] = chunk.map(|c| c.wrapping_mul(16));
    // (block state, vein size, attempt count, exclusive upper height).
    for (state, size, count, height) in [
        (3 << 4, 33, 10, 256),
        (13 << 4, 33, 8, 256),
        (1 << 4 | 3, 33, 10, 80),
        (1 << 4 | 1, 33, 10, 80),
        (1 << 4 | 5, 33, 10, 80),
        (16 << 4, 17, 20, 128),
        (15 << 4, 9, 20, 64),
        (14 << 4, 9, 2, 32),
        (73 << 4, 8, 8, 16),
        (56 << 4, 8, 1, 16),
    ] {
        for _ in 0..count {
            let pos = [
                x.wrapping_add(random.bounded(16)),
                random.bounded(height),
                z.wrapping_add(random.bounded(16)),
            ];
            vein(random, pos, size, state, &mut replace_stone);
        }
    }
    let pos = [
        x.wrapping_add(random.bounded(16)),
        random.bounded(16) + random.bounded(16),
        z.wrapping_add(random.bounded(16)),
    ];
    vein(random, pos, 7, 21 << 4, &mut replace_stone);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn official_ores_in_loaded_neighbors() {
        // Six DEFAULT columns after the surface and both carvers. Seed each
        // isolated ore pass at population_seed (earlier phases disabled in
        // the harness), reversing both load and pass order for vector two.
        let mut provider = crate::overworld::Overworld::new(12345);
        provider.structures = false;
        for (reverse, expected) in [(false, 0x8b0faf85d7d52bba), (true, 0x2bfa60e5d091a45a)] {
            let mut cols = BTreeMap::new();
            for n in 0..6 {
                let i = if reverse { 5 - n } else { n };
                let pos = (-1 + i / 2, -1 + i % 2);
                let col = provider.column(pos.0, pos.1);
                let states = (0..65536)
                    .map(|i| {
                        col.sections
                            .get(&((i / 4096) as u8))
                            .map_or(0, |s| s.state(i % 4096))
                    })
                    .collect::<Vec<_>>();
                cols.insert(pos, states);
            }
            for i in 0..2 {
                let cx = if reverse { -i } else { i - 1 };
                let seed = crate::overworld::population_seed(12345, cx, -1);
                generate_ores(&mut JavaRandom::new(seed), [cx, -1], |[x, y, z], ore| {
                    if (0..256).contains(&y) {
                        let col = cols.get_mut(&(x >> 4, z >> 4)).unwrap();
                        let state = &mut col[(y * 256 + (z & 15) * 16 + (x & 15)) as usize];
                        if *state >> 4 == 1 {
                            *state = ore;
                        }
                    }
                });
            }
            let hash = cols
                .values()
                .flatten()
                .fold(0xcbf29ce484222325u64, |h, &s| {
                    (h ^ u64::from(s)).wrapping_mul(0x100000001b3)
                });
            assert_eq!(hash, expected);
        }
    }
    #[test]
    fn official_default_ore_phase() {
        // aeb.generateOres in the dedicated server, on four stone chunks.
        let mut random = JavaRandom::new(12345);
        let mut blocks = BTreeMap::new();
        generate_ores(&mut random, [-1, -1], |p, state| {
            if (0..256).contains(&p[1]) && blocks.get(&p).copied().unwrap_or(16) >> 4 == 1 {
                blocks.insert(p, state);
            }
        });
        assert_eq!(blocks.len(), 5204);
        let mut h = 0xcbf29ce484222325u64;
        for x in -16..16 {
            for z in -16..16 {
                for y in 0..256 {
                    h = (h ^ u64::from(blocks.get(&[x, y, z]).copied().unwrap_or(16)))
                        .wrapping_mul(0x100000001b3);
                }
            }
        }
        assert_eq!(h, 0x6e998d2f677469b6);
        assert_eq!(random.long(), 6232022826071505423);
    }
}
