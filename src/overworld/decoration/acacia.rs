use super::{JavaRandom, biomes::BiomeWorld};
type Pos = [i32; 3];
const DIRECTIONS: [[i32; 2]; 4] = [[0, -1], [1, 0], [0, 1], [-1, 0]];
fn place(w: &mut impl BiomeWorld, p: Pos, id: u16) -> bool {
    if !matches!(w.state(p) >> 4, 0 | 18 | 161) {
        return false;
    }
    w.set_state(p, id << 4);
    true
}
fn crown(w: &mut impl BiomeWorld, [x, y, z]: Pos, radius: i32) {
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            if dx.abs() != radius || dz.abs() != radius {
                place(w, [x + dx, y, z + dz], 161);
            }
        }
    }
    for dx in -1..=1 {
        for dz in -1..=1 {
            place(w, [x + dx, y + 1, z + dz], 161);
        }
    }
    if radius == 3 {
        for [dx, dz] in DIRECTIONS {
            place(w, [x + 2 * dx, y + 1, z + 2 * dz], 161);
        }
    }
}
pub(super) fn generate(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, y, z]: Pos) {
    let h = r.bounded(3) + r.bounded(3) + 5;
    if y < 1 || y >= 256 - h - 1 || !matches!(w.state([x, y - 1, z]) >> 4, 2 | 3) {
        return;
    }
    for dy in 0..=h + 1 {
        let radius = if dy >= h - 1 { 2 } else { i32::from(dy != 0) };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if !matches!(
                    w.state([x + dx, y + dy, z + dz]) >> 4,
                    0 | 2 | 3 | 6 | 17 | 18 | 106 | 161 | 162
                ) {
                    return;
                }
            }
        }
    }
    if w.state([x, y - 1, z]) >> 4 != 3 {
        w.set_state([x, y - 1, z], 3 << 4);
    }
    let dir = DIRECTIONS[r.bounded(4) as usize];
    let bend = h - r.bounded(4) - 1;
    let mut steps = 3 - r.bounded(3);
    let (mut xx, mut zz, mut top) = (x, z, 0);
    for dy in 0..h {
        if dy >= bend && steps > 0 {
            xx += dir[0];
            zz += dir[1];
            steps -= 1;
        }
        if place(w, [xx, y + dy, zz], 162) {
            top = y + dy;
        }
    }
    crown(w, [xx, top, zz], 3);
    let branch = DIRECTIONS[r.bounded(4) as usize];
    if branch != dir {
        let first = bend - r.bounded(2) - 1;
        let len = 1 + r.bounded(3);
        let (mut xx, mut zz, mut top) = (x, z, 0);
        for dy in first..h.min(first + len) {
            if dy < 1 {
                continue;
            }
            xx += branch[0];
            zz += branch[1];
            if place(w, [xx, y + dy, zz], 162) {
                top = y + dy;
            }
        }
        if top > 0 {
            crown(w, [xx, top, zz], 2);
        }
    }
}
