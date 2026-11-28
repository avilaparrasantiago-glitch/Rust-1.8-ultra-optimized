use crate::arena::{BlockPos, World};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

const LIMIT: usize = 512;
const OFFSETS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];
const HORIZONTAL: [(i32, i32, i32); 4] = [(0, 0, -1), (1, 0, 0), (0, 0, 1), (-1, 0, 0)];
fn at(p: BlockPos, d: (i32, i32, i32)) -> Option<BlockPos> {
    Some((
        p.0 + d.0,
        u8::try_from(i32::from(p.1) + d.1).ok()?,
        p.2 + d.2,
    ))
}
fn reverse((x, y, z): (i32, i32, i32)) -> (i32, i32, i32) {
    (-x, -y, -z)
}
fn output(w: &World, from: BlockPos, to: BlockPos) -> u16 {
    let state = w.block(from);
    let meta = state & 15;
    match state >> 4 {
        152 | 76 => 15,
        69 | 77 | 143 if meta & 8 != 0 => 15,
        70 | 72 if meta != 0 => 15,
        55 => meta.saturating_sub(1),
        94 if at(from, HORIZONTAL[(meta & 3) as usize]) == Some(to) => 15,
        _ => 0,
    }
}
#[derive(Default)]
pub struct Circuit {
    queue: VecDeque<BlockPos>,
    queued: HashSet<BlockPos>,
    due: BTreeMap<u64, Vec<BlockPos>>,
    pending: HashMap<BlockPos, (u64, u16, u16)>,
    sleeping: Vec<(u64, BlockPos)>,
}
impl Circuit {
    pub fn changed(&mut self, p: BlockPos) {
        self.enqueue(p);
        for dir in OFFSETS {
            if let Some(n) = at(p, dir) {
                self.enqueue(n);
            }
        }
    }
    fn enqueue(&mut self, p: BlockPos) {
        if self.queued.insert(p) {
            self.queue.push_back(p);
        }
    }
    pub fn step_active(
        &mut self,
        w: &mut World,
        tick: u64,
        out: &mut Vec<(BlockPos, u16)>,
        active: impl Fn(BlockPos) -> bool,
    ) {
        out.clear();
        self.sleeping.retain(|&(time, p)| {
            if active(p) {
                self.due.entry(time).or_default().push(p);
                false
            } else {
                true
            }
        });
        let mut budget = LIMIT;
        while budget > 0 && self.due.first_key_value().is_some_and(|(&t, _)| t <= tick) {
            let Some(mut e) = self.due.first_entry() else {
                break;
            };
            let time = *e.key();
            let Some(p) = e.get_mut().pop() else {
                e.remove();
                continue;
            };
            if e.get().is_empty() {
                e.remove();
            }
            if let Some(&(t, before, after)) = self.pending.get(&p)
                && t == time
            {
                if !active(p) {
                    self.sleeping.push((time, p));
                    budget -= 1;
                    continue;
                }
                self.pending.remove(&p);
                if w.block(p) == before {
                    self.write(w, p, after, out);
                }
            }
            budget -= 1;
        }
        // Deduplication and a bounded frontier keep feedback loops iterative.
        let visits = budget.min(self.queue.len());
        for _ in 0..visits {
            let Some(p) = self.queue.pop_front() else {
                break;
            };
            if !active(p) {
                self.queue.push_back(p);
                continue;
            }
            self.queued.remove(&p);
            let s = w.block(p);
            let id = s >> 4;
            let meta = s & 15;
            if !matches!(id, 55 | 93 | 94 | 64 | 71 | 96 | 167 | 29 | 33) {
                continue;
            }
            let strength = OFFSETS
                .iter()
                .filter_map(|&d| at(p, d))
                .map(|n| output(w, n, p))
                .max()
                .unwrap_or(0);
            match id {
                55 if meta != strength => self.write(w, p, (55 << 4) | strength, out),
                93 | 94 => {
                    let dir = HORIZONTAL[(meta & 3) as usize];
                    let powered = at(p, reverse(dir)).is_some_and(|q| output(w, q, p) > 0);
                    let target = (if powered { 94 } else { 93 }) << 4 | meta;
                    if s == target {
                        self.pending.remove(&p);
                    } else if !self
                        .pending
                        .get(&p)
                        .is_some_and(|&(_, b, a)| b == s && a == target)
                    {
                        let due = tick + 2 * (1 + u64::from(meta >> 2));
                        self.pending.insert(p, (due, s, target));
                        self.due.entry(due).or_default().push(p);
                    }
                }
                64 | 71 if meta & 8 == 0 => {
                    let target = (s & !4) | (u16::from(strength != 0) * 4);
                    if s != target {
                        self.write(w, p, target, out);
                    }
                }
                96 | 167 => {
                    let target = (s & !4) | (u16::from(strength != 0) * 4);
                    if s != target {
                        self.write(w, p, target, out);
                    }
                }
                29 | 33 if (strength != 0) != (meta & 8 != 0) => {
                    self.piston(w, p, s, strength != 0, out)
                }
                _ => {}
            }
        }
    }
    fn write(&mut self, w: &mut World, p: BlockPos, s: u16, out: &mut Vec<(BlockPos, u16)>) {
        if w.set(p, s).is_ok() {
            out.push((p, s));
            self.changed(p);
        }
    }
    fn piston(
        &mut self,
        w: &mut World,
        p: BlockPos,
        s: u16,
        extend: bool,
        out: &mut Vec<(BlockPos, u16)>,
    ) {
        let Some(&dir) = OFFSETS.get((s & 7) as usize) else {
            return;
        };
        let Some(head) = at(p, dir) else { return };
        if !w.contains((head.0 >> 4, head.2 >> 4)) {
            return;
        }
        if !extend {
            if w.block(head) >> 4 == 34 {
                self.write(w, head, 0, out);
                if s >> 4 == 29
                    && let Some(src) = at(head, dir)
                {
                    let v = w.block(src);
                    if movable(v) {
                        self.write(w, head, v, out);
                        self.write(w, src, 0, out);
                    }
                }
            }
            self.write(w, p, s & !8, out);
            return;
        }
        let mut scan = head;
        let mut moves = [((0, 0, 0), 0); 12];
        let mut n = 0;
        loop {
            if !w.contains((scan.0 >> 4, scan.2 >> 4)) {
                return;
            }
            let v = w.block(scan);
            if v == 0 {
                break;
            }
            if !movable(v) || n == 12 {
                return;
            }
            moves[n] = (scan, v);
            n += 1;
            let Some(next) = at(scan, dir) else { return };
            scan = next;
        }
        for &(src, v) in moves[..n].iter().rev() {
            self.write(w, at(src, dir).unwrap(), v, out);
        }
        self.write(
            w,
            head,
            (34 << 4) | (s & 7) | if s >> 4 == 29 { 8 } else { 0 },
            out,
        );
        self.write(w, p, s | 8, out);
    }
}
fn movable(s: u16) -> bool {
    s != 0
        && matches!(s>>4,1..=5|12..=17|20..=25|35|41..=43|45..=48|56..=58|80|82|87..=89|98|112|121|133|152|155|159)
}
