use crate::overworld::{
    JavaRandom,
    structure_fixtures::{SpawnerMob, portal_eyes, spawner},
};
use fastnbt::Value;

pub fn place_portal(
    bb: [i32; 6],
    facing: usize,
    owner: [i32; 2],
    rng: &mut JavaRandom,
    spawned: &mut bool,
    mut write: impl FnMut([i32; 3], u16),
    mut tile: impl FnMut([i32; 3], Value),
) {
    let global = |x, y, z| {
        let (xx, zz) = match facing {
            0 => (bb[0] + x, bb[2] + z),
            1 => (bb[3] - z, bb[2] + x),
            2 => (bb[0] + x, bb[5] - z),
            _ => (bb[0] + z, bb[2] + x),
        };
        [xx, bb[1] + y, zz]
    };
    let inside = |[x, _, z]: [i32; 3]| ((x - 8) >> 4) == owner[0] && ((z - 8) >> 4) == owner[1];
    let mut put = |x, y, z, state| {
        let pos = global(x, y, z);
        if inside(pos) {
            write(pos, state);
        }
    };
    let mut fill = |[x0, y0, z0, x1, y1, z1]: [i32; 6], fixed: Option<u16>| {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    let state = fixed.unwrap_or_else(|| {
                        if x != x0 && x != x1 && y != y0 && y != y1 && z != z0 && z != z1 {
                            return 0;
                        }
                        let draw = rng.float();
                        if draw < 0.2 {
                            98 * 16 + 2
                        } else if draw < 0.5 {
                            98 * 16 + 1
                        } else if draw < 0.55 {
                            97 * 16 + 2
                        } else {
                            98 * 16
                        }
                    });
                    put(x, y, z, state);
                }
            }
        }
    };
    fill([0, 0, 0, 10, 7, 15], None);
    for x in 4..=6 {
        for y in 1..=3 {
            fill(
                [x, y, 0, x, y, 0],
                Some(if x == 5 && y < 3 { 0 } else { 101 * 16 }),
            );
        }
    }
    for bb in [
        [1, 6, 1, 1, 6, 14],
        [9, 6, 1, 9, 6, 14],
        [2, 6, 1, 8, 6, 2],
        [2, 6, 14, 8, 6, 14],
        [1, 1, 1, 2, 1, 4],
        [8, 1, 1, 9, 1, 4],
    ] {
        fill(bb, None);
    }
    for x in [1, 9] {
        fill([x, 1, 1, x, 1, 3], Some(10 * 16));
    }
    fill([3, 1, 8, 7, 1, 12], None);
    fill([4, 1, 9, 6, 1, 11], Some(10 * 16));
    for z in (3..14).step_by(2) {
        for x in [0, 10] {
            fill([x, 3, z, x, 4, z], Some(101 * 16));
        }
    }
    for x in (2..9).step_by(2) {
        fill([x, 3, 15, x, 4, 15], Some(101 * 16));
    }
    for y in 1..=3 {
        fill([4, y, y + 4, 6, y, 7], None);
    }
    for x in 4..=6 {
        for y in 1..=3 {
            put(x, y, y + 3, 109 * 16 + [2, 1, 3, 0][facing]);
        }
    }
    let eyes = portal_eyes(rng);
    let dirs = [[0, 2, 3, 1], [1, 3, 0, 2], [2, 0, 3, 1], [3, 1, 0, 2]][facing];
    for (index, eye) in eyes.into_iter().enumerate() {
        let edge = index / 3;
        let offset = (index % 3) as i32;
        let (x, z) = match edge {
            0 => (4 + offset, 8),
            1 => (4 + offset, 12),
            2 => (3, 9 + offset),
            _ => (7, 9 + offset),
        };
        put(x, 3, z, 120 * 16 + dirs[edge] + 4 * u16::from(eye));
    }
    let pos = global(5, 3, 6);
    if !*spawned && inside(pos) {
        *spawned = true;
        put(5, 3, 6, 52 * 16);
        tile(pos, spawner(pos, SpawnerMob::Silverfish));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, io::Read};
    #[test]
    fn clipped_portal_rooms_match_official_states_and_rng() {
        let mut data = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../../FONTS EXTRAS/vectors/T2.4b-2/portal-room.gz")[..])
            .read_to_end(&mut data)
            .unwrap();
        let mut input = &data[..];
        for facing in 0..4 {
            let (width, length) = if facing % 2 == 0 { (11, 16) } else { (16, 11) };
            let bb = [-4, 32, 505, -5 + width, 39, 504 + length];
            let mut blocks = HashMap::new();
            let mut tiles = Vec::new();
            let mut spawned = false;
            for cx in ((bb[0] - 8) >> 4)..=((bb[3] - 8) >> 4) {
                for cz in ((bb[2] - 8) >> 4)..=((bb[5] - 8) >> 4) {
                    let mut rng = JavaRandom::new(12345 + i64::from(cx * 31 + cz));
                    place_portal(
                        bb,
                        facing,
                        [cx, cz],
                        &mut rng,
                        &mut spawned,
                        |pos, state| {
                            blocks.insert(pos, state);
                        },
                        |pos, nbt| tiles.push((pos, nbt)),
                    );
                    let mut next = [0; 4];
                    input.read_exact(&mut next).unwrap();
                    assert_eq!(rng.bits(32), u32::from_be_bytes(next));
                }
            }
            assert_eq!(tiles.len(), 1);
            assert_region_roundtrip(&blocks, &tiles);
            for y in bb[1]..=bb[4] {
                for z in bb[2]..=bb[5] {
                    for x in bb[0]..=bb[3] {
                        let mut expected = [0; 2];
                        input.read_exact(&mut expected).unwrap();
                        assert_eq!(
                            blocks[&[x, y, z]],
                            u16::from_be_bytes(expected),
                            "facing {facing} at {x},{y},{z}"
                        );
                    }
                }
            }
        }
        assert!(input.is_empty());
    }

    fn assert_region_roundtrip(blocks: &HashMap<[i32; 3], u16>, tiles: &[([i32; 3], Value)]) {
        use crate::{anvil, arena::Template, overworld::column::pack};
        use std::collections::BTreeMap;
        let mut raw = BTreeMap::new();
        for (&[x, y, z], &state) in blocks {
            raw.entry((x >> 4, z >> 4))
                .or_insert_with(|| vec![0; 65536])[(y * 256 + (z & 15) * 16 + (x & 15)) as usize] =
                state;
        }
        let mut columns = BTreeMap::new();
        for (pos, states) in raw {
            let mut column = pack(&states, [1; 256]);
            let here = tiles
                .iter()
                .filter(|([x, _, z], _)| (*x >> 4, *z >> 4) == pos)
                .map(|(_, nbt)| nbt.clone())
                .collect();
            anvil::store_overworld_tiles(pos, &mut column, here, vec![],vec![]).unwrap();
            columns.insert(pos, column);
        }
        let mut template = Template {
            spawn_hint: None,
            columns,
        };
        let dir = tempfile::tempdir().unwrap();
        for _ in 0..2 {
            anvil::export(dir.path(), &template, &anvil::Edits::new()).unwrap();
            template = anvil::import(dir.path()).unwrap();
            assert_eq!(
                template
                    .columns
                    .values()
                    .map(|col| col.spawners.len())
                    .sum::<usize>(),
                1
            );
            for (&[x, y, z], &state) in blocks {
                let column = &template.columns[&(x >> 4, z >> 4)];
                let actual = column.sections.get(&((y >> 4) as u8)).map_or(0, |s| {
                    s.state(((y & 15) * 256 + (z & 15) * 16 + (x & 15)) as usize)
                });
                assert_eq!(actual, state);
            }
        }
    }
}
