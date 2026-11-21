use super::JavaRandom;
pub(super) struct Simplex {
    offset: [f64; 3],
    p: [u8; 256],
}
impl Simplex {
    pub(super) fn new(r: &mut JavaRandom) -> Self {
        let offset = std::array::from_fn(|_| r.double() * 256.0);
        let mut p = std::array::from_fn(|i| i as u8);
        for i in 0..256 {
            let j = i + r.bounded((256 - i) as i32) as usize;
            p.swap(i, j);
        }
        Self { offset, p }
    }
    pub(super) fn shifted(&self, x: f64, z: f64) -> f64 {
        self.sample(x + self.offset[0], z + self.offset[1])
    }
    pub(super) fn sample(&self, x: f64, z: f64) -> f64 {
        let skew = (3f64.sqrt() - 1.0) * 0.5;
        let unskew = (3.0 - 3f64.sqrt()) / 6.0;
        let s = (x + z) * skew;
        let floor = |n: f64| if n > 0.0 { n as i32 } else { n as i32 - 1 };
        let (i, j) = (floor(x + s), floor(z + s));
        let t = f64::from(i + j) * unskew;
        let (u, v) = (x - (f64::from(i) - t), z - (f64::from(j) - t));
        let (a, b) = if u > v { (1, 0) } else { (0, 1) };
        let corner = |di: i32, dj: i32, dx: f64, dz: f64| {
            let weight = 0.5 - dx * dx - dz * dz;
            if weight < 0.0 {
                return 0.0;
            }
            let h = self.p
                [((i + di + i32::from(self.p[((j + dj) & 255) as usize])) & 255) as usize]
                % 12;
            let grad = match h {
                0 => dx + dz,
                1 => -dx + dz,
                2 => dx - dz,
                3 => -dx - dz,
                4 | 6 => dx,
                5 | 7 => -dx,
                8 | 10 => dz,
                _ => -dz,
            };
            let w = weight * weight;
            w * w * grad
        };
        70.0 * (corner(0, 0, u, v)
            + corner(a, b, u - f64::from(a) + unskew, v - f64::from(b) + unskew)
            + corner(1, 1, u - 1.0 + 2.0 * unskew, v - 1.0 + 2.0 * unskew))
    }
}
