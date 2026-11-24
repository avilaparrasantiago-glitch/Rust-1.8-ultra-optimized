use super::{JavaRandom, biomes::BiomeWorld, huge};
use crate::overworld::carvers::trig;
type Pos = [i32; 3];
pub(super) const SIDES: [(i32, i32, u16); 4] = [(-1, 0, 8), (1, 0, 2), (0, -1, 1), (0, 1, 4)];
const WOOD: u16 = 17 << 4 | 3;
const LEAF: u16 = 18 << 4 | 3;

pub(super) fn trunk_vine(w: &mut impl BiomeWorld, r: &mut JavaRandom, p: Pos, meta: u16) {
    if r.bounded(3) > 0 && w.state(p) == 0 {
        w.set_state(p, 106 << 4 | meta);
    }
}
pub(super) fn vines_and_cocoa(
    w: &mut impl BiomeWorld,
    r: &mut JavaRandom,
    [x, y, z]: Pos,
    height: i32,
) {
    for dy in -3..=0 {
        let radius = 2 - dy / 2;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let cy = y + height + dy;
                if !matches!(w.state([x + dx, cy, z + dz]) >> 4, 18 | 161) {
                    continue;
                }
                for (vx, vz, meta) in SIDES {
                    if r.bounded(4) == 0 {
                        for drop in 0..5 {
                            let p = [x + dx + vx, cy - drop, z + dz + vz];
                            if w.state(p) != 0 {
                                break;
                            }
                            w.set_state(p, 106 << 4 | meta);
                        }
                    }
                }
            }
        }
    }
    if r.bounded(5) == 0 && height > 5 {
        for level in 0..2 {
            for (dx, dz, meta) in [(0, 1, 2), (-1, 0, 3), (0, -1, 0), (1, 0, 1)] {
                if r.bounded(4 - level) == 0 {
                    let age = r.bounded(3) as u16;
                    w.set_state(
                        [x + dx, y + height - 5 + level, z + dz],
                        127 << 4 | age << 2 | meta,
                    );
                }
            }
        }
    }
}
pub(super) fn shrub(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, mut y, z]: Pos) {
    while y > 0 && matches!(w.state([x, y, z]) >> 4, 0 | 18 | 161) {
        y -= 1;
    }
    if !matches!(w.state([x, y, z]) >> 4, 2 | 3) {
        return;
    }
    y += 1;
    w.set_state([x, y, z], WOOD);
    for dy in 0..=2 {
        let radius = 2 - dy;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if i32::abs(dx) == radius && i32::abs(dz) == radius && r.bounded(2) == 0 {
                    continue;
                }
                let p = [x + dx, y + dy, z + dz];
                if !w.full_block(p) {
                    w.set_state(p, 18 << 4);
                }
            }
        }
    }
}
pub(super) fn giant(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, y, z]: Pos) {
    let height = 10 + r.bounded(3) + r.bounded(20);
    if !huge::prepare(w, [x, y, z], height) {
        return;
    }
    let top = y + height;
    for dy in -2..=0 {
        huge::crown(w, [x, top + dy, z], 3 - dy, LEAF);
    }
    let mut branch = top - 2 - r.bounded(4);
    while branch > y + height / 2 {
        let angle = r.float() * std::f32::consts::PI * 2.;
        let mut end = [x, branch, z];
        for i in 0..5 {
            end = [
                x + (1.5 + trig(angle, true) * i as f32) as i32,
                branch - 3 + i / 2,
                z + (1.5 + trig(angle, false) * i as f32) as i32,
            ];
            w.set_state(end, WOOD);
        }
        let depth = 1 + r.bounded(2);
        for dy in -depth..=0 {
            let radius = 1 - dy;
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    if dx * dx + dz * dz <= radius * radius {
                        huge::place(w, [end[0] + dx, branch + dy, end[2] + dz], LEAF);
                    }
                }
            }
        }
        branch -= 2 + r.bounded(4);
    }
    for dy in 0..height {
        for (dx, dz, sides) in [
            (0, 0, [0, 2]),
            (1, 0, [1, 2]),
            (1, 1, [1, 3]),
            (0, 1, [0, 3]),
        ] {
            if dy == height - 1 && (dx != 0 || dz != 0) {
                continue;
            }
            let p = [x + dx, y + dy, z + dz];
            if matches!(w.state(p) >> 4, 0 | 2 | 3 | 6 | 17 | 18 | 106 | 161 | 162) {
                w.set_state(p, WOOD);
                if dy > 0 {
                    for side in sides {
                        let (vx, vz, meta) = SIDES[side];
                        trunk_vine(w, r, [p[0] + vx, p[1], p[2] + vz], meta);
                    }
                }
            }
        }
    }
}
