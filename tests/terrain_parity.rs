use sha2::{Digest, Sha256};
use syrust_engine::overworld::Carver;
use syrust_engine::overworld::Overworld;

#[test]
fn official_spawn_search() {
    assert_eq!(Overworld::new(12345).spawn(), [-243, 84, 130]);
    assert_eq!(Overworld::new(-9581).spawn(), [-826, 64, -331]);
}

#[test]
fn official_stronghold_starts() {
    assert_eq!(
        Overworld::new(12345).stronghold_starts(),
        [(-44, 59), (-16, -38), (61, 18)]
    );
    assert_eq!(
        Overworld::new(-9581).stronghold_starts(),
        [(47, 29), (-49, 25), (3, -54)]
    );
}

#[test]
fn official_biome_surfaces() {
    let g = Overworld::new(12345);
    let mut differences = Vec::new();
    for line in include_str!("fixtures/overworld-12345.txt")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        let x: i32 = fields[1].parse().unwrap();
        let z: i32 = fields[2].parse().unwrap();
        let mut blocks = g.set_blocks_in_chunk(x, z);
        if hash(&blocks) != fields[3] {
            differences.push((fields[0], "density", x, z));
        }
        g.replace_blocks_for_biome(x, z, &g.biomes.blocks(x * 16, z * 16, 16, 16), &mut blocks);
        if hash(&blocks) != fields[4] {
            differences.push((fields[0], "surface", x, z));
        }
    }
    assert!(differences.is_empty(), "{differences:?}");
}

#[test]
fn official_nether_surface_and_caves() {
    let g = DimensionTerrain::new(12345, Dimension::Nether);
    for (x, expected) in [
        (
            0,
            "1a528b1b1395c2881b9be87f3a7c360447d6a594e5c28ea015879490a4115acb",
        ),
        (
            -7,
            "0413467899036bde1e6acc7f4c19701de7cd235a097e84278ce377d3744c5b4f",
        ),
    ] {
        assert_eq!(hash(&g.provide(x, 0)), expected, "nether {x}");
    }
}

#[test]
fn official_carved_primers() {
    let g = Overworld::new(12345);
    for (x, expected) in [
        (
            0,
            "57b6e92c0f143c24614108c7f5d8e27ba040a7c4acd17587c5c594184e863b11",
        ),
        (
            -7,
            "565668cfd356735097bdf59876002be58ba9d6dcdebafa726f14364c8f672391",
        ),
        (
            8,
            "040b0a1573facc50995f1d00a0c6a115d26bf27474e723160cfb7dad02460072",
        ),
    ] {
        let mut blocks = g.set_blocks_in_chunk(x, 0);
        let ids = g.biomes.blocks(x * 16, 0, 16, 16);
        g.replace_blocks_for_biome(x, 0, &ids, &mut blocks);
        Carver::Caves.carve(12345, x, 0, &mut blocks, &ids);
        assert_eq!(hash(&blocks), expected, "caves {x}");
        Carver::Ravine.carve(12345, x, 0, &mut blocks, &ids);
        // The reference ravine pass leaves the two western primers untouched.
        let after_ravines = if x == 8 {
            "b2ec1522c9ff29bd26dbbb5b7c68dd8177c800f93ff93ce50ec12265d604f50c"
        } else {
            expected
        };
        assert_eq!(hash(&blocks), after_ravines, "ravines {x}");
    }
}
use syrust_engine::overworld::{Dimension, DimensionTerrain};

#[test]
fn official_dimension_primers() {
    for (kind, hashes) in [
        (
            Dimension::Nether,
            [
                "85f10c2f388c0e2f86c61209d1900eb97ee3e788ad2903fd7db105e0af3cbf4e",
                "26e5674efa939fd20168c98695d1a44eb9755e2eab05b249976665589870cee7",
                "e5d4581c4cda1d9fc8e6ed0a2db36675b40954063147e4128cef2f4428d34c21",
            ],
        ),
        (
            Dimension::End,
            [
                "d191139929308558c95b561a6e0c9a8f00eb53f6e157e7e9287535fac926e150",
                "456199c51aec6e5ebb6200091fe0c600075870e1d913e784af37652a0cbb0bfd",
                "fa43239bcee7b97ca62f007cc68487560a39e19f74f3dde7486db3f98df8e471",
            ],
        ),
    ] {
        let g = DimensionTerrain::new(12345, kind);
        for (x, expected) in [0, -7, 99].into_iter().zip(hashes) {
            assert_eq!(hash(&g.terrain(x, 0)), expected, "chunk {x}");
        }
    }
}
fn hash(states: &[u16]) -> String {
    let mut digest = Sha256::new();
    for v in states {
        digest.update(v.to_be_bytes());
    }
    format!("{:x}", digest.finalize())
}
#[test]
fn official_overworld_primers() {
    let g = Overworld::new(12345);
    for (x, base, surface) in [
        (
            0,
            "2b8fceacbd89e27d3145dff95703fe0c22ddb2eeee758936a04a1d8382c3924f",
            "844ba9fd17680383c2f7ba0143b27544909c738b8c6bbe05f43cd3c43b237672",
        ),
        (
            -7,
            "3c727f6c6b27791528621a563d0c4213e19d86543ba8ee2e71e11c5b36fdecb7",
            "64828cc7f5c63694890b991b023118de1f5c15729bb78a6e3ab164b7f554f013",
        ),
    ] {
        let mut blocks = g.set_blocks_in_chunk(x, 0);
        assert_eq!(hash(&blocks), base, "base {x}");
        g.replace_blocks_for_biome(x, 0, &g.biomes.blocks(x * 16, 0, 16, 16), &mut blocks);
        assert_eq!(hash(&blocks), surface, "surface {x}");
    }
}
