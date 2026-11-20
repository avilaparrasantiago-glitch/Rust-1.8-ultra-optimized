use super::{Player, Room, Window, p, reachable};
use crate::{
    arena::BlockPos,
    crafting::{self, Furnace},
    inventory::{click_one, transfer},
    packets::Stack,
    wire::{Packet, WriteWire},
};
use bytes::BufMut;

impl Room {
    pub(super) fn open_window(&mut self, i: usize, pos: BlockPos, id: u16) {
        let p = &mut self.players[i];
        let (window, kind, name, count) = if id == 58 {
            (
                Window::Table(pos),
                "minecraft:crafting_table",
                "Crafting",
                0,
            )
        } else {
            self.furnaces.entry(pos).or_default();
            (Window::Furnace(pos), "minecraft:furnace", "Furnace", 3)
        };
        p.window = Some(window);
        p.send(Packet::new(0x2d, |w| {
            w.put_u8(1);
            w.string(kind);
            w.string(&serde_json::json!({"text":name}).to_string());
            w.put_u8(count);
        }));
        send_contents(p, window, self.furnaces.get(&pos));
    }
    pub(super) fn window_click(
        &mut self,
        i: usize,
        window: u8,
        idx: i16,
        button: u8,
        seq: i16,
        mode: u8,
    ) {
        let active = self.players[i].window;
        let player = &mut self.players[i];
        let before = player.inv.slots;
        let mut refresh = None;
        let ok = match (active, window) {
            (None, 0) => {
                let ok = player.inv.click(idx, button, mode);
                player.inv.slots[0] = crafting::craft(&player.inv.slots[1..5], 2);
                ok
            }
            (Some(w), 1) => {
                let (pos, offset) = match w {
                    Window::Table(p) => (p, 10),
                    Window::Furnace(p) => (p, 3),
                };
                if !reachable(player, pos)
                    || !matches!(
                        (w, self.world.block(pos) >> 4),
                        (Window::Table(_), 58) | (Window::Furnace(_), 61 | 62)
                    )
                {
                    player.send(Packet::new(0x2e, |w| w.put_u8(1)));
                    player.window = None;
                    return;
                }
                refresh = Some(pos);
                if mode == 6 {
                    if button != 0 || !(0..offset + 36).contains(&idx) {
                        false
                    } else {
                        let (inputs, first): (&mut [Option<Stack>], i16) = match w {
                            Window::Table(_) => (&mut player.table, 1),
                            Window::Furnace(_) => {
                                (&mut self.furnaces.entry(pos).or_default().slots, 0)
                            }
                        };
                        let storage = &mut player.inv.slots[9..];
                        let empty = if idx >= offset {
                            storage[(idx - offset) as usize].is_none()
                        } else if idx >= first {
                            inputs[(idx - first) as usize].is_none()
                        } else {
                            crafting::craft(inputs, 3).is_none()
                        };
                        if empty {
                            crate::inventory::Inventory::collect(
                                &mut player.inv.cursor,
                                inputs,
                                storage,
                            );
                        }
                        true
                    }
                } else if (offset..offset + 36).contains(&idx) {
                    let source = (9 + idx - offset) as usize;
                    if mode == 1 && button <= 1 {
                        match w {
                            Window::Table(_) => player.inv.click(source as i16, button, 1),
                            Window::Furnace(_) => {
                                let machine = self.furnaces.entry(pos).or_default();
                                let slot = usize::from(
                                    player.inv.slots[source]
                                        .is_some_and(|s| matches!(s.id, 263 | 5 | 17 | 280)),
                                );
                                transfer(
                                    &mut player.inv.slots[source],
                                    &mut machine.slots[slot],
                                    64,
                                );
                                true
                            }
                        }
                    } else {
                        player.inv.click(source as i16, button, mode)
                    }
                } else {
                    match w {
                        Window::Table(_) if idx == 0 && mode <= 1 && button <= 1 => {
                            while let Some(result) = crafting::craft(&player.table, 3) {
                                if !player.inv.take_result(result, mode == 1) {
                                    break;
                                }
                                for s in &mut player.table {
                                    crafting::consume(s);
                                }
                                if mode == 0 {
                                    break;
                                }
                            }
                            true
                        }
                        Window::Table(_) if (1..=9).contains(&idx) => container_click(
                            &mut player.inv,
                            &mut player.table[idx as usize - 1],
                            mode,
                            button,
                            false,
                        ),
                        Window::Furnace(_) if (0..=2).contains(&idx) => container_click(
                            &mut player.inv,
                            &mut self.furnaces.entry(pos).or_default().slots[idx as usize],
                            mode,
                            button,
                            idx == 2,
                        ),
                        _ => false,
                    }
                }
            }
            _ => false,
        };
        player.send(Packet::new(0x32, |w| {
            w.put_u8(window);
            w.put_i16(seq);
            w.put_u8(ok as u8);
        }));
        if let Some(pos) = refresh
            && let Some(w) = active
        {
            send_contents(player, w, self.furnaces.get(&pos));
            if matches!(w, Window::Furnace(_))
                && let Some(f) = self.furnaces.get(&pos)
            {
                for (j, peer) in self.players.iter().enumerate() {
                    if j != i && peer.window == Some(w) {
                        for (slot, item) in f.slots.iter().enumerate() {
                            peer.send(p::slot(1, slot as i16, *item));
                        }
                    }
                }
            }
        } else {
            for (j, item) in player.inv.slots.iter().enumerate() {
                if !ok || before[j] != *item {
                    player.send(p::slot(0, j as i16, *item));
                }
            }
        }
        let player = &self.players[i];
        player.send(p::slot(-1, -1, player.inv.cursor));
    }
}
fn send_contents(p: &Player, w: Window, f: Option<&Furnace>) {
    let offset = match w {
        Window::Table(_) => {
            p.send(p::slot(1, 0, crafting::craft(&p.table, 3)));
            for (j, item) in p.table.iter().enumerate() {
                p.send(p::slot(1, 1 + j as i16, *item));
            }
            10
        }
        Window::Furnace(_) => {
            if let Some(f) = f {
                for (j, item) in f.slots.iter().enumerate() {
                    p.send(p::slot(1, j as i16, *item));
                }
            }
            3
        }
    };
    for (j, item) in p.inv.slots[9..].iter().enumerate() {
        p.send(p::slot(1, offset + j as i16, *item));
    }
}
fn container_click(
    inv: &mut crate::inventory::Inventory,
    slot: &mut Option<Stack>,
    mode: u8,
    button: u8,
    output: bool,
) -> bool {
    if button > 1 {
        return false;
    }
    if mode == 1 {
        if let Some(stack) = *slot
            && inv.add(stack)
        {
            *slot = None;
        }
        return true;
    }
    if mode != 0 {
        return false;
    }
    if output {
        transfer(slot, &mut inv.cursor, 64);
    } else {
        click_one(&mut inv.cursor, slot, button);
    }
    true
}
