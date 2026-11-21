use super::{JavaRandom, simplex::Simplex};
const CLAY: u16 = 172 << 4;
const COLOR: u16 = 159 << 4;
const ORANGE: u16 = COLOR | 1;

pub(super) struct Mesa {
    bands: [u16; 64],
    offset: Simplex,
    pillars: [Simplex; 4],
    cap: Simplex,
}
impl Mesa {
    pub(super) fn new(seed: i64) -> Self {
        let mut r = JavaRandom::new(seed);
        let offset = Simplex::new(&mut r);
        let mut bands = [CLAY; 64];
        let mut i = 0;
        while i < 64 {
            i += r.bounded(5) as usize + 1;
            if i < 64 {
                bands[i] = ORANGE;
            }
            i += 1;
        }
        for (color, min) in [(4, 1), (12, 2), (14, 1)] {
            let count = r.bounded(4) + 2;
            for _ in 0..count {
                let len = (r.bounded(3) + min) as usize;
                let start = r.bounded(64) as usize;
                bands[start..(start + len).min(64)].fill(COLOR | color);
            }
        }
        let mut i = 0;
        for _ in 0..r.bounded(3) + 3 {
            i += r.bounded(16) as usize + 4;
            if i < 64 {
                bands[i] = COLOR;
                if i > 1 && r.bits(1) != 0 {
                    bands[i - 1] = COLOR | 8;
                }
                if i < 63 && r.bits(1) != 0 {
                    bands[i + 1] = COLOR | 8;
                }
            }
        }
        // BiomeGenMesa initializes the Bryce noise with its previous world seed (zero on first use).
        let mut bryce = JavaRandom::new(0);
        Self {
            bands,
            offset,
            pillars: std::array::from_fn(|_| Simplex::new(&mut bryce)),
            cap: Simplex::new(&mut bryce),
        }
    }
    pub(super) fn surface(
        &self,
        states: &mut [u16],
        id: i32,
        world: [i32; 2],
        noise: f64,
        r: &mut JavaRandom,
    ) {
        let [x, z] = world;
        let local = ((x & 15) * 16 + (z & 15)) as usize;
        let mut pillar = 0.;
        if id == 165 {
            let (u, v) = (
                f64::from((x & !15) + (z & 15)),
                f64::from((z & !15) + (x & 15)),
            );
            let mut f = 1.;
            let mut value = 0.;
            for layer in &self.pillars {
                value += layer.sample(u * 0.25 * f, v * 0.25 * f) / f;
                f *= 0.5;
            }
            let n = noise.abs().min(value);
            if n > 0. {
                pillar = (n * n * 2.5)
                    .min((self.cap.sample(u / 512., v / 512.).abs() * 50.).ceil() + 14.)
                    + 64.;
            }
        }
        let shift = (self.offset.sample(f64::from(x) / 512., f64::from(x) / 512.) * 2. + 0.5)
            .floor() as i32;
        let thickness = (noise / 3. + 3. + r.double() * 0.25) as i32;
        let pale = (noise / 3. * std::f64::consts::PI).cos() > 0.;
        let mut remaining = -1;
        let mut sandy = false;
        let (mut top, mut fill) = (COLOR, COLOR);
        for y in (0..256).rev() {
            let block = &mut states[y * 256 + local];
            let y = y as i32;
            if *block == 0 && y < (pillar as i32) {
                *block = 16;
            }
            if y <= r.bounded(5) {
                *block = 7 << 4;
                continue;
            }
            if *block == 0 {
                remaining = -1;
                continue;
            }
            if *block != 16 {
                continue;
            }
            let band = self.bands[((y + shift + 64) & 63) as usize];
            if remaining == -1 {
                sandy = false;
                if thickness <= 0 {
                    top = 0;
                    fill = 16;
                } else if (59..=64).contains(&y) {
                    top = COLOR;
                    fill = COLOR;
                }
                if y < 63 && top == 0 {
                    top = 9 << 4;
                }
                remaining = thickness + (y - 63).max(0);
                *block = if y < 62 {
                    if fill == COLOR { ORANGE } else { fill }
                } else if matches!(id, 38 | 166) && y > 86 + thickness * 2 {
                    if pale { 49 } else { 32 }
                } else if y > 66 + thickness {
                    if !(64..=127).contains(&y) {
                        ORANGE
                    } else if pale {
                        CLAY
                    } else {
                        band
                    }
                } else {
                    sandy = true;
                    (12 << 4) | 1
                };
            } else if remaining > 0 {
                remaining -= 1;
                *block = if sandy { ORANGE } else { band };
            }
        }
    }
}
