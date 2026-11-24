use super::{JavaRandom, biomes::BiomeWorld};

pub(super) fn tree(
    w: &mut impl BiomeWorld,
    r: &mut JavaRandom,
    pos: [i32; 3],
    birch: bool,
    tall: bool,
) {
    let height = r.bounded(3) + if birch { 5 } else { 4 } + if tall { r.bounded(7) } else { 0 };
    small_tree(w, r, pos, height, if birch { 2 } else { 0 });
}

pub(super) fn small_tree(
    w: &mut impl BiomeWorld,
    r: &mut JavaRandom,
    [x, y, z]: [i32; 3],
    height: i32,
    species: u16,
) {
    if y < 1 || y + height + 1 >= 256 || !matches!(w.state([x, y - 1, z]) >> 4, 2 | 3 | 60) {
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
    let replaceable =
        |id| matches!(id, 0 | 18 | 161) || species != 2 && matches!(id, 31 | 32 | 106 | 175);
    for dy in -3..=0 {
        let radius = 1 - dy / 2;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if i32::abs(dx) == radius
                    && i32::abs(dz) == radius
                    && (r.bounded(2) == 0 || dy == 0)
                {
                    continue;
                }
                let p = [x + dx, y + height + dy, z + dz];
                if replaceable(w.state(p) >> 4) {
                    w.set_state(p, (18 << 4) | species);
                }
            }
        }
    }
    for dy in 0..height {
        let p = [x, y + dy, z];
        if replaceable(w.state(p) >> 4) {
            w.set_state(p, (17 << 4) | species);
            if species == 3 && dy > 0 {
                for (dx, dz, meta) in super::jungle::SIDES {
                    super::jungle::trunk_vine(w, r, [x + dx, y + dy, z + dz], meta);
                }
            }
        }
    }
    if species == 3 {
        super::jungle::vines_and_cocoa(w, r, [x, y, z], height);
    }
}

pub(super) fn double_flowers(
    w: &mut impl BiomeWorld,
    r: &mut JavaRandom,
    base: [i32; 2],
    extra: i32,
) {
    for _ in 0..r.bounded(5) - 3 + extra {
        let meta = [1, 4, 5][r.bounded(3) as usize];
        for _ in 0..5 {
            if double_patch(w, r, base, meta) {
                break;
            }
        }
    }
}

pub(super) fn double_patch(
    w: &mut impl BiomeWorld,
    r: &mut JavaRandom,
    base: [i32; 2],
    meta: u16,
) -> bool {
    let x = base[0] + r.bounded(16);
    let z = base[1] + r.bounded(16);
    let y = r.bounded(w.height(x, z) + 32);
    let mut placed = false;
    for _ in 0..64 {
        let dx = r.bounded(8) - r.bounded(8);
        let dy = r.bounded(4) - r.bounded(4);
        let dz = r.bounded(8) - r.bounded(8);
        let p = [x + dx, y + dy, z + dz];
        let above = [p[0], p[1] + 1, p[2]];
        if w.state(p) == 0
            && w.state(above) == 0
            && matches!(w.state([p[0], p[1] - 1, p[2]]) >> 4, 2 | 3 | 60)
        {
            w.set_state(p, (175 << 4) | meta);
            w.set_state(above, (175 << 4) | 10);
            placed = true;
        }
    }

    placed
}
pub(super) fn conifer(
    w: &mut impl BiomeWorld,
    r: &mut JavaRandom,
    [x, y, z]: [i32; 3],
    pine: bool,
) {
    let (height, start, max_radius) = if pine {
        let h = r.bounded(5) + 7;
        let b = h - r.bounded(2) - 3;
        (h, b, 1 + r.bounded(h - b + 1))
    } else {
        (r.bounded(4) + 6, 1 + r.bounded(2), 2 + r.bounded(2))
    };
    let floor = w.state([x, y - 1, z]) >> 4;
    if y < 1 || y >= 256 - height - 1 || !(matches!(floor, 2 | 3) || !pine && floor == 60) {
        return;
    }
    for dy in 0..=height + 1 {
        let radius = if dy < start { 0 } else { max_radius };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let id = w.state([x + dx, y + dy, z + dz]) >> 4;
                if !(matches!(id, 0 | 18 | 161) || pine && matches!(id, 2 | 3 | 6 | 17 | 106 | 162))
                {
                    return;
                }
            }
        }
    }
    if floor != 3 {
        w.set_state([x, y - 1, z], 3 << 4);
    }
    let mut radius = if pine { 0 } else { r.bounded(2) };
    let mut limit = 1;
    let mut reset = 0;
    for dy in (start..=height).rev() {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let p = [x + dx, y + dy, z + dz];
                if (radius == 0 || dx.abs() != radius || dz.abs() != radius) && !w.full_block(p) {
                    w.set_state(p, (18 << 4) | 1);
                }
            }
        }
        if pine {
            if radius >= 1 && dy == start + 1 {
                radius -= 1;
            } else if radius < max_radius {
                radius += 1;
            }
        } else if radius >= limit {
            radius = reset;
            reset = 1;
            limit = (limit + 1).min(max_radius);
        } else {
            radius += 1;
        }
    }
    let trim = if pine { 1 } else { r.bounded(3) };
    for dy in 0..height - trim {
        let p = [x, y + dy, z];
        if matches!(w.state(p) >> 4, 0 | 18 | 161) {
            w.set_state(p, (17 << 4) | 1);
        }
    }
}
pub(super) fn swamp_tree(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, mut y, z]: [i32; 3]) {
    let height = r.bounded(4) + 5;
    while matches!(w.state([x, y - 1, z]) >> 4, 8 | 9) {
        y -= 1;
    }
    if y < 1 || y >= 256 - height - 1 || !matches!(w.state([x, y - 1, z]) >> 4, 2 | 3) {
        return;
    }
    for dy in 0..=height + 1 {
        let radius = if dy >= height - 1 {
            3
        } else {
            i32::from(dy != 0)
        };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let id = w.state([x + dx, y + dy, z + dz]) >> 4;
                if !(matches!(id, 0 | 18 | 161) || dy == 0 && matches!(id, 8 | 9)) {
                    return;
                }
            }
        }
    }
    if w.state([x, y - 1, z]) >> 4 != 3 {
        w.set_state([x, y - 1, z], 3 << 4);
    }
    for dy in -3..=0 {
        let radius = 2 - dy / 2;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if i32::abs(dx) == radius
                    && i32::abs(dz) == radius
                    && (r.bounded(2) == 0 || dy == 0)
                {
                    continue;
                }
                let p = [x + dx, y + height + dy, z + dz];
                if !w.full_block(p) {
                    w.set_state(p, 18 << 4);
                }
            }
        }
    }
    for dy in 0..height {
        let p = [x, y + dy, z];
        if matches!(w.state(p) >> 4, 0 | 18 | 161 | 8 | 9) {
            w.set_state(p, 17 << 4);
        }
    }
    for dy in -3..=0 {
        let radius = 2 - dy / 2;
        let yy = y + height + dy;
        for xx in x - radius..=x + radius {
            for zz in z - radius..=z + radius {
                if !matches!(w.state([xx, yy, zz]) >> 4, 18 | 161) {
                    continue;
                }
                for (dx, dz, meta) in [(-1, 0, 8), (1, 0, 2), (0, -1, 1), (0, 1, 4)] {
                    if r.bounded(4) == 0 {
                        for drop in 0..5 {
                            let p = [xx + dx, yy - drop, zz + dz];
                            if w.state(p) != 0 {
                                break;
                            }
                            w.set_state(p, (106 << 4) | meta);
                        }
                    }
                }
            }
        }
    }
}
