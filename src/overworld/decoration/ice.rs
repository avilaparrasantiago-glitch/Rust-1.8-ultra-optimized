use super::biomes::BiomeWorld;
use crate::overworld::JavaRandom;

const PACKED_ICE: u16 = 174 << 4;

/// BiomeGenSnow.decorate runs three spikes, then two ice paths, before ores.
pub(super) fn decorate(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, z]: [i32; 2]) {
    for pass in 0..5 {
        let x = x.wrapping_add(r.bounded(16));
        let z = z.wrapping_add(r.bounded(16));
        let mut y = w.height(x, z);
        while y > 2 && w.state([x, y, z]) == 0 {
            y -= 1;
        }
        if w.state([x, y, z]) >> 4 != 80 {
            continue;
        }
        if pass < 3 {
            spike(w, r, x, y, z);
        } else {
            let radius = 2 + r.bounded(2);
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    if dx * dx + dz * dz <= radius * radius {
                        for dy in -1..=1 {
                            let pos = [x + dx, y + dy, z + dz];
                            if matches!(w.state(pos) >> 4, 3 | 79 | 80) {
                                w.set_state(pos, PACKED_ICE);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn spike(w: &mut impl BiomeWorld, r: &mut JavaRandom, x: i32, surface: i32, z: i32) {
    let mut y = surface + r.bounded(4);
    let height = 7 + r.bounded(4);
    let width = height / 4 + r.bounded(2);
    if width > 1 && r.bounded(60) == 0 {
        y += 10 + r.bounded(30);
    }
    for dy in 0..height {
        let radius = (1.0 - dy as f32 / height as f32) * width as f32;
        let extent = radius.ceil() as i32;
        for dx in -extent..=extent {
            let fx = dx.abs() as f32 - 0.25;
            for dz in -extent..=extent {
                let fz = dz.abs() as f32 - 0.25;
                if (dx != 0 || dz != 0) && fx * fx + fz * fz > radius * radius {
                    continue;
                }
                if (dx.abs() == extent || dz.abs() == extent) && r.float() > 0.75 {
                    continue;
                }
                let mut freeze = |dy| {
                    let pos = [x + dx, y + dy, z + dz];
                    if matches!(w.state(pos) >> 4, 0 | 3 | 79 | 80) {
                        w.set_state(pos, PACKED_ICE);
                    }
                };
                freeze(dy);
                if dy != 0 && extent > 1 {
                    freeze(-dy);
                }
            }
        }
    }
    let roots = (width - 1).clamp(0, 1);
    for dx in -roots..=roots {
        for dz in -roots..=roots {
            let mut remaining = if dx.abs() == 1 && dz.abs() == 1 {
                r.bounded(5)
            } else {
                50
            };
            let mut pos = [x + dx, y - 1, z + dz];
            while pos[1] > 50 && matches!(w.state(pos) >> 4, 0 | 3 | 79 | 80 | 174) {
                w.set_state(pos, PACKED_ICE);
                pos[1] -= 1;
                remaining -= 1;
                if remaining <= 0 {
                    pos[1] -= 1 + r.bounded(5);
                    remaining = r.bounded(5);
                }
            }
        }
    }
}
