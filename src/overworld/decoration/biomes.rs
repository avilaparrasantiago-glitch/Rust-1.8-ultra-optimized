use super::{
    DecorationWorld, decorate_ground,
    plants::{Plant, PlantWorld},
};
use crate::overworld::JavaRandom;
type Pos = [i32; 3];

pub trait BiomeWorld: PlantWorld + DecorationWorld {
    /// Material.isReplaceable, used by Block.canPlaceBlockAt for melons.
    fn replaceable(&self, pos: Pos) -> bool {
        matches!(
            self.state(pos) >> 4,
            0 | 8..=11 | 31 | 32 | 51 | 78 | 106 | 175
        )
    }
    /// Y from World.getHeight at the time of the query.
    fn height(&self, x: i32, z: i32) -> i32;
    /// World.forceBlockUpdateTick, immediately after placing a spring.
    fn tick_spring(&mut self, pos: Pos, fluid: u16, random: &mut JavaRandom);
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Biome {
    ExtremeHills = 3,
    ExtremeHillsEdge = 20,
    ExtremeHillsPlus = 34,
    Savanna = 35,
    SavannaPlateau = 36,
    Mesa = 37,
    MesaPlateauF = 38,
    MesaPlateau = 39,
    ExtremeHillsM = 131,
    ExtremeHillsPlusM = 162,
    SavannaM = 163,
    SavannaPlateauM = 164,
    MesaBryce = 165,
    MesaPlateauFM = 166,
    MesaPlateauM = 167,
    Plains = 1,
    SunflowerPlains = 129,
    FlowerForest = 132,
    Ocean = 0,
    Desert = 2,
    Forest = 4,
    Taiga = 5,
    Swamp = 6,
    River = 7,
    FrozenOcean = 10,
    FrozenRiver = 11,
    IcePlains = 12,
    IceMountains = 13,
    IcePlainsSpikes = 140,
    MushroomIsland = 14,
    MushroomShore = 15,
    Beach = 16,
    DesertHills = 17,
    ForestHills = 18,
    Jungle = 21,
    JungleHills = 22,
    JungleEdge = 23,
    JungleM = 149,
    JungleEdgeM = 151,
    TaigaHills = 19,
    MegaTaiga = 32,
    MegaTaigaHills = 33,
    MegaSpruceTaiga = 160,
    MegaSpruceTaigaHills = 161,
    DeepOcean = 24,
    StoneBeach = 25,
    ColdBeach = 26,
    BirchForest = 27,
    BirchForestHills = 28,
    RoofedForest = 29,
    RoofedForestM = 157,
    ColdTaiga = 30,
    ColdTaigaHills = 31,
    DesertM = 130,
    TaigaM = 133,
    SwampM = 134,
    BirchForestM = 155,
    BirchForestHillsM = 156,
    ColdTaigaM = 158,
}

fn column(r: &mut JavaRandom, [x, z]: [i32; 2]) -> [i32; 2] {
    [x.wrapping_add(r.bounded(16)), z.wrapping_add(r.bounded(16))]
}
fn random_height(
    w: &impl BiomeWorld,
    r: &mut JavaRandom,
    base: [i32; 2],
    flower: bool,
) -> Option<Pos> {
    let [x, z] = column(r, base);
    let height = w.height(x, z);
    let bound = if flower { height + 32 } else { height * 2 };
    (bound > 0).then(|| [x, r.bounded(bound), z])
}

fn spring(w: &mut impl BiomeWorld, r: &mut JavaRandom, [x, y, z]: Pos, id: u16) {
    if w.state([x, y + 1, z]) >> 4 != 1
        || w.state([x, y - 1, z]) >> 4 != 1
        || !matches!(w.state([x, y, z]) >> 4, 0 | 1)
    {
        return;
    }
    let neighbors =
        [[x - 1, y, z], [x + 1, y, z], [x, y, z - 1], [x, y, z + 1]].map(|p| w.state(p) >> 4);
    if neighbors.iter().filter(|&&n| n == 1).count() == 3 && neighbors.contains(&0) {
        w.set_state([x, y, z], id << 4);
        w.tick_spring([x, y, z], id, r);
    }
}
fn springs(w: &mut impl BiomeWorld, r: &mut JavaRandom, base: [i32; 2]) {
    for _ in 0..50 {
        let [x, z] = column(r, base);
        let bound = r.bounded(248) + 8;
        let y = r.bounded(bound);
        spring(w, r, [x, y, z], 8);
    }
    for _ in 0..20 {
        let [x, z] = column(r, base);
        let inner = r.bounded(240) + 8;
        let outer = r.bounded(inner) + 8;
        let y = r.bounded(outer);
        spring(w, r, [x, y, z], 10);
    }
}
fn desert_well(w: &mut impl BiomeWorld, [x, mut y, z]: Pos) {
    while y > 2 && w.state([x, y, z]) == 0 {
        y -= 1;
    }
    if w.state([x, y, z]) != 12 << 4 {
        return;
    }
    for dx in -2..=2 {
        for dz in -2..=2 {
            if w.state([x + dx, y - 1, z + dz]) == 0 && w.state([x + dx, y - 2, z + dz]) == 0 {
                return;
            }
        }
    }
    let sandstone = 24 << 4;
    let slab = 44 << 4 | 1;
    for dy in -1..=0 {
        for dx in -2..=2 {
            for dz in -2..=2 {
                w.set_state([x + dx, y + dy, z + dz], sandstone);
            }
        }
    }
    for [dx, dz] in [[0, 0], [0, -1], [1, 0], [0, 1], [-1, 0]] {
        w.set_state([x + dx, y, z + dz], 8 << 4);
    }
    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            if dx.abs() == 2 || dz.abs() == 2 {
                w.set_state([x + dx, y + 1, z + dz], sandstone);
            }
        }
    }
    for [dx, dz] in [[2, 0], [-2, 0], [0, 2], [0, -2]] {
        w.set_state([x + dx, y + 1, z + dz], slab);
    }
    for dx in -1..=1 {
        for dz in -1..=1 {
            w.set_state(
                [x + dx, y + 4, z + dz],
                if dx == 0 && dz == 0 { sandstone } else { slab },
            );
        }
    }
    for dy in 1..=3 {
        for dx in [-1, 1] {
            for dz in [-1, 1] {
                w.set_state([x + dx, y + dy, z + dz], sandstone);
            }
        }
    }
}

#[derive(Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecoratorState {
    // BiomeGenBase owns and reuses these generator instances.
    oak_heights: std::collections::BTreeMap<u8, i32>,
    mushrooms: [Option<bool>; 2],
}
fn flower_noise(x: i32, z: i32, scale: f64) -> f64 {
    use crate::overworld::simplex::Simplex;
    static NOISE: std::sync::OnceLock<Simplex> = std::sync::OnceLock::new();
    NOISE
        .get_or_init(|| Simplex::new(&mut JavaRandom::new(2345)))
        .sample(f64::from(x) / scale, f64::from(z) / scale)
}
impl Biome {
    pub fn from_id(id: u8) -> Option<Self> {
        Some(match id {
            0 => Self::Ocean,
            3 => Self::ExtremeHills,
            20 => Self::ExtremeHillsEdge,
            34 => Self::ExtremeHillsPlus,
            35 => Self::Savanna,
            36 => Self::SavannaPlateau,
            37 => Self::Mesa,
            38 => Self::MesaPlateauF,
            39 => Self::MesaPlateau,
            131 => Self::ExtremeHillsM,
            162 => Self::ExtremeHillsPlusM,
            163 => Self::SavannaM,
            164 => Self::SavannaPlateauM,
            165 => Self::MesaBryce,
            166 => Self::MesaPlateauFM,
            167 => Self::MesaPlateauM,
            1 => Self::Plains,
            129 => Self::SunflowerPlains,
            132 => Self::FlowerForest,
            2 => Self::Desert,
            4 => Self::Forest,
            5 => Self::Taiga,
            6 => Self::Swamp,
            7 => Self::River,
            10 => Self::FrozenOcean,
            11 => Self::FrozenRiver,
            12 => Self::IcePlains,
            13 => Self::IceMountains,
            140 => Self::IcePlainsSpikes,
            14 => Self::MushroomIsland,
            15 => Self::MushroomShore,
            16 => Self::Beach,
            17 => Self::DesertHills,
            18 => Self::ForestHills,
            21 => Self::Jungle,
            22 => Self::JungleHills,
            23 => Self::JungleEdge,
            149 => Self::JungleM,
            151 => Self::JungleEdgeM,
            19 => Self::TaigaHills,
            32 => Self::MegaTaiga,
            33 => Self::MegaTaigaHills,
            160 => Self::MegaSpruceTaiga,
            161 => Self::MegaSpruceTaigaHills,
            24 => Self::DeepOcean,
            25 => Self::StoneBeach,
            26 => Self::ColdBeach,
            27 => Self::BirchForest,
            28 => Self::BirchForestHills,
            29 => Self::RoofedForest,
            157 => Self::RoofedForestM,
            30 => Self::ColdTaiga,
            31 => Self::ColdTaigaHills,
            130 => Self::DesertM,
            133 => Self::TaigaM,
            134 => Self::SwampM,
            155 => Self::BirchForestM,
            156 => Self::BirchForestHillsM,
            158 => Self::ColdTaigaM,
            _ => return None,
        })
    }
    /// BiomeGenBase.decorate for the selected biome, including the
    /// BiomeGenDesert override. Mutated desert delegates only the decorator.
    pub fn decorate(
        self,
        generators: &mut DecoratorState,
        w: &mut impl BiomeWorld,
        r: &mut JavaRandom,
        chunk: [i32; 2],
    ) {
        let owner = match self {
            Self::DesertM => 2,
            Self::JungleM => 21,
            Self::JungleEdgeM => 23,
            Self::SwampM => 6,
            Self::TaigaM => 5,
            Self::ColdTaigaM => 30,
            Self::BirchForestM => 27,
            Self::BirchForestHillsM => 28,
            Self::RoofedForestM => 29,
            _ => self as u8,
        };
        let ocean_or_river = matches!(owner, 0 | 7 | 10 | 11 | 24);
        let hills = matches!(owner, 3 | 20 | 34 | 131 | 162);
        let savanna = matches!(owner, 35 | 36 | 163 | 164);
        let island = matches!(owner, 14 | 15);
        let jungle = matches!(owner, 21..=23);
        let jungle_decorate = matches!(self, Self::Jungle | Self::JungleHills | Self::JungleEdge);
        let base = chunk.map(|n| n.wrapping_mul(16).wrapping_add(8));
        let forest = matches!(owner, 4 | 18 | 27 | 28 | 29 | 132);
        if owner == 29 {
            super::roofed::grove(w, r, base);
        }
        let mutated = matches!(self, Self::BirchForestM | Self::BirchForestHillsM);
        if forest && !mutated {
            super::forest::double_flowers(w, r, base, if owner == 132 { 2 } else { 0 });
        }
        let plains = matches!(self, Self::Plains | Self::SunflowerPlains);
        let tulips = plains && flower_noise(base[0], base[1], 200.) < -0.8;
        if plains && !tulips {
            for _ in 0..7 {
                super::forest::double_patch(w, r, base, 2);
            }
        }
        if matches!(self, Self::SunflowerPlains) {
            for _ in 0..10 {
                super::forest::double_patch(w, r, base, 0);
            }
        }
        let mega = matches!(owner, 32 | 33 | 160 | 161);
        let taiga = mega || matches!(owner, 5 | 19 | 30 | 31);
        if mega {
            for _ in 0..r.bounded(3) {
                let [x, z] = column(r, base);
                super::huge::mossy_rock(w, r, [x, w.height(x, z), z]);
            }
        }
        let swamp = owner == 6;
        let ice = matches!(owner, 12 | 13 | 140);
        if matches!(self, Self::IcePlainsSpikes) {
            super::ice::decorate(w, r, base);
        }
        if taiga && !matches!(self, Self::TaigaM | Self::ColdTaigaM) {
            for _ in 0..7 {
                super::forest::double_patch(w, r, base, 3);
            }
        }
        if matches!(owner, 35 | 36) {
            for _ in 0..7 {
                super::forest::double_patch(w, r, base, 2);
            }
        }
        decorate_ground(w, r, chunk, owner);
        // Counts in BiomeDecorator field order (after trees, the passes run
        // flowers, grass, dead bushes, lilies, mushrooms, reeds and cacti).
        let (trees, flowers, grass, bushes, lilies, mushrooms, reeds, cacti) = match owner {
            1 | 129 if tulips => (-999, 15, 5, 0, 0, 0, 0, 0),
            1 | 129 => (-999, 4, 10, 0, 0, 0, 0, 0),
            2 | 17 => (-999, 2, 1, 2, 0, 0, 50, 10),
            4 | 18 | 27 | 28 => (10, 2, 2, 0, 0, 0, 0, 0),
            29 => (-999, 2, 2, 0, 0, 0, 0, 0),
            21 | 22 => (50, 4, 25, 0, 0, 0, 0, 0),
            23 => (2, 4, 25, 0, 0, 0, 0, 0),
            132 => (6, 100, 1, 0, 0, 0, 0, 0),
            5 | 19 | 30 | 31 => (10, 2, 1, 0, 0, 1, 0, 0),
            32 | 33 | 160 | 161 => (10, 2, 7, 1, 0, 3, 0, 0),
            6 => (2, 1, 5, 1, 4, 8, 10, 0),
            14 | 15 => (-100, -100, -100, 0, 0, 1, 0, 0),
            16 | 25 | 26 => (-999, 2, 1, 0, 0, 0, 0, 0),
            20 | 34 => (3, 2, 1, 0, 0, 0, 0, 0),
            35 | 36 => (1, 4, 20, 0, 0, 0, 0, 0),
            163 | 164 => (2, 2, 5, 0, 0, 0, 0, 0),
            37 | 39 | 165 | 167 => (-999, 0, 1, 20, 0, 0, 3, 5),
            38 | 166 => (5, 0, 1, 20, 0, 0, 3, 5),
            _ => (0, 2, 1, 0, 0, 0, 0, 0),
        };
        let attempts = trees + i32::from(r.bounded(10) == 0);
        for _ in 0..attempts {
            let [x, z] = column(r, base);
            let p = [x, w.height(x, z), z];
            if jungle {
                if r.bounded(10) == 0 {
                    super::big_oak::generate(
                        w,
                        r,
                        p,
                        generators.oak_heights.entry(owner).or_default(),
                    );
                } else if r.bounded(2) == 0 {
                    super::jungle::shrub(w, r, p);
                } else if owner != 23 && r.bounded(3) == 0 {
                    super::jungle::giant(w, r, p);
                } else {
                    let height = 4 + r.bounded(7) + r.bounded(3);
                    super::forest::small_tree(w, r, p, height, 3);
                }
                continue;
            }
            if taiga || ice {
                if mega && r.bounded(3) == 0 {
                    let long_crown = matches!(owner, 160 | 161) || r.bounded(13) == 0;
                    super::huge::spruce(w, r, p, long_crown);
                    continue;
                }
                let pine = !ice && r.bounded(3) == 0;
                super::forest::conifer(w, r, p, pine);
                continue;
            }
            if swamp {
                super::forest::swamp_tree(w, r, p);
                continue;
            }
            if hills && r.bounded(3) > 0 {
                super::forest::conifer(w, r, p, false);
                continue;
            }
            if savanna && r.bounded(5) > 0 {
                super::acacia::generate(w, r, p);
                continue;
            }
            if (ocean_or_river || hills) && r.bounded(10) == 0 {
                super::big_oak::generate(w, r, p, generators.oak_heights.entry(owner).or_default());
                continue;
            }
            let birch = forest && (matches!(owner, 27 | 28) || r.bounded(5) == 0);
            let tall = mutated && r.bounded(2) != 0;
            super::forest::tree(w, r, [x, w.height(x, z), z], birch, tall);
        }
        if island {
            let [x, z] = column(r, base);
            let brown = *generators.mushrooms[usize::from(owner - 14)]
                .get_or_insert_with(|| r.bounded(2) != 0);
            super::mushrooms::giant(w, r, [x, w.height(x, z), z], brown);
        }
        for _ in 0..flowers {
            if let Some(p) = random_height(w, r, base, true) {
                let state = if owner == 132 {
                    let index = (((1. + flower_noise(p[0], p[2], 48.)) * 0.5).clamp(0., 0.9999)
                        * 10.) as u16;
                    match index {
                        0 => 37 << 4,
                        2 => 38 << 4,
                        _ => (38 << 4) | (index - 1),
                    }
                } else if plains {
                    if flower_noise(p[0], p[2], 200.) < -0.8 {
                        (38 << 4) | [5, 4, 7, 6][r.bounded(4) as usize]
                    } else if r.bounded(3) > 0 {
                        (38 << 4) | [0, 3, 8][r.bounded(3) as usize]
                    } else {
                        37 << 4
                    }
                } else if matches!(self, Self::Swamp) {
                    (38 << 4) | 1
                } else if r.bounded(3) > 0 {
                    37 << 4
                } else {
                    38 << 4
                };
                Plant::Flower(state).generate(w, r, p);
            }
        }
        for _ in 0..grass {
            if let Some(p) = random_height(w, r, base, false) {
                let meta = if jungle_decorate && r.bounded(4) == 0
                    || taiga && !matches!(self, Self::TaigaM | Self::ColdTaigaM) && r.bounded(5) > 0
                {
                    2
                } else {
                    1
                };
                Plant::Grass(meta).generate(w, r, p);
            }
        }
        for _ in 0..bushes {
            if let Some(p) = random_height(w, r, base, false) {
                Plant::DeadBush.generate(w, r, p);
            }
        }
        {
            for _ in 0..lilies {
                if let Some(mut p) = random_height(w, r, base, false) {
                    while p[1] > 0 && w.state([p[0], p[1] - 1, p[2]]) == 0 {
                        p[1] -= 1;
                    }
                    Plant::Lily.generate(w, r, p);
                }
            }
        }
        for _ in 0..mushrooms {
            if r.bounded(4) == 0 {
                let [x, z] = column(r, base);
                Plant::Mushroom(false).generate(w, r, [x, w.height(x, z), z]);
            }
            if r.bounded(8) == 0
                && let Some(p) = random_height(w, r, base, false)
            {
                Plant::Mushroom(true).generate(w, r, p);
            }
        }
        for (chance, red) in [(4, false), (8, true)] {
            if r.bounded(chance) == 0
                && let Some(p) = random_height(w, r, base, false)
            {
                Plant::Mushroom(red).generate(w, r, p);
            }
        }
        for _ in 0..reeds + 10 {
            if let Some(p) = random_height(w, r, base, false) {
                Plant::Reed.generate(w, r, p);
            }
        }
        if r.bounded(32) == 0
            && let Some(p) = random_height(w, r, base, false)
        {
            Plant::Pumpkin.generate(w, r, p);
        }
        {
            for _ in 0..cacti {
                if let Some(p) = random_height(w, r, base, false) {
                    Plant::Cactus.generate(w, r, p);
                }
            }
        }
        springs(w, r, base);
        if jungle_decorate {
            let [x, z] = column(r, base);
            let y = r.bounded(w.height(x, z) * 2);
            for _ in 0..64 {
                let p = [
                    x + r.bounded(8) - r.bounded(8),
                    y + r.bounded(4) - r.bounded(4),
                    z + r.bounded(8) - r.bounded(8),
                ];
                if w.replaceable(p) && w.state([p[0], p[1] - 1, p[2]]) >> 4 == 2 {
                    w.set_state(p, 103 << 4);
                }
            }
            // WorldGenVines starts at Y=128. Its loop only runs below 128.
            for _ in 0..50 {
                column(r, base);
            }
        }
        if hills {
            let [x, z] = chunk.map(|n| n.wrapping_mul(16));
            for _ in 0..3 + r.bounded(6) {
                let p = [x + r.bounded(16), 4 + r.bounded(28), z + r.bounded(16)];
                if w.state(p) >> 4 == 1 {
                    w.set_state(p, 129 << 4);
                }
            }
            for _ in 0..7 {
                let p = [x + r.bounded(16), r.bounded(64), z + r.bounded(16)];
                crate::overworld::ores::vein(r, p, 9, 97 << 4, &mut |p, s| {
                    if w.state(p) >> 4 == 1 {
                        w.set_state(p, s);
                    }
                });
            }
        }
        if matches!(self, Self::Desert | Self::DesertHills) && r.bounded(1000) == 0 {
            let [x, z] = column(r, base);
            desert_well(w, [x, w.height(x, z) + 1, z]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::lakes::LakeWorld;
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Ground(BTreeMap<Pos, u16>, bool);
    impl LakeWorld for Ground {
        fn state(&self, p: Pos) -> u16 {
            self.0.get(&p).copied().unwrap_or(match p[1] {
                5.. => 0,
                4 if self.1 => 2 << 4,
                2..=3 if self.1 => 3 << 4,
                2..=4 => 12 << 4,
                _ => 1 << 4,
            })
        }
        fn set_state(&mut self, p: Pos, s: u16) {
            self.0.insert(p, s);
        }
        fn solid(&self, p: Pos) -> bool {
            !matches!(self.state(p) >> 4, 0 | 32)
        }
        fn liquid(&self, _: Pos) -> bool {
            false
        }
        fn skylight(&self, _: Pos) -> u8 {
            15
        }
        fn can_freeze(&self, _: Pos) -> bool {
            false
        }
        fn mycelium_surface(&self, _: Pos) -> bool {
            false
        }
    }
    impl PlantWorld for Ground {
        fn full_block(&self, p: Pos) -> bool {
            !matches!(
                self.state(p) >> 4,
                0 | 18 | 31 | 32 | 37..=40 | 81 | 83 | 106 | 111 | 175
            )
        }
        fn combined_light(&self, _: Pos) -> u8 {
            15
        }
    }
    impl DecorationWorld for Ground {
        fn top_solid_or_liquid(&self, x: i32, z: i32) -> i32 {
            self.height(x, z)
        }
    }
    impl BiomeWorld for Ground {
        fn height(&self, x: i32, z: i32) -> i32 {
            (4..256)
                .rev()
                .find(|&y| self.state([x, y, z]) != 0)
                .unwrap_or(3)
                + 1
        }
        fn tick_spring(&mut self, _: Pos, _: u16, _: &mut JavaRandom) {
            unreachable!("solid layers have no spring opening")
        }
    }
    #[test]
    fn ice_spikes_and_paths_across_negative_seams() {
        // MCP 1.8.9 WorldGenIceSpike / WorldGenIcePath on a snow-covered
        // dirt shelf. Hash includes every packed block state in all four columns.
        for (seed, hash, next) in [
            (12345, 0xca379acb86bec145, -6099763786001518257),
            (0, 0x011e731cafa184a5, -9189524968491074724),
            (95, 0x5576319cff36d9f5, -997865948986671191),
        ] {
            for reverse in [false, true] {
                let mut world = Ground::default();
                for i in 0..4 {
                    let n = if reverse { 3 - i } else { i };
                    let x0 = (n / 2 - 1) * 16;
                    let z0 = (n % 2 - 1) * 16;
                    for x in x0..x0 + 16 {
                        for z in z0..z0 + 16 {
                            for y in 50..=64 {
                                let id = match y {
                                    64 => 80,
                                    52.. => 3,
                                    _ => 1,
                                };
                                world.set_state([x, y, z], id << 4);
                            }
                        }
                    }
                }
                let mut random = JavaRandom::new(seed);
                super::super::ice::decorate(&mut world, &mut random, [-8, -8]);
                let mut h = 0xcbf29ce484222325u64;
                for x in -16..16 {
                    for z in -16..16 {
                        for y in 50..128 {
                            h = (h ^ u64::from(world.state([x, y, z]))).wrapping_mul(0x100000001b3);
                        }
                    }
                }
                assert_eq!((h, random.long()), (hash, next));
            }
        }
    }
    #[test]
    fn official_big_oak_reuses_its_height() {
        let mut height = 0;
        for expected in [0xe03e35bfc1593855, 0x69a321110355c195] {
            let mut w = Ground(BTreeMap::new(), true);
            let mut r = JavaRandom::new(12345);
            super::super::big_oak::generate(&mut w, &mut r, [-2, 5, -3], &mut height);
            let mut hash = 0xcbf29ce484222325u64;
            for x in -16..16 {
                for z in -16..16 {
                    for y in 0..256 {
                        hash = (hash ^ u64::from(w.state([x, y, z]))).wrapping_mul(0x100000001b3);
                    }
                }
            }
            assert_eq!(hash, expected);
            assert_eq!(r.long(), -1236052134575208584);
        }
    }
    #[test]
    fn official_biome_decorators() {
        use Biome::*;
        let mut generators = DecoratorState::default();
        for (biome, hash, next) in [
            (IcePlainsSpikes, 0xfe28533208b37be3, 5460916541143391814),
            (RoofedForest, 0xb9cff21d35a4eb51, 6112916508743067375),
            (RoofedForestM, 0xb9cff21d35a4eb51, 6112916508743067375),
            (Jungle, 0x0e7e8254e9cb5947, -5671044967139637135),
            (JungleHills, 0x0e7e8254e9cb5947, -5671044967139637135),
            (JungleEdge, 0xd700aafaaec964d6, 4452241728281497773),
            (JungleM, 0x0637aa743b1a7e83, 2337671690166262963),
            (JungleEdgeM, 0x1bf413e2f2c4531b, 3364964520848539756),
            (MegaTaiga, 0x6fc25bbb16bab7fe, 9121588737154090077),
            (MegaTaigaHills, 0x6fc25bbb16bab7fe, 9121588737154090077),
            (MegaSpruceTaiga, 0x93c30fb6fa75dd25, 9121588737154090077),
            (
                MegaSpruceTaigaHills,
                0x93c30fb6fa75dd25,
                9121588737154090077,
            ),
            (MushroomIsland, 0x2c502ac79c7b07fb, -7539978330448666849),
            (MushroomIsland, 0xf05cf630a68da71f, -5946840496078021208),
            (MushroomShore, 0x2c502ac79c7b07fb, -7539978330448666849),
            (MushroomShore, 0xf05cf630a68da71f, -5946840496078021208),
            (ExtremeHills, 0x86146e202e5ec7c4, 9158717183010001236),
            (ExtremeHillsEdge, 0x2d2f08cb9595eee5, 1991537931463437436),
            (ExtremeHillsPlus, 0x2d2f08cb9595eee5, 1991537931463437436),
            (Savanna, 0x9f36a68e34dd7708, 3454130638914611354),
            (SavannaPlateau, 0x9f36a68e34dd7708, 3454130638914611354),
            (Mesa, 0x670105182165bfe0, 7282476345425584697),
            (MesaPlateauF, 0xc223e7e49726fc83, 8475289679656435228),
            (MesaPlateau, 0x670105182165bfe0, 7282476345425584697),
            (ExtremeHillsM, 0x86146e202e5ec7c4, 9158717183010001236),
            (ExtremeHillsPlusM, 0x86146e202e5ec7c4, 9158717183010001236),
            (SavannaM, 0x96a6ec5b8e50f92f, -8521376896336515338),
            (SavannaPlateauM, 0x96a6ec5b8e50f92f, -8521376896336515338),
            (MesaBryce, 0x670105182165bfe0, 7282476345425584697),
            (MesaPlateauFM, 0xc223e7e49726fc83, 8475289679656435228),
            (MesaPlateauM, 0x670105182165bfe0, 7282476345425584697),
            (Plains, 0xffd2c92c61d17a23, 3777715662547530666),
            (SunflowerPlains, 0x0f1397415ee3e1ed, 2899601499277817866),
            (FlowerForest, 0xe2aaeb5957b2ed24, -1072273557199588180),
            (Ocean, 0x86146e202e5ec7c4, -1245352112429479037),
            (River, 0x86146e202e5ec7c4, -1245352112429479037),
            (FrozenOcean, 0x86146e202e5ec7c4, -1245352112429479037),
            (FrozenRiver, 0x86146e202e5ec7c4, -1245352112429479037),
            (DeepOcean, 0x86146e202e5ec7c4, -1245352112429479037),
            (Taiga, 0x4d38f8ca796229a7, 7480494199529097750),
            (TaigaHills, 0x4d38f8ca796229a7, 7480494199529097750),
            (ColdTaiga, 0x4d38f8ca796229a7, 7480494199529097750),
            (ColdTaigaHills, 0x4d38f8ca796229a7, 7480494199529097750),
            (TaigaM, 0x0e529ffcaeda2598, -4453391631534951751),
            (ColdTaigaM, 0x0e529ffcaeda2598, -4453391631534951751),
            (Swamp, 0x135d0103d88ad934, 1285199822983936846),
            (SwampM, 0x7cfb194d0e9376f7, 3387411241529302714),
            (IcePlains, 0x86146e202e5ec7c4, -1245352112429479037),
            (IceMountains, 0x86146e202e5ec7c4, -1245352112429479037),
            (Forest, 0xd4dafdf0c60f2731, 2827415228422624813),
            (ForestHills, 0xd4dafdf0c60f2731, 2827415228422624813),
            (BirchForest, 0x47af77fe2e1119f2, -2705644398092876636),
            (BirchForestHills, 0x47af77fe2e1119f2, -2705644398092876636),
            (BirchForestM, 0x417d9d5ae69c5b1c, -7231020933443655901),
            (BirchForestHillsM, 0x417d9d5ae69c5b1c, -7231020933443655901),
            (Desert, 0xd458900a71a22f8c, -5563023718104558844),
            (DesertHills, 0xd458900a71a22f8c, -5563023718104558844),
            (DesertM, 0xd458900a71a22f8c, 1023561215819656039),
            (Beach, 0x07ef4a061a9663cc, -1245352112429479037),
            (ColdBeach, 0x07ef4a061a9663cc, -1245352112429479037),
            (StoneBeach, 0x07ef4a061a9663cc, -1245352112429479037),
        ] {
            let mut w = Ground(
                BTreeMap::new(),
                !matches!(
                    biome,
                    Desert | DesertHills | DesertM | Beach | StoneBeach | ColdBeach
                ),
            );
            let mut r = JavaRandom::new(12345);
            biome.decorate(&mut generators, &mut w, &mut r, [-1, -1]);
            let mut h = 0xcbf29ce484222325u64;
            for x in -16..16 {
                for z in -16..16 {
                    for y in 0..256 {
                        h = (h ^ u64::from(w.state([x, y, z]))).wrapping_mul(0x100000001b3);
                    }
                }
            }
            assert_eq!(h, hash, "{biome:?}");
            assert_eq!(r.long(), next);
        }
    }
}
