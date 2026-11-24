use super::{JavaRandom, biomes::BiomeWorld};
type Pos = [i32; 3];

fn line(a: Pos, b: Pos) -> impl Iterator<Item = Pos> {
    let delta = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
    let length = delta.into_iter().map(i32::abs).max().unwrap();
    let steps = delta.map(|d| d as f32 / length.max(1) as f32);
    (0..=length)
        .map(move |n| std::array::from_fn(|i| a[i] + (0.5 + n as f32 * steps[i]).floor() as i32))
}

fn obstruction(w: &impl BiomeWorld, a: Pos, b: Pos) -> Option<usize> {
    if a == b {
        return None;
    }
    line(a, b).position(|p| !matches!(w.state(p) >> 4, 0 | 2 | 3 | 6 | 17 | 18 | 106 | 161 | 162))
}

fn logs(w: &mut impl BiomeWorld, a: Pos, b: Pos) {
    for p in line(a, b) {
        let dx = (p[0] - a[0]).abs();
        let dz = (p[2] - a[2]).abs();
        let axis = if dx == 0 && dz == 0 {
            0
        } else if dx >= dz {
            4
        } else {
            8
        };
        w.set_state(p, 17 << 4 | axis);
    }
}

/// WorldGenBigTree keeps its height limit across invocations on the
/// same biome's generator, including after a rejected placement.
pub(super) fn generate(
    w: &mut impl BiomeWorld,
    stream: &mut JavaRandom,
    [x, y, z]: Pos,
    limit: &mut i32,
) {
    let mut r = JavaRandom::new(stream.long());
    if *limit == 0 {
        *limit = 5 + r.bounded(12);
    }
    if !matches!(w.state([x, y - 1, z]) >> 4, 2 | 3 | 60) {
        return;
    }
    if let Some(hit) = obstruction(w, [x, y, z], [x, y + *limit - 1, z]) {
        if hit < 6 {
            return;
        }
        *limit = hit as i32;
    }
    let h = *limit;
    let trunk = (f64::from(h) * 0.618) as i32;
    let clusters = (1.382 + (f64::from(h) / 13.).powi(2)) as i32;
    let mut nodes = vec![([x, y + h - 5, z], y + trunk)];
    for layer in (0..=h - 5).rev() {
        if (layer as f32) < h as f32 * 0.3 {
            continue;
        }
        let half = h as f32 / 2.;
        let delta = half - layer as f32;
        let width = (half * half - delta * delta).sqrt() * 0.5;
        for _ in 0..clusters {
            let radius = f64::from(width) * (f64::from(r.float()) + 0.328);
            let angle = f64::from(r.float() * 2.) * std::f64::consts::PI;
            let xx = (f64::from(x) + radius * angle.sin() + 0.5).floor() as i32;
            let zz = (f64::from(z) + radius * angle.cos() + 0.5).floor() as i32;
            let yy = y + layer - 1;
            let node = [xx, yy, zz];
            if obstruction(w, node, [xx, yy + 5, zz]).is_some() {
                continue;
            }
            let distance = f64::from((x - xx).pow(2) + (z - zz).pow(2)).sqrt();
            let branch_y = (f64::from(yy) - distance * 0.381).min(f64::from(y + trunk)) as i32;
            if obstruction(w, [x, branch_y, z], node).is_none() {
                nodes.push((node, branch_y));
            }
        }
    }
    for &([xx, yy, zz], _) in &nodes {
        for dy in 0..5 {
            let radius = if dy == 0 || dy == 4 { 2i32 } else { 3 };
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let p = [xx + dx, yy + dy, zz + dz];
                    if (f64::from(dx.abs()) + 0.5).powi(2) + (f64::from(dz.abs()) + 0.5).powi(2)
                        <= f64::from(radius * radius)
                        && matches!(w.state(p) >> 4, 0 | 18 | 161)
                    {
                        w.set_state(p, 18 << 4);
                    }
                }
            }
        }
    }
    logs(w, [x, y, z], [x, y + trunk, z]);
    for (node, branch_y) in nodes {
        let from = [x, branch_y, z];
        if from != node && f64::from(branch_y - y) >= f64::from(h) * 0.2 {
            logs(w, from, node);
        }
    }
}
