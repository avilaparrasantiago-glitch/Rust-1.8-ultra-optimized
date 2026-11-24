use super::JavaRandom;
use super::biomes::BiomeWorld;

pub(super) fn giant(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, y, z]: [i32; 3], brown: bool) {
    let h = r.bounded(3) + 4;
    if y < 1 || y + h + 1 >= 256 || !matches!(w.state([x, y - 1, z]) >> 4, 2 | 3 | 110) {
        return;
    }
    for dy in 0..=h + 1 {
        let radius = if dy <= 3 { 0 } else { 3 };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if !matches!(w.state([x + dx, y + dy, z + dz]) >> 4, 0 | 18 | 161) {
                    return;
                }
            }
        }
    }
    let id = if brown { 99 } else { 100 };
    for dy in if brown { h } else { h - 3 }..=h {
        let radius: i32 = if brown {
            3
        } else if dy < h {
            2
        } else {
            1
        };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let mut mx = if dx.abs() == radius { dx.signum() } else { 0 };
                let mut mz = if dz.abs() == radius { dz.signum() } else { 0 };
                if brown || dy < h {
                    if mx != 0 && mz != 0 {
                        continue;
                    }
                    if mx != 0 && dz.abs() == radius - 1 {
                        mz = dz.signum();
                    } else if mz != 0 && dx.abs() == radius - 1 {
                        mx = dx.signum();
                    }
                }
                let meta = (5 + mx + 3 * mz) as u16;
                let p = [x + dx, y + dy, z + dz];
                if (dy == h || meta != 5) && !w.full_block(p) {
                    w.set_state(p, id << 4 | meta);
                }
            }
        }
    }
    for dy in 0..h {
        let p = [x, y + dy, z];
        if !w.full_block(p) {
            w.set_state(p, id << 4 | 10);
        }
    }
}
