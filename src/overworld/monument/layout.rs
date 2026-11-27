use super::*;
#[derive(Clone, Copy)]
pub(super) struct Room {
    pub neighbor: [usize; 6],
    pub open: [bool; 6],
    claimed: bool,
}
const EMPTY: Room = Room {
    neighbor: [NONE; 6],
    open: [false; 6],
    claimed: false,
};
pub(super) struct Layout {
    pub rooms: [Room; 78],
    pub pieces: Vec<(usize, Kind, i32)>,
    pub wing: i32,
}
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Entry,
    Core,
    DoubleXY,
    DoubleYZ,
    DoubleZ,
    DoubleX,
    DoubleY,
    Sponge,
    Simple,
}
impl Kind {
    pub fn size(self) -> [i32; 3] {
        match self {
            Self::Core => [2, 2, 2],
            Self::DoubleXY => [2, 2, 1],
            Self::DoubleYZ => [1, 2, 2],
            Self::DoubleX => [2, 1, 1],
            Self::DoubleY => [1, 2, 1],
            Self::DoubleZ => [1, 1, 2],
            _ => [1, 1, 1],
        }
    }
}
fn reaches_entry(rooms: &[Room; 78], from: usize, seen: &mut [bool; 78]) -> bool {
    if from == 2 {
        return true;
    }
    seen[from] = true;
    rooms[from].open.iter().enumerate().any(|(d, &open)| {
        let next = rooms[from].neighbor[d];
        open && !seen[next] && reaches_entry(rooms, next, seen)
    })
}
impl Layout {
    pub fn generate(r: &mut JavaRandom) -> Self {
        let mut rooms = [EMPTY; 78];
        let mut order: Vec<usize> = Vec::new();
        for y in 0..3 {
            for z in 0..4 {
                for x in 0..5 {
                    if y == 2 && (z >= 2 || x == 0 || x == 4) {
                        continue;
                    }
                    order.push(y * 25 + z * 5 + x);
                }
            }
        }
        for &id in &order {
            for (d, valid, n) in [
                (0, id >= 25, id.wrapping_sub(25)),
                (1, id < 50, id + 25),
                (2, id / 5 % 5 < 4, id + 5),
                (3, id / 5 % 5 > 0, id.wrapping_sub(5)),
                (4, id % 5 > 0, id.wrapping_sub(1)),
                (5, id % 5 < 4, id + 1),
            ] {
                if valid && order.contains(&n) {
                    rooms[id].neighbor[d] = n;
                }
            }
        }
        for (id, d, special) in [(52, 1, 75), (25, 3, 76), (29, 3, 77)] {
            rooms[id].neighbor[d] = special;
            rooms[special].neighbor[d ^ 1] = id;
            rooms[special].claimed = true;
        }
        for &id in order.iter().chain([75].iter()) {
            rooms[id].open = rooms[id].neighbor.map(|v| v != NONE);
        }
        let core = 10 + r.bounded(4) as usize;
        for offset in [0, 1, 5, 6, 25, 26, 30, 31] {
            rooms[core + offset].claimed = true;
        }
        for n in (2..=order.len()).rev() {
            order.swap(n - 1, r.bounded(n as i32) as usize);
        }
        for &id in &order {
            let mut closed = 0;
            for _ in 0..5 {
                if closed == 2 {
                    break;
                }
                let dir = r.bounded(6) as usize;
                if !rooms[id].open[dir] {
                    continue;
                }
                let next = rooms[id].neighbor[dir];
                rooms[id].open[dir] = false;
                rooms[next].open[dir ^ 1] = false;
                if reaches_entry(&rooms, id, &mut [false; 78])
                    && reaches_entry(&rooms, next, &mut [false; 78])
                {
                    closed += 1;
                } else {
                    rooms[id].open[dir] = true;
                    rooms[next].open[dir ^ 1] = true;
                }
            }
        }
        rooms[2].claimed = true;
        let mut pieces = vec![(2, Kind::Entry, 0), (core, Kind::Core, 0)];
        for id in order {
            if rooms[id].claimed {
                continue;
            }
            let free =
                |i: usize, d: usize| rooms[i].open[d] && !rooms[rooms[i].neighbor[d]].claimed;
            let kind = if free(id, 5) && free(id, 1) && free(rooms[id].neighbor[5], 1) {
                Kind::DoubleXY
            } else if free(id, 2) && free(id, 1) && free(rooms[id].neighbor[2], 1) {
                Kind::DoubleYZ
            } else if free(id, 2) {
                Kind::DoubleZ
            } else if free(id, 5) {
                Kind::DoubleX
            } else if free(id, 1) {
                Kind::DoubleY
            } else if !rooms[id].open[1..].iter().any(|&v| v) {
                Kind::Sponge
            } else {
                Kind::Simple
            };
            let [w, h, d] = kind.size();
            for y in 0..h {
                for z in 0..d {
                    for x in 0..w {
                        rooms[id + (x + 5 * z + 25 * y) as usize].claimed = true;
                    }
                }
            }
            let variant = if matches!(kind, Kind::Simple) {
                r.bounded(3)
            } else {
                0
            };
            pieces.push((id, kind, variant));
        }
        Self {
            rooms,
            pieces,
            wing: (r.bits(32) & 1) as i32,
        }
    }
}
