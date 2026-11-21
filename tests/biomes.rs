use sha2::{Digest, Sha256};
use syrust_engine::overworld::Biomes;

#[test]
fn both_scales_with_signed_and_high_bit_seeds() {
    for (seed, quarter, blocks) in [
        (
            -9581,
            "ebbbeb62b0f5f5d58620045599b23f2cf86afca8b5972a37878259485bb0ff0c",
            "ea4a3463485324a25467352c7610c28846c077048b8ef8c3a2cf80cafed6df66",
        ),
        (
            281474976723001,
            "7efd7617686bd5c900aef32eae82dcc3e445edc2afd1460b4a8d00cb8fb55992",
            "4d12c251cf45db2092d9549c2528655a7481d1c8a93a2eede308101f2d93d3d5",
        ),
        (
            12345,
            "daa17dd6d5ffe21548f508001849cb1f75084cf32b26e92bbedd95d63f823605",
            "0a89c6fc271a2c59a225b2172663b40a9c4efeb6c16f05f29ca989de39c7a7ba",
        ),
    ] {
        let g = Biomes::new(seed);
        for (size, expected) in [(4, quarter), (16, blocks)] {
            let mut digest = Sha256::new();
            for z in 83..92 {
                let row = if size == 4 {
                    g.quarter(-67 * size, z * size, 11 * size as usize, size as usize)
                } else {
                    g.blocks(-67 * size, z * size, 11 * size as usize, size as usize)
                };
                for chunk in 0..11 {
                    for line in row.chunks_exact(11 * size as usize) {
                        for &id in &line[chunk * size as usize..(chunk + 1) * size as usize] {
                            digest.update([id as u8]);
                        }
                    }
                }
            }
            assert_eq!(
                format!("{:x}", digest.finalize()),
                expected,
                "seed {seed} size {size}"
            );
        }
    }
}

#[test]
fn official_default_biome_vector() {
    let biomes = Biomes::new(12345);
    let mut digest = Sha256::new();
    let width = 101 * 16;
    for cz in -50..=50 {
        let strip = biomes.blocks(-800, cz * 16, width, 16);
        for cx in 0..101 {
            for row in strip.chunks_exact(width) {
                for &id in &row[cx * 16..cx * 16 + 16] {
                    digest.update([id as u8]);
                }
            }
        }
    }
    assert_eq!(
        format!("{:x}", digest.finalize()),
        "79c0c7d67dbd3613d5b0996cc9a5a3a77b09b312dd878c2575b7d69c4437ed39"
    );
}
