use super::{JavaRandom, Overworld, StructureGrid, dungeons::DungeonWorld};
use crate::anvil::structures::StructureFile;
use fastnbt::Value;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io};
mod layout;
mod rooms;
use layout::{Kind, Layout, Room};
const NONE: usize = usize::MAX;
const ROUGH: u16 = 168 << 4;
const BRICK: u16 = ROUGH + 1;
const DARK: u16 = ROUGH + 2;
const LANTERN: u16 = 169 << 4;
const WATER: u16 = 9 << 4;
const GOLD: u16 = 41 << 4;
const SPONGE: u16 = (19 << 4) + 1;
type Pos = [i32; 3];
#[derive(Clone, Copy)]
enum Mode {
    Flood,
    WaterOnly,
    Solid,
}
struct Placement<'a, W> {
    world: &'a mut W,
    rng: &'a mut JavaRandom,
    graph: &'a [Room; 78],
    elders: &'a mut Vec<Pos>,
    origin: Pos,
    dir: i32,
    clip: [i32; 4],
    room: usize,
    variant: i32,
}
impl<W: DungeonWorld> Placement<'_, W> {
    fn transform(&self, [x, y, z]: Pos) -> Pos {
        let [ox, oy, oz] = self.origin;
        let (dx, dz) = match self.dir {
            0 => (x, z),
            1 => (-z, x),
            2 => (x, -z),
            _ => (z, x),
        };
        [ox + dx, oy + y, oz + dz]
    }
    fn inside(&self, [x, _, z]: Pos) -> bool {
        x >= self.clip[0] && x <= self.clip[2] && z >= self.clip[1] && z <= self.clip[3]
    }
    fn intersects(&self, [x, z, xx, zz]: [i32; 4]) -> bool {
        let [a, _, b] = self.transform([x, 0, z]);
        let [c, _, d] = self.transform([xx, 0, zz]);
        a.min(c) <= self.clip[2]
            && a.max(c) >= self.clip[0]
            && b.min(d) <= self.clip[3]
            && b.max(d) >= self.clip[1]
    }
    fn elder(&mut self, p: Pos) {
        let p = self.transform(p);
        if self.inside(p) {
            self.elders.push(p);
        }
    }
    fn block(&mut self, p: Pos, state: u16) {
        let p = self.transform(p);
        if self.inside(p) {
            self.world.set_state(p, state);
        }
    }
    fn support(&mut self, p: Pos, state: u16) {
        let [x, y, z] = self.transform(p);
        if !self.inside([x, y, z]) {
            return;
        }
        for yy in (2..=y).rev() {
            if !matches!(self.world.state([x, yy, z]) >> 4, 0 | 8..=11) {
                break;
            }
            self.world.set_state([x, yy, z], state);
        }
    }
    fn floor(&mut self, x: i32, z: i32, open: bool) {
        if !open {
            self.fill([x, 0, z], [x + 7, 0, z + 7], ROUGH, Mode::Solid);
            return;
        }
        for (a, b, c, d, state) in [
            (0, 0, 2, 7, ROUGH),
            (5, 0, 7, 7, ROUGH),
            (3, 0, 4, 2, ROUGH),
            (3, 5, 4, 7, ROUGH),
            (3, 2, 4, 2, BRICK),
            (3, 5, 4, 5, BRICK),
            (2, 3, 2, 4, BRICK),
            (5, 3, 5, 4, BRICK),
        ] {
            self.fill([x + a, 0, z + b], [x + c, 0, z + d], state, Mode::Solid);
        }
    }
    fn fill(&mut self, lo: Pos, hi: Pos, state: u16, mode: Mode) {
        let [a, y0, b] = self.transform(lo);
        let [c, y1, d] = self.transform(hi);
        for y in y0..=y1 {
            for x in a.min(c).max(self.clip[0])..=a.max(c).min(self.clip[2]) {
                for z in b.min(d).max(self.clip[1])..=b.max(d).min(self.clip[3]) {
                    let p = [x, y, z];
                    let state = match mode {
                        Mode::Flood if y >= 63 => 0,
                        Mode::WaterOnly if self.world.state(p) != WATER => continue,
                        _ => state,
                    };
                    self.world.set_state(p, state);
                }
            }
        }
    }
}
#[derive(Default, Serialize, Deserialize)]
pub(super) struct MonumentChanges {
    pub starts: Vec<Vec<u8>>,
    pub elders: Vec<Pos>,
}
impl MonumentChanges {
    pub fn apply(&self, file: &mut StructureFile) -> io::Result<()> {
        for bytes in &self.starts {
            file.insert(fastnbt::from_bytes(bytes).map_err(io::Error::other)?)?;
        }
        Ok(())
    }
}
impl Overworld {
    pub(super) fn monument_pieces(
        &self,
        file: &StructureFile,
        (cx, cz): (i32, i32),
        world: &mut impl DungeonWorld,
        rng: &mut JavaRandom,
    ) -> io::Result<MonumentChanges> {
        let mut changes = MonumentChanges::default();
        let clip = [cx * 16 + 8, cz * 16 + 8, cx * 16 + 23, cz * 16 + 23];
        for rx in (cx - 3).div_euclid(32)..=(cx + 3).div_euclid(32) {
            for rz in (cz - 3).div_euclid(32)..=(cz + 3).div_euclid(32) {
                let (x, z) = StructureGrid::OceanMonument.candidate(self.seed, rx * 32, rz * 32);
                let Some(start) = file.get(x, z) else {
                    continue;
                };
                let (ox, oz) = (x * 16 - 21, z * 16 - 21);
                if ox > clip[2] || ox + 57 < clip[0] || oz > clip[3] || oz + 57 < clip[1] {
                    continue;
                }
                let processed = Value::Compound(HashMap::from([
                    ("X".into(), Value::Int(cx)),
                    ("Z".into(), Value::Int(cz)),
                ]));
                let mut start = start.clone();
                let Some(Value::List(done)) = start.get_mut("Processed") else {
                    return Err(io::Error::other("missing monument Processed"));
                };
                if done.contains(&processed) {
                    continue;
                }
                done.push(processed);
                let mut seed = JavaRandom::new(self.seed);
                let mut init = JavaRandom::new(
                    self.seed
                        ^ seed.long().wrapping_mul(x.into())
                        ^ seed.long().wrapping_mul(z.into()),
                );
                let dir = (init.bounded(4) + 2) % 4;
                let layout = Layout::generate(&mut init);
                let origin = [
                    ox + if dir == 1 { 57 } else { 0 },
                    39,
                    oz + if dir == 2 { 57 } else { 0 },
                ];
                let mut p = Placement {
                    world,
                    rng,
                    graph: &layout.rooms,
                    elders: &mut changes.elders,
                    origin,
                    dir,
                    clip,
                    room: 0,
                    variant: 0,
                };
                p.building();
                for (id, kind, variant) in layout.pieces {
                    p.origin = origin;
                    let [w, _, d] = kind.size();
                    let local = [
                        9 + (id % 5) as i32 * 8,
                        (id / 25) as i32 * 4,
                        22 + (id / 5 % 5) as i32 * 8,
                    ];
                    p.origin = p.transform(local);
                    if !p.intersects([0, 0, w * 8 - 1, d * 8 - 1]) {
                        continue;
                    }
                    p.room = id;
                    p.variant = variant;

                    match kind {
                        Kind::Entry => p.entry(),
                        Kind::Core => p.core(),
                        Kind::DoubleXY => p.double_xy(),
                        Kind::DoubleYZ => p.double_yz(),
                        Kind::DoubleZ => p.double_z(),
                        Kind::DoubleX => p.double_x(),
                        Kind::DoubleY => p.double_y(),
                        Kind::Sponge => p.sponge(),
                        Kind::Simple => p.simple(),
                    }
                }
                for (offset, width, depth, variant) in [
                    ([1, 1, 1], 23, 21, layout.wing),
                    ([34, 1, 1], 23, 21, layout.wing ^ 1),
                    ([22, 13, 22], 14, 14, 2),
                ] {
                    p.origin = origin;
                    p.origin = p.transform(offset);
                    if !p.intersects([0, 0, width - 1, depth - 1]) {
                        continue;
                    }
                    p.variant = variant;
                    if variant == 2 {
                        p.penthouse()
                    } else {
                        p.wing()
                    }
                }
                changes
                    .starts
                    .push(fastnbt::to_bytes(&start).map_err(io::Error::other)?);
            }
        }
        Ok(changes)
    }
}
