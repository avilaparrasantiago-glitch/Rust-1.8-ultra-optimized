use super::{Player, Room, Window};
use crate::{
    arena::BlockPos,
    entities::{Entity, EntityArena, EntityKind},
    packets::{self as p, Stack},
    wire::{Packet, WriteWire},
};

impl Room {
    pub(super) fn break_block(&mut self, player: usize, pos: BlockPos, state: u16, survival: bool) {
        let drop = if survival {
            let block = state >> 4;
            let stack = Stack {
                id: if block == 2 { 3 } else { block as i16 },
                count: 1,
                damage: (state & 15) as i16,
            };
            let (x, y, z) = pos;
            let item = Entity {
                kind: EntityKind::Item(stack),
                position: [f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5],
                velocity: [0.; 3],
                age: 0,
            };
            let Ok(handle) = self.entities.insert(item) else {
                self.players[player].send(p::block(pos, state));
                return;
            };
            Some(handle)
        } else {
            None
        };
        if self.set_block(pos, 0).is_err() {
            if let Some(handle) = drop {
                self.entities.remove(handle);
            }
            self.players[player].send(p::block(pos, state));
            return;
        }
        self.broadcast(p::effect(2001, pos, i32::from(state)));
    }
}
pub(super) fn drop_xp(entities: &mut EntityArena, victim: &mut Player) {
    let value = (victim.level() * 7).clamp(0, 100) as u16;
    if value == 0 {
        return;
    }
    let orb = Entity {
        kind: EntityKind::Orb { value },
        position: victim.pos,
        velocity: [0.; 3],
        age: 0,
    };
    if entities.insert(orb).is_ok() {
        victim.add_xp(-i32::from(value));
    }
}

fn attract_orb(e: &mut Entity, value: u16, players: &mut [Player]) -> bool {
    let col = e.column();
    let nearest = players
        .iter_mut()
        .filter(|p| {
            p.hp > 0.
                && p.mode != 3
                && p.loaded.contains(&col)
                && p.xp.checked_add(i32::from(value)).is_some()
        })
        .map(|p| {
            let target = [p.pos[0], p.pos[1] + 0.8, p.pos[2]];
            let squared = super::distance(target, e.position);
            (p, target, squared)
        })
        .filter(|(_, _, squared)| *squared <= 64.)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((player, target, squared)) = nearest else {
        return true;
    };
    if e.age >= 10 && within_pickup_box(player.pos, e.position) {
        player.add_xp(i32::from(value));
        return false;
    }
    if squared > 0. {
        let fraction = (0.25 / squared.sqrt()).min(1.);
        for (coordinate, goal) in e.position.iter_mut().zip(target) {
            *coordinate += (goal - *coordinate) * fraction;
        }
    }
    true
}

pub(super) fn collect_drops(entities: &mut EntityArena, players: &mut [Player]) {
    entities.retain(|handle, e| {
        if let EntityKind::Orb { value } = e.kind {
            return attract_orb(e, value, players);
        }
        let EntityKind::Item(stack) = e.kind else {
            return true;
        };
        if e.age < 10 {
            return true;
        }
        for i in 0..players.len() {
            let player = &mut players[i];
            if player.hp <= 0.
                || player.mode == 3
                || !player.loaded.contains(&e.column())
                || !within_pickup_box(player.pos, e.position)
            {
                continue;
            }
            let before = player.inv.slots;
            if !player.inv.add(stack) {
                continue;
            }
            let collector = player.id;
            let collect = Packet::new(0x0d, |w| {
                w.var(handle.wire_id());
                w.var(collector);
            });
            for viewer in players.iter() {
                if viewer.seen_entities.contains_key(&handle) {
                    viewer.send(collect.clone());
                }
            }
            sync_slots(&players[i], &before);
            return false;
        }
        true
    });
}

// The 0.6 x 1.8 player box expands by one block horizontally and half a
// block vertically. The 0.25-wide item box starts at its entity position.
fn within_pickup_box([x, y, z]: [f64; 3], [ix, iy, iz]: [f64; 3]) -> bool {
    (ix - x).abs() < 1.425 && (iz - z).abs() < 1.425 && iy < y + 2.3 && iy + 0.25 > y - 0.5
}

fn sync_slots(player: &Player, before: &[Option<Stack>; 45]) {
    let (window, offset) = match player.window {
        None => (0, 9),
        Some(Window::Table(_)) => (1, 10),
        Some(Window::Furnace(_)) => (1, 3),
    };
    for (i, (old, new)) in before[9..].iter().zip(&player.inv.slots[9..]).enumerate() {
        if old != new {
            player.send(p::slot(window, offset + i as i16, *new));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_inventory_leaves_drop_available_to_another_player() {
        let stack = Stack {
            id: 3,
            count: 1,
            damage: 0,
        };
        let mut arena = EntityArena::with_capacity(1);
        let handle = arena
            .insert(Entity {
                kind: EntityKind::Item(stack),
                position: [0.; 3],
                velocity: [0.; 3],
                age: 10,
            })
            .unwrap();
        let mut players = Vec::new();
        let mut outputs = Vec::new();
        for i in 0..2 {
            let (_, input) = crossbeam_channel::bounded(1);
            let (output, rx) = tokio::sync::mpsc::channel(8);
            let mut p = Player::new(
                i,
                crate::identity::offline("Test", [127, 0, 0, 1].into()),
                input,
                output,
            );
            p.loaded.insert((0, 0));
            players.push(p);
            outputs.push(rx);
        }
        players[0].inv.slots[9..].fill(Some(Stack { count: 64, ..stack }));
        players[1].pos = [2., 0., 0.];
        collect_drops(&mut arena, &mut players);
        assert!(arena.get(handle).is_some());
        players[1].pos = [1., 0., 0.];
        collect_drops(&mut arena, &mut players);
        assert!(arena.get(handle).is_none());
        assert_eq!(players[1].inv.slots[9], Some(stack));
        assert!(outputs[0].try_recv().is_err());
    }
}
