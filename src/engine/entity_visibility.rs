use super::{ENTITIES_PER_ROOM, Player, distance};
use crate::{
    arena::ColumnPos,
    entities::{Entity, EntityArena, EntityId, EntityKind},
    packets as p,
    wire::{Packet, WriteWire},
};
use bytes::BufMut;
use std::collections::HashMap;

pub(super) type Seen = HashMap<EntityId, ([i32; 3], Packet)>;

pub(super) fn empty_seen() -> Seen {
    HashMap::with_capacity(ENTITIES_PER_ROOM)
}

fn spawn(id: i32, e: &Entity, pos: [i32; 3]) -> (Packet, Option<Packet>) {
    let packet_id = match e.kind {
        EntityKind::Item(_) | EntityKind::Arrow => 0x0e,
        EntityKind::Mob { .. } => 0x0f,
        EntityKind::Orb { .. } => 0x11,
    };
    let first = Packet::new(packet_id, |w| {
        w.var(id);
        match e.kind {
            EntityKind::Item(_) => w.put_u8(2),
            EntityKind::Arrow => w.put_u8(60),
            EntityKind::Mob { kind } => w.put_u8(kind),
            EntityKind::Orb { .. } => {}
        }
        for x in pos {
            w.put_i32(x);
        }
        match e.kind {
            EntityKind::Orb { value } => w.put_i16(value.min(i16::MAX as u16) as i16),
            EntityKind::Mob { .. } => {
                w.extend_from_slice(&[0; 3]);
                for v in e.velocity {
                    w.put_i16((v.clamp(-3.9, 3.9) * 8000.) as i16);
                }
                w.put_u8(0x7f);
            }
            EntityKind::Arrow | EntityKind::Item(_) => {
                w.put_u16(0);
                w.put_i32(0);
            }
        }
    });
    let extra = match e.kind {
        EntityKind::Item(stack) => Some(Packet::new(0x1c, |w| {
            w.var(id);
            w.put_u8(0xaa);
            w.put_i16(stack.id);
            w.put_u8(stack.count);
            w.put_i16(stack.damage);
            w.extend_from_slice(&[0, 0x7f]);
        })),
        _ => None,
    };
    (first, extra)
}

pub(super) fn broadcast(
    arena: &EntityArena,
    grid: &HashMap<ColumnPos, Vec<usize>>,
    players: &mut [Player],
) {
    for player in players.iter_mut() {
        let mut seen = std::mem::take(&mut player.seen_entities);
        seen.retain(|id, (_, destroy)| {
            let visible = arena.get(*id).is_some_and(|e| {
                player.loaded.contains(&e.column()) && distance(player.pos, e.position) <= 80. * 80.
            });
            if !visible {
                player.send(destroy.clone());
            }
            visible
        });
        player.seen_entities = seen;
    }

    for (handle, entity) in arena.iter() {
        let id = handle.wire_id();
        let pos = entity.position.map(|v| (v * 32.).floor() as i32);
        let center = entity.column();
        let mut spawn_packets = None;
        let mut movement = None;
        let mut destroy = None;
        for x in center.0 - 5..=center.0 + 5 {
            for z in center.1 - 5..=center.1 + 5 {
                for &i in grid.get(&(x, z)).into_iter().flatten() {
                    let player = &mut players[i];
                    if !player.loaded.contains(&center)
                        || distance(player.pos, entity.position) > 80. * 80.
                    {
                        continue;
                    }
                    if let Some((previous, _)) = player.seen_entities.get_mut(&handle) {
                        if *previous != pos {
                            *previous = pos;
                            player.send(
                                movement
                                    .get_or_insert_with(|| {
                                        p::entity_pos(id, entity.position, [0.; 2], false)
                                    })
                                    .clone(),
                            );
                        }
                    } else {
                        let (first, extra) =
                            spawn_packets.get_or_insert_with(|| spawn(id, entity, pos));
                        player.send(first.clone());
                        if let Some(packet) = extra {
                            player.send(packet.clone());
                        }
                        player.seen_entities.insert(
                            handle,
                            (pos, destroy.get_or_insert_with(|| p::destroy(id)).clone()),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entities::EntityKind, identity::offline};

    #[test]
    fn object_and_mob_packet_vectors() {
        let mut e = Entity {
            kind: EntityKind::Arrow,
            position: [0.; 3],
            velocity: [0.; 3],
            age: 0,
        };
        let (arrow, _) = spawn(1, &e, [0; 3]);
        assert_eq!(
            arrow.body(),
            &[
                14, 1, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        e.kind = EntityKind::Item(p::Stack {
            id: 5,
            count: 2,
            damage: 3,
        });
        let (item, metadata) = spawn(1, &e, [0; 3]);
        assert_eq!(
            item.body(),
            &[
                14, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        assert_eq!(
            metadata.unwrap().body(),
            &[28, 1, 170, 0, 5, 2, 0, 3, 0, 127]
        );
        e.kind = EntityKind::Mob { kind: 54 };
        e.velocity = [0.5, 0., -0.5];
        assert_eq!(
            spawn(1, &e, [0; 3]).0.body(),
            &[
                15, 1, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 15, 160, 0, 0, 240, 96, 127
            ]
        );
    }

    #[test]
    fn shares_spawns_moves_and_destroys_before_slot_reuse() {
        let mut players = Vec::new();
        let mut receivers = Vec::new();
        for i in 0..3 {
            let (_, input) = crossbeam_channel::bounded(1);
            let (output, rx) = tokio::sync::mpsc::channel(16);
            let mut p = Player::new(i, offline("Test", [127, 0, 0, 1].into()), input, output);
            if i < 2 {
                p.loaded.insert((0, 0));
            }
            players.push(p);
            receivers.push(rx);
        }
        let grid = HashMap::from([((0, 0), vec![0, 1, 2])]);
        let mut arena = EntityArena::with_capacity(1);
        let orb = Entity {
            kind: EntityKind::Orb { value: 3 },
            position: [1., 0., 0.],
            velocity: [1., 0., 0.],
            age: 0,
        };
        let id = arena.insert(orb).unwrap();
        broadcast(&arena, &grid, &mut players);
        let first = receivers[0].try_recv().unwrap();
        assert_eq!(
            first.body(),
            &[
                0x11, 255, 255, 255, 255, 15, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3
            ]
        );
        assert_eq!(
            first.body().as_ptr(),
            receivers[1].try_recv().unwrap().body().as_ptr()
        );
        arena.tick();
        players[1].loaded.clear();
        broadcast(&arena, &grid, &mut players);
        assert_eq!(receivers[0].try_recv().unwrap().body()[0], 0x18);
        assert_eq!(
            receivers[1].try_recv().unwrap().body(),
            &[0x13, 1, 255, 255, 255, 255, 15]
        );
        arena.remove(id);
        arena.insert(orb).unwrap();
        broadcast(&arena, &grid, &mut players);
        assert_eq!(receivers[0].try_recv().unwrap().body()[0], 0x13);
        assert_eq!(receivers[0].try_recv().unwrap().body(), first.body());
        assert!(receivers[2].try_recv().is_err());
    }
}
