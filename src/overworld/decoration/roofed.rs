use super::{JavaRandom, biomes::BiomeWorld};
type Pos = [i32; 3];

pub(super) fn grove(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, z]: [i32; 2]) {
    for i in 0..4 {
        for j in 0..4 {
            let x = x + i * 4 + 1 + r.bounded(3);
            let z = z + j * 4 + 1 + r.bounded(3);
            let p = [x, w.height(x, z), z];
            if r.bounded(20) == 0 {
                let brown = r.bounded(2) != 0;
                super::mushrooms::giant(w, r, p, brown);
            } else if r.bounded(3) > 0 {
                canopy(w, r, p);
            } else {
                let birch = r.bounded(5) == 0;
                super::forest::tree(w, r, p, birch, false);
            }
        }
    }
}

fn growable(state: u16) -> bool {
    matches!(state >> 4, 0 | 2 | 3 | 6 | 17 | 18 | 106 | 161 | 162)
}
fn log(w: &mut impl BiomeWorld, p: Pos) {
    if growable(w.state(p)) {
        w.set_state(p, 162 << 4 | 1);
    }
}
fn leaf(w: &mut impl BiomeWorld, p: Pos) {
    if w.state(p) == 0 {
        w.set_state(p, 161 << 4 | 1);
    }
}
fn canopy(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, y, z]: Pos) {
    let height = r.bounded(3) + r.bounded(2) + 6;
    if y < 1 || y + height + 1 >= 256 || !matches!(w.state([x, y - 1, z]) >> 4, 2 | 3) {
        return;
    }
    for dy in 0..=height + 1 {
        let radius = if dy >= height - 1 {
            2
        } else {
            i32::from(dy != 0)
        };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if !growable(w.state([x + dx, y + dy, z + dz])) {
                    return;
                }
            }
        }
    }
    for [dx, dz] in [[0, 0], [1, 0], [0, 1], [1, 1]] {
        let p = [x + dx, y - 1, z + dz];
        if w.state(p) >> 4 != 3 {
            w.set_state(p, 3 << 4);
        }
    }
    let [dx, dz] = [[0, -1], [1, 0], [0, 1], [-1, 0]][r.bounded(4) as usize];
    let bend = height - r.bounded(4);
    let mut moves = 2 - r.bounded(3);
    let (mut tx, mut tz) = (x, z);
    let top = y + height - 1;
    for dy in 0..height {
        if dy >= bend && moves > 0 {
            tx += dx;
            tz += dz;
            moves -= 1;
        }
        if matches!(w.state([tx, y + dy, tz]) >> 4, 0 | 18 | 161) {
            for [dx, dz] in [[0, 0], [1, 0], [0, 1], [1, 1]] {
                log(w, [tx + dx, y + dy, tz + dz]);
            }
        }
    }
    for i in -2..=0 {
        for j in -2..=0 {
            for dx in [i, 1 - i] {
                for dz in [j, 1 - j] {
                    leaf(w, [tx + dx, top - 1, tz + dz]);
                    if (i > -2 || j > -1) && (i != -1 || j != -2) {
                        leaf(w, [tx + dx, top + 1, tz + dz]);
                    }
                }
            }
        }
    }
    if r.bounded(2) != 0 {
        for [dx, dz] in [[0, 0], [1, 0], [1, 1], [0, 1]] {
            leaf(w, [tx + dx, top + 2, tz + dz]);
        }
    }
    for dx in -3i32..=4 {
        for dz in -3i32..=4 {
            if dx.abs() < 3 || dz.abs() < 3 {
                leaf(w, [tx + dx, top, tz + dz]);
            }
        }
    }
    for dx in -1..=2 {
        for dz in -1..=2 {
            if ((0..=1).contains(&dx) && (0..=1).contains(&dz)) || r.bounded(3) != 0 {
                continue;
            }
            for dy in 0..r.bounded(3) + 2 {
                log(w, [x + dx, top - dy - 1, z + dz]);
            }
            for i in -1..=1 {
                for j in -1..=1 {
                    leaf(w, [tx + dx + i, top, tz + dz + j]);
                }
            }
            for i in -2i32..=2 {
                for j in -2i32..=2 {
                    if i.abs() != 2 || j.abs() != 2 {
                        leaf(w, [tx + dx + i, top - 1, tz + dz + j]);
                    }
                }
            }
        }
    }
}
