use super::JavaRandom;

struct Improved {
    offset: [f64; 3],
    perm: [u8; 256],
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

fn lattice(v: f64) -> (usize, f64, f64) {
    let floor = v.floor();
    let u = v - floor;
    (
        (floor as i64 & 255) as usize,
        u,
        u * u * u * (u * (u * 6.0 - 15.0) + 10.0),
    )
}

fn grad(hash: u8, x: f64, y: f64, z: f64) -> f64 {
    const G: [[i8; 3]; 16] = [
        [1, 1, 0],
        [-1, 1, 0],
        [1, -1, 0],
        [-1, -1, 0],
        [1, 0, 1],
        [-1, 0, 1],
        [1, 0, -1],
        [-1, 0, -1],
        [0, 1, 1],
        [0, -1, 1],
        [0, 1, -1],
        [0, -1, -1],
        [1, 1, 0],
        [0, -1, 1],
        [-1, 1, 0],
        [0, -1, -1],
    ];
    let [a, b, c] = G[(hash & 15) as usize];
    f64::from(a) * x + f64::from(b) * y + f64::from(c) * z
}

impl Improved {
    fn new(rng: &mut JavaRandom) -> Self {
        let offset = std::array::from_fn(|_| rng.double() * 256.0);
        let mut perm = std::array::from_fn(|i| i as u8);
        for i in 0..256 {
            let j = i + rng.bounded((256 - i) as i32) as usize;
            perm.swap(i, j);
        }
        Self { offset, perm }
    }

    fn add(
        &self,
        out: &mut [f64],
        origin: [f64; 3],
        size: [usize; 3],
        step: [f64; 3],
        weight: f64,
    ) {
        let p = |i: usize| self.perm[i & 255] as usize;
        let mut index = 0;
        for ix in 0..size[0] {
            let (x, dx, fx) = lattice(origin[0] + ix as f64 * step[0] + self.offset[0]);
            for iz in 0..size[2] {
                let (z, dz, fz) = lattice(origin[2] + iz as f64 * step[2] + self.offset[2]);
                let mut previous = None;
                let mut edge = [[0.0; 2]; 2];
                for iy in 0..size[1] {
                    let (y, dy, fy) = if size[1] == 1 {
                        (0, 0.0, 0.0)
                    } else {
                        lattice(origin[1] + iy as f64 * step[1] + self.offset[1])
                    };
                    // Vanilla retains its four X interpolants until the Y lattice changes.
                    if previous != Some(y) {
                        for (k, row) in edge.iter_mut().enumerate() {
                            for (j, value) in row.iter_mut().enumerate() {
                                let gradient = |i: usize| {
                                    let hash = p(p(p(x + i) + y + j) + z + k) as u8;
                                    grad(hash, dx - i as f64, dy - j as f64, dz - k as f64)
                                };
                                *value = lerp(fx, gradient(0), gradient(1));
                            }
                        }
                        previous = Some(y);
                    }
                    out[index] += lerp(
                        fz,
                        lerp(fy, edge[0][0], edge[0][1]),
                        lerp(fy, edge[1][0], edge[1][1]),
                    ) * weight;
                    index += 1;
                }
            }
        }
    }
}

pub struct Octaves(Vec<Improved>);

impl Octaves {
    pub fn new(seed: i64, count: usize) -> Self {
        let mut rng = JavaRandom::new(seed);
        Self::from_random(&mut rng, count)
    }
    pub(super) fn from_random(rng: &mut JavaRandom, count: usize) -> Self {
        Self((0..count).map(|_| Improved::new(rng)).collect())
    }

    /// Fill Java's X, Z, Y order. The caller can reuse the same buffer across chunks.
    pub fn fill(&self, out: &mut [f64], origin: [i32; 3], size: [usize; 3], scale: [f64; 3]) {
        assert_eq!(
            Some(out.len()),
            size.into_iter().try_fold(1usize, usize::checked_mul)
        );
        out.fill(0.0);
        let mut frequency = 1.0;
        for layer in &self.0 {
            let mut start = std::array::from_fn(|i| (f64::from(origin[i]) * frequency) * scale[i]);
            for i in [0, 2] {
                let whole = start[i].floor();
                start[i] = (start[i] - whole) + (whole as i64 % 16777216) as f64;
            }
            layer.add(
                out,
                start,
                size,
                scale.map(|s| s * frequency),
                1.0 / frequency,
            );
            frequency *= 0.5;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_octave_arrays() {
        let noise = Octaves::new(-9581, 4);
        let scale = [0.13, 0.2, 0.37];
        let mut surface = [0.0; 4];
        noise.fill(&mut surface, [-17, 3, 27], [2, 1, 2], scale);
        assert_eq!(
            surface.map(f64::to_bits),
            [
                0x3fd01df0ec64b4c0,
                0xbfe6754dc6ce16e8,
                0x3fc1137da5e6a49c,
                0xbfe85d5bee5dfd98
            ]
        );
        let mut volume = [0.0; 20];
        noise.fill(&mut volume, [-17, 3, 27], [2, 5, 2], scale);
        assert_eq!(
            volume.map(f64::to_bits),
            [
                0x3ff6d12dc07f8f0c,
                0x3ff112793714bb3e,
                0x3ff23456e9c1f758,
                0x3ff473a1366ad673,
                0x3ff3f238c26de38a,
                0x40038d44fe71bbf8,
                0x400017da9ee7af76,
                0x3ffd6a954010d41f,
                0x3ffa1f2a994c9270,
                0x3ff4e509c658ce2e,
                0x3ff755e692d4f278,
                0x3ff23b4d2c5dc17a,
                0x3ff48860e3b96962,
                0x3ff82a0508d3eba0,
                0x3ff8a9bde84b4bb9,
                0x4002d501cbe85e56,
                0x3fffc3b8d2afc0d7,
                0x3fffa0fcb57c49c4,
                0x3fffa5b45f804f40,
                0x3ffcbdfc569c26a0
            ]
        );
    }
}
