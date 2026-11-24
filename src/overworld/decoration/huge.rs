use super::{JavaRandom, biomes::BiomeWorld};

type Pos = [i32; 3];

pub(super) fn prepare(w: &mut impl BiomeWorld, [x, y, z]: Pos, height: i32) -> bool {
    if y < 2 || y + height + 1 >= 256 || !matches!(w.state([x, y - 1, z]) >> 4, 2 | 3) {
        return false;
    }
    for dy in 0..=height + 1 {
        let radius = if dy == 0 { 1 } else { 2 };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if !matches!(
                    w.state([x + dx, y + dy, z + dz]) >> 4,
                    0 | 2 | 3 | 6 | 17 | 18 | 106 | 161 | 162
                ) {
                    return false;
                }
            }
        }
    }
    for [dx, dz] in [[0, 0], [1, 0], [0, 1], [1, 1]] {
        let pos = [x + dx, y - 1, z + dz];
        if w.state(pos) >> 4 != 3 {
            w.set_state(pos, 3 << 4);
        }
    }
    true
}

pub(super) fn place(w: &mut impl BiomeWorld, p: Pos, state: u16) -> bool {
    if matches!(w.state(p) >> 4, 0 | 18 | 161) {
        w.set_state(p, state);
        true
    } else {
        false
    }
}

pub(super) fn crown(w: &mut impl BiomeWorld, [x, y, z]: Pos, radius: i32, state: u16) {
    for dx in -radius..=radius + 1 {
        for dz in -radius..=radius + 1 {
            let a = dx.abs().min((dx - 1).abs());
            let b = dz.abs().min((dz - 1).abs());
            if a * a + b * b <= radius * radius {
                place(w, [x + dx, y, z + dz], state);
            }
        }
    }
}

pub(super) fn spruce(
    w: &mut impl BiomeWorld,
    r: &mut JavaRandom,
    [x, y, z]: Pos,
    long_crown: bool,
) {
    let height = 13 + r.bounded(3) + r.bounded(15);
    if !prepare(w, [x, y, z], height) {
        return;
    }
    let depth = r.bounded(5) + if long_crown { 13 } else { 3 };
    let top = y + height;
    let mut previous = 0;
    for cy in top - depth..=top {
        let d = top - cy;
        let radius = (d as f32 / depth as f32 * 3.5) as i32;
        let extra = i32::from(d > 0 && radius == previous && cy & 1 == 0);
        crown(w, [x, cy, z], radius + extra, 18 << 4 | 1);
        previous = radius;
    }
    for dy in 0..height {
        place(w, [x, y + dy, z], 17 << 4 | 1);
        if dy < height - 1 {
            for [dx, dz] in [[1, 0], [1, 1], [0, 1]] {
                place(w, [x + dx, y + dy, z + dz], 17 << 4 | 1);
            }
        }
    }
    for [dx, dz] in [[-1, -1], [2, -1], [-1, 2], [2, 2]] {
        podzol(w, [x + dx, y, z + dz]);
    }
    for _ in 0..5 {
        let n = r.bounded(64);
        let (dx, dz) = (n % 8, n / 8);
        if matches!(dx, 0 | 7) || matches!(dz, 0 | 7) {
            podzol(w, [x - 3 + dx, y, z - 3 + dz]);
        }
    }
}

fn podzol(w: &mut impl BiomeWorld, [x, y, z]: Pos) {
    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            if dx.abs() == 2 && dz.abs() == 2 {
                continue;
            }
            for dy in (-3..=2).rev() {
                let pos = [x + dx, y + dy, z + dz];
                let id = w.state(pos) >> 4;
                if matches!(id, 2 | 3) {
                    w.set_state(pos, 3 << 4 | 2);
                    break;
                }
                if dy < 0 && id != 0 {
                    break;
                }
            }
        }
    }
}

pub(super) fn mossy_rock(w: &mut impl BiomeWorld, r: &mut JavaRandom, [mut x, mut y, mut z]: Pos) {
    while y > 3 && !matches!(w.state([x, y - 1, z]) >> 4, 1..=3) {
        y -= 1;
    }
    if y <= 3 {
        return;
    }
    for _ in 0..3 {
        let (rx, ry, rz) = (r.bounded(2), r.bounded(2), r.bounded(2));
        let radius = (rx + ry + rz) as f32 * 0.333 + 0.5;
        for dz in -rz..=rz {
            for dy in -ry..=ry {
                for dx in -rx..=rx {
                    if (dx * dx + dy * dy + dz * dz) as f32 <= radius * radius {
                        w.set_state([x + dx, y + dy, z + dz], 48 << 4);
                    }
                }
            }
        }
        x += r.bounded(2) - 1;
        y -= r.bounded(2);
        z += r.bounded(2) - 1;
    }
}
