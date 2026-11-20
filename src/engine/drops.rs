use super::{Player, Room, Window, column, solid};
use crate::{
    arena::BlockPos,
    entities::{Entity, Kind},
    packets::{self as p, Stack},
    wire::WriteWire,
};

const PICKUP_DELAY: u64 = 10;
const ITEM_LIFETIME: u64 = 6000;
// Player half-width (0.3), pickup expansion (1.0), and item half-width (0.125).
const HORIZONTAL_REACH: f64 = 1.425;

impl Room {
    pub(super) fn break_block(&mut self, miner: usize, pos: BlockPos, state: u16, survival: bool) {
        let drop = if survival {
            let id = state >> 4;
            let stack = Stack {
                id: if id == 2 { 3 } else { id as i16 },
                count: 1,
                damage: (state & 15) as i16,
            };
            let center = [
                f64::from(pos.0) + 0.5,
                f64::from(pos.1) + 0.5,
                f64::from(pos.2) + 0.5,
            ];
            let Ok(handle) = self.entities.spawn(Entity::new(Kind::Item(stack), center)) else {
                self.players[miner].send(p::block(pos, state));
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
            self.players[miner].send(p::block(pos, state));
            return;
        }
        self.broadcast(p::effect(2001, pos, i32::from(state)));
    }

    pub(super) fn tick_drops(&mut self) {
        let players = &mut self.players;
        self.entities.retain_mut(|entity| {
            let Kind::Item(stack) = entity.kind else {
                return true;
            };
            if entity.age >= ITEM_LIFETIME || entity.position[1] < -64. {
                return false;
            }
            let [x, y, z] = entity.position;
            let below = y - 0.001;
            if solid(&self.world, [x, below, z]) {
                entity.position[1] = below.floor() + 1.;
                entity.velocity[1] = 0.;
            } else {
                entity.velocity[1] = ((entity.velocity[1] - 0.04) * 0.98).max(-0.9);
            }
            if entity.age < PICKUP_DELAY {
                return true;
            }
            let lo = column([x - HORIZONTAL_REACH, y, z - HORIZONTAL_REACH]);
            let hi = column([x + HORIZONTAL_REACH, y, z + HORIZONTAL_REACH]);
            for cx in lo.0..=hi.0 {
                for cz in lo.1..=hi.1 {
                    for &i in self.grid.get(&(cx, cz)).into_iter().flatten() {
                        let p = &mut players[i];
                        let dy = entity.position[1] - p.pos[1];
                        if p.mode != 3
                            && p.hp > 0.
                            && p.pending.is_none()
                            && (p.pos[0] - x).abs() < HORIZONTAL_REACH
                            && (p.pos[2] - z).abs() < HORIZONTAL_REACH
                            && dy > -0.75
                            && dy < 2.3
                            && p.pick_up(stack)
                        {
                            return false;
                        }
                    }
                }
            }
            true
        });
    }
}

impl Player {
    fn pick_up(&mut self, stack: Stack) -> bool {
        let before = self.inv.slots;
        if !self.inv.add(stack) {
            return false;
        }
        let (window, base) = match self.window {
            None => (0, 9),
            Some(Window::Table(_)) => (1, 10),
            Some(Window::Furnace(_)) => (1, 3),
        };
        let mut changed = 0u64;
        for (i, (&old, &new)) in before[9..].iter().zip(&self.inv.slots[9..]).enumerate() {
            if old != new {
                let ready = self.pickup_slots[i].rewrite(|w| {
                    w.var(0x2f);
                    p::slot_fields(w, window, base + i as i16, new);
                });
                if !ready {
                    self.inv.slots = before;
                    return false;
                }
                changed |= 1 << i;
            }
        }
        if self.output.remaining() < changed.count_ones() as usize {
            self.inv.slots = before;
            return false;
        }
        while changed != 0 {
            let i = changed.trailing_zeros() as usize;
            self.send(self.pickup_slots[i].clone());
            changed &= changed - 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{allocations::ALLOCATIONS, identity::offline};
    use crossbeam_channel::bounded;
    use std::net::Ipv4Addr;

    #[test]
    fn pickup_preserves_full_inventory_and_retries_in_furnace_window() {
        let (output, packets) = bounded(2);
        let mut player = Player::with_queue(
            1,
            offline("Bob", Ipv4Addr::LOCALHOST.into()),
            bounded(1).1,
            output,
        );
        let item = Stack {
            id: 3,
            count: 1,
            damage: 0,
        };
        player.inv.slots[9..].fill(Some(Stack { count: 64, ..item }));
        assert!(!player.pick_up(item));
        assert!(packets.is_empty());
        player.window = Some(Window::Furnace((0, 0, 0)));
        player.inv.slots[9] = None;
        assert!(player.pick_up(item));
        let packet = packets.try_recv().unwrap();
        assert_eq!(packet.body(), &[0x2f, 1, 0, 3, 0, 3, 1, 0, 0, 0]);
        assert!(!player.pick_up(item));
        assert_eq!(player.inv.slots[9], Some(item));
        drop(packet);
        let before = ALLOCATIONS.get();
        for _ in 0..200 {
            player.inv.slots[9] = None;
            assert!(player.pick_up(item));
            drop(packets.try_recv().unwrap());
        }
        assert_eq!(ALLOCATIONS.get(), before);
    }
}
