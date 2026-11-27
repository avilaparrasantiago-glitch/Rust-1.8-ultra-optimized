use super::{JavaRandom, simplex::Simplex};
use std::sync::LazyLock;

static TEMPERATURE_NOISE: LazyLock<Simplex> =
    LazyLock::new(|| Simplex::new(&mut JavaRandom::new(1234)));

// Block.getLightOpacity, getLightValue and surface predicates from 1.8.9.
pub(super) fn block_properties(id: u16) -> [u8; 3] {
    let table = include_bytes!("../../FONTS EXTRAS/vectors/T2.4b-4/block-climate.bin")
        .as_chunks::<3>()
        .0;
    table.get(usize::from(id)).copied().unwrap_or([0; 3])
}

pub(super) fn temperature(biome: u8, [x, y, z]: [i32; 3]) -> f32 {
    let base = match biome {
        30 | 31 | 158 => -0.5,
        10..=13 | 140 => 0.0,
        26 => 0.05,
        3 | 20 | 25 | 34 | 131 | 162 => 0.2,
        5 | 19 | 133 | 160 | 161 => 0.25,
        32 | 33 => 0.3,
        27 | 28 | 155 | 156 => 0.6,
        4 | 18 | 29 | 132 | 157 => 0.7,
        1 | 6 | 16 | 129 | 134 => 0.8,
        14 | 15 => 0.9,
        21..=23 | 149 | 151 => 0.95,
        36 | 164 => 1.0,
        163 => 1.1,
        35 => 1.2,
        2 | 8 | 17 | 37..=39 | 130 | 165..=167 => 2.0,
        _ => 0.5,
    };
    if y <= 64 {
        return base;
    }
    let noise = (TEMPERATURE_NOISE.sample(f64::from(x) / 8.0, f64::from(z) / 8.0) * 4.0) as f32;
    base - (noise + y as f32 - 64.0) * 0.05 / 30.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperatures_match_server_float_bits() {
        for line in include_str!("../../FONTS EXTRAS/vectors/T2.4b-4/temperatures.tsv").lines() {
            let mut fields = line.split_whitespace();
            let id = fields.next().unwrap().parse().unwrap();
            let pos = std::array::from_fn(|_| fields.next().unwrap().parse().unwrap());
            let bits = u32::from_str_radix(fields.next().unwrap(), 16).unwrap();
            assert_eq!(temperature(id, pos).to_bits(), bits, "{line}");
        }
    }
}
