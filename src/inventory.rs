use crate::packets::Stack;

#[derive(Clone, Default)]
struct Drag {
    button: u8,
    slots: u64,
}

#[derive(Clone)]
pub struct Inventory {
    pub slots: [Option<Stack>; 45],
    pub cursor: Option<Stack>,
    pub held: u8,
    drag: Option<Drag>,
}
impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: [None; 45],
            cursor: None,
            held: 0,
            drag: None,
        }
    }
}
fn matches(a: Stack, b: Stack) -> bool {
    a.id == b.id && a.damage == b.damage
}
fn limit(id: i16) -> u8 {
    match id {
        256..=259
        | 261
        | 267..=279
        | 282..=286
        | 290..=294
        | 298..=317
        | 325..=327
        | 328
        | 329
        | 333
        | 335
        | 346
        | 359
        | 373
        | 386
        | 387 => 1,
        323 | 332 | 344 | 368 => 16,
        _ => 64,
    }
}
pub(crate) fn transfer(src: &mut Option<Stack>, dst: &mut Option<Stack>, max: u8) -> u8 {
    let Some(mut a) = *src else { return 0 };
    let mut b = dst.unwrap_or(Stack { count: 0, ..a });
    if !matches(a, b) {
        return 0;
    }
    let n = a.count.min(limit(a.id).saturating_sub(b.count)).min(max);
    if n > 0 {
        a.count -= n;
        b.count += n;
        *dst = Some(b);
        *src = (a.count != 0).then_some(a);
    }
    n
}
impl Inventory {
    pub(crate) fn collect(
        cursor: &mut Option<Stack>,
        inputs: &mut [Option<Stack>],
        storage: &mut [Option<Stack>],
    ) {
        let Some(item) = *cursor else { return };
        let cap = limit(item.id);
        for full in [false, true] {
            for slot in inputs.iter_mut().chain(storage.iter_mut()) {
                if cursor.is_some_and(|s| s.count >= cap) {
                    return;
                }
                if slot.is_some_and(|s| (s.count == cap) == full) {
                    transfer(slot, cursor, cap);
                }
            }
        }
    }
    pub fn add(&mut self, item: Stack) -> bool {
        let mut copy = self.slots;
        let mut source = Some(item);
        for empty in [false, true] {
            for dst in &mut copy[9..] {
                if dst.is_none() == empty {
                    transfer(&mut source, dst, 64);
                }
                if source.is_none() {
                    self.slots = copy;
                    return true;
                }
            }
        }
        false
    }
    pub fn click(&mut self, idx: i16, button: u8, mode: u8) -> bool {
        if mode == 5 {
            return self.drag(idx, button);
        }
        self.drag = None;
        if mode == 6 {
            if button != 0 || !(0..45).contains(&idx) {
                return false;
            }
            if self.slots[idx as usize].is_none() {
                let (equipment, storage) = self.slots.split_at_mut(9);
                Self::collect(&mut self.cursor, &mut equipment[1..], storage);
            }
            return true;
        }
        if idx == 0 {
            if !matches!(mode, 0 | 1) || button > 1 {
                return false;
            }
            while let Some(item) = crate::crafting::craft(&self.slots[1..5], 2) {
                if !self.take_result(item, mode == 1) {
                    break;
                }
                for s in &mut self.slots[1..5] {
                    crate::crafting::consume(s);
                }
                if mode == 0 {
                    break;
                }
            }
            self.slots[0] = crate::crafting::craft(&self.slots[1..5], 2);
            return true;
        }
        if !(1..=4).contains(&idx) && !(9..45).contains(&idx) {
            return false;
        }
        let i = idx as usize;
        match (mode, button) {
            (0, 0 | 1) => {
                click_one(&mut self.cursor, &mut self.slots[i], button);
                true
            }
            (1, 0 | 1) => {
                let range = if i < 36 { 36..45 } else { 9..36 };
                let mut source = self.slots[i].take();
                for empty in [false, true] {
                    for j in range.clone() {
                        if self.slots[j].is_none() == empty {
                            transfer(&mut source, &mut self.slots[j], 64);
                        }
                    }
                }
                self.slots[i] = source;
                true
            }
            (2, 0..=8) => {
                self.slots.swap(i, 36 + usize::from(button));
                true
            }
            _ => false,
        }
    }
    fn drag(&mut self, idx: i16, button: u8) -> bool {
        let kind = button / 4;
        let stage = button % 4;
        if kind > 1 || self.cursor.is_none() {
            self.drag = None;
            return false;
        }
        if stage == 0 && idx == -999 {
            self.drag = Some(Drag {
                button: kind,
                slots: 0,
            });
            return true;
        }
        let Some(mut drag) = self.drag.take() else {
            return false;
        };
        if drag.button != kind {
            return false;
        }
        if stage == 1 && (9..45).contains(&idx) {
            if self.slots[idx as usize]
                .is_none_or(|s| matches(s, self.cursor.unwrap()) && s.count < limit(s.id))
            {
                drag.slots |= 1u64 << idx;
            }
            self.drag = Some(drag);
            return true;
        }
        if stage != 2 || idx != -999 || drag.slots == 0 {
            return false;
        }
        let n = if kind == 0 {
            self.cursor.unwrap().count / drag.slots.count_ones() as u8
        } else {
            1
        };
        for i in 9..45 {
            if drag.slots & (1 << i) != 0 {
                transfer(&mut self.cursor, &mut self.slots[i], n);
            }
        }
        true
    }
}

pub(crate) fn click_one(cursor: &mut Option<Stack>, slot: &mut Option<Stack>, button: u8) {
    if cursor.is_none() {
        let n = if button == 0 {
            64
        } else {
            slot.map_or(0, |v| v.count.div_ceil(2))
        };
        transfer(slot, cursor, n);
    } else if let (Some(a), Some(b)) = (*cursor, *slot)
        && !matches(a, b)
    {
        std::mem::swap(cursor, slot);
    } else {
        transfer(cursor, slot, if button == 0 { 64 } else { 1 });
    }
}
impl Inventory {
    pub(crate) fn take_result(&mut self, item: Stack, shift: bool) -> bool {
        if shift {
            return self.add(item);
        }
        let mut src = Some(item);
        let mut cursor = self.cursor;
        transfer(&mut src, &mut cursor, 64);
        if src.is_some() {
            return false;
        }
        self.cursor = cursor;
        true
    }
}
