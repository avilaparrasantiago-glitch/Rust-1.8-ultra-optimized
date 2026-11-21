use super::JavaRandom;
use std::{f32::consts::PI, sync::OnceLock};

#[derive(Clone, Copy, PartialEq)]
pub enum Carver {
    Caves,
    Ravine,
    Nether,
}
struct Target<'a> {
    x: i32,
    z: i32,
    kind: Carver,
    states: &'a mut [u16],
    biomes: &'a [i32],
}
#[derive(Clone, Copy)]
struct Tunnel {
    point: [f64; 3],
    width: f32,
    yaw: f32,
    pitch: f32,
    start: i32,
    end: i32,
    stretch: f64,
}

fn trig(angle: f32, cosine: bool) -> f32 {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        (0..65536)
            .map(|i| (f64::from(i) * std::f64::consts::PI * 2. / 65536.).sin() as f32)
            .collect()
    });
    table[((angle * 10430.378 + if cosine { 16384. } else { 0. }) as i32 & 65535) as usize]
}
impl JavaRandom {
    fn float(&mut self) -> f32 {
        self.bits(24) as f32 / 16777216.
    }
    fn wobble(&mut self) -> f32 {
        (self.float() - self.float()) * self.float()
    }
}
impl Carver {
    pub fn carve(self, seed: i64, cx: i32, cz: i32, states: &mut [u16], biomes: &[i32]) {
        let mut base = JavaRandom::new(seed);
        let (mx, mz) = (base.long(), base.long());
        let mut target = Target {
            x: cx * 16,
            z: cz * 16,
            kind: self,
            states,
            biomes,
        };
        for x in cx - 8..=cx + 8 {
            for z in cz - 8..=cz + 8 {
                let mut r = JavaRandom::new(
                    seed ^ i64::from(x).wrapping_mul(mx) ^ i64::from(z).wrapping_mul(mz),
                );
                let systems = match self {
                    Self::Ravine => i32::from(r.bounded(50) == 0),
                    _ => {
                        let a = r.bounded(if self == Self::Nether { 10 } else { 15 });
                        let b = r.bounded(a + 1);
                        let count = r.bounded(b + 1);
                        if r.bounded(if self == Self::Nether { 5 } else { 7 }) == 0 {
                            count
                        } else {
                            0
                        }
                    }
                };
                for _ in 0..systems {
                    let px = x * 16 + r.bounded(16);
                    let py = match self {
                        Self::Ravine => {
                            let cap = r.bounded(40) + 8;
                            r.bounded(cap) + 20
                        }
                        Self::Caves => {
                            let cap = r.bounded(120) + 8;
                            r.bounded(cap)
                        }
                        Self::Nether => r.bounded(128),
                    };
                    let pz = z * 16 + r.bounded(16);
                    let point = [px, py, pz].map(f64::from);
                    let mut count = 1;
                    if self != Self::Ravine && r.bounded(4) == 0 {
                        let seed = r.long();
                        let width = 1. + r.float() * 6.;
                        target.tunnel(
                            seed,
                            Tunnel {
                                point,
                                width,
                                yaw: 0.,
                                pitch: 0.,
                                start: -1,
                                end: -1,
                                stretch: 0.5,
                            },
                        );
                        count += r.bounded(4);
                    }
                    for _ in 0..count {
                        let yaw = r.float() * PI * 2.;
                        let pitch = (r.float() - 0.5) * 2. / 8.;
                        let mut width = r.float() * 2. + r.float();
                        if self == Self::Caves && r.bounded(10) == 0 {
                            width *= r.float() * r.float() * 3. + 1.;
                        }
                        if self != Self::Caves {
                            width *= 2.;
                        }
                        let stretch = match self {
                            Self::Nether => 0.5,
                            Self::Ravine => 3.,
                            _ => 1.,
                        };
                        target.tunnel(
                            r.long(),
                            Tunnel {
                                point,
                                width,
                                yaw,
                                pitch,
                                start: 0,
                                end: 0,
                                stretch,
                            },
                        );
                    }
                }
            }
        }
    }
}
impl Target<'_> {
    fn tunnel(&mut self, seed: i64, mut t: Tunnel) {
        let mut r = JavaRandom::new(seed);
        if t.end <= 0 {
            t.end = 112 - r.bounded(28);
        }
        let chamber = t.start == -1;
        if chamber {
            t.start = t.end / 2;
        }
        let ravine = self.kind == Carver::Ravine;
        let mut weights = [0f32; 256];
        let (fork, gentle) = if ravine {
            let mut w = 1.;
            for (y, slot) in weights.iter_mut().enumerate() {
                if y == 0 || r.bounded(3) == 0 {
                    w = 1. + r.float() * r.float();
                }
                *slot = w * w;
            }
            (-1, false)
        } else {
            (r.bounded(t.end / 2) + t.end / 4, r.bounded(6) == 0)
        };
        let (mut yaw_d, mut pitch_d) = (0., 0.);
        for step in t.start..t.end {
            let mut radius =
                1.5 + f64::from(trig(step as f32 * PI / t.end as f32, false) * t.width);
            let mut height = radius * t.stretch;
            if ravine {
                radius *= f64::from(r.float()) * 0.25 + 0.75;
                height *= f64::from(r.float()) * 0.25 + 0.75;
            }
            let horiz = trig(t.pitch, true);
            t.point[0] += f64::from(trig(t.yaw, true) * horiz);
            t.point[1] += f64::from(trig(t.pitch, false));
            t.point[2] += f64::from(trig(t.yaw, false) * horiz);
            t.pitch *= if gentle { 0.92 } else { 0.7 };
            let drift = if ravine { 0.05 } else { 0.1 };
            t.pitch += pitch_d * drift;
            t.yaw += yaw_d * drift;
            pitch_d *= if ravine { 0.8 } else { 0.9 };
            yaw_d *= if ravine { 0.5 } else { 0.75 };
            pitch_d += r.wobble() * 2.;
            yaw_d += r.wobble() * 4.;
            if !chamber && step == fork && t.width > 1. {
                for turn in [-PI / 2., PI / 2.] {
                    let seed = r.long();
                    self.tunnel(
                        seed,
                        Tunnel {
                            width: r.float() * 0.5 + 0.5,
                            yaw: t.yaw + turn,
                            pitch: t.pitch / 3.,
                            start: step,
                            stretch: 1.,
                            ..t
                        },
                    );
                }
                return;
            }
            if !chamber && r.bounded(4) == 0 {
                continue;
            }
            let dx = t.point[0] - f64::from(self.x + 8);
            let dz = t.point[2] - f64::from(self.z + 8);
            if dx * dx + dz * dz - f64::from(t.end - step).powi(2)
                > f64::from(t.width + 18.).powi(2)
            {
                return;
            }
            if dx.abs() > 16. + 2. * radius || dz.abs() > 16. + 2. * radius {
                continue;
            }
            if self.excavate(t.point, radius, height, &weights) && chamber {
                break;
            }
        }
    }
    fn excavate(&mut self, [px, py, pz]: [f64; 3], r: f64, h: f64, weights: &[f32; 256]) -> bool {
        let bounds = |p: f64, origin: i32| {
            (
                ((p - r).floor() as i32 - origin - 1).max(0),
                ((p + r).floor() as i32 - origin + 1).min(16),
            )
        };
        let (x0, x1) = bounds(px, self.x);
        let (z0, z1) = bounds(pz, self.z);
        let nether = self.kind == Carver::Nether;
        let y0 = ((py - h).floor() as i32 - 1).max(1);
        let y1 = ((py + h).floor() as i32 + 1).min(if nether { 120 } else { 248 });
        let at = |x: i32, y: i32, z: i32| (y * 256 + z * 16 + x) as usize;
        for x in x0..x1 {
            for z in z0..z1 {
                for y in y0 - 1..=y1 + 1 {
                    if x != x0
                        && x != x1 - 1
                        && z != z0
                        && z != z1 - 1
                        && y != y0 - 1
                        && y != y1 + 1
                    {
                        continue;
                    }
                    let id = self.states[at(x, y, z)] >> 4;
                    if if nether {
                        matches!(id, 10 | 11)
                    } else {
                        matches!(id, 8 | 9)
                    } {
                        return false;
                    }
                }
            }
        }
        for x in x0..x1 {
            let dx = (f64::from(x + self.x) + 0.5 - px) / r;
            for z in z0..z1 {
                let dz = (f64::from(z + self.z) + 0.5 - pz) / r;
                let horizontal = dx * dx + dz * dz;
                if horizontal >= 1. {
                    continue;
                }
                let mut exposed = false;
                for y in (y0 + 1..=y1).rev() {
                    let dy = (f64::from(y) - 0.5 - py) / h;
                    let inside = if self.kind == Carver::Ravine {
                        horizontal * f64::from(weights[(y - 1) as usize]) + dy * dy / 6. < 1.
                    } else {
                        dy > -0.7 && dx * dx + dy * dy + dz * dz < 1.
                    };
                    if !inside {
                        continue;
                    }
                    let i = at(x, y, z);
                    let id = self.states[i] >> 4;
                    let above = self.states[i + 256] >> 4;
                    exposed |= id == 2 || (self.kind == Carver::Caves && id == 110);
                    let removable = match self.kind {
                        Carver::Nether => matches!(id, 2 | 3 | 87),
                        Carver::Ravine => matches!(id, 1..=3),
                        Carver::Caves => {
                            matches!(id, 1..=3 | 159 | 172 | 24 | 179 | 110 | 80)
                                || (matches!(id, 12 | 13) && !matches!(above, 8 | 9))
                        }
                    };
                    if !removable {
                        continue;
                    }
                    self.states[i] = if !nether && y <= 10 {
                        if self.kind == Carver::Ravine {
                            10 << 4
                        } else {
                            11 << 4
                        }
                    } else {
                        0
                    };
                    if self.states[i] == 0 && !nether {
                        if self.kind == Carver::Caves && above == 12 {
                            self.states[i + 256] = if self.states[i + 256] & 15 == 1 {
                                179 << 4
                            } else {
                                24 << 4
                            };
                        }
                        if exposed && self.states[i - 256] >> 4 == 3 {
                            self.states[i - 256] = match self.biomes[(z * 16 + x) as usize] {
                                14 | 15 => 110 << 4,
                                2 | 17 | 16 | 26 | 130 => 12 << 4,
                                25 => 16,
                                140 => 80 << 4,
                                _ => 2 << 4,
                            };
                        }
                    }
                }
            }
        }
        true
    }
}
