use super::{ENTITIES_PER_ROOM, Player, distance};
use crate::{
    arena::{ColumnPos, World},
    entities::{Entity, EntityArena, EntityId, EntityKind},
    packets as p,
    wire::{Packet, WriteWire},
};
use bytes::BufMut;
use std::collections::HashMap;

// Protocol 47 does not report the client FOV. Use a generous 160 x 130 degree
// frustum and a margin around the target to avoid clipping at the screen edge.
pub(super) struct View {
    yaw: (f64, f64),
    pitch: (f64, f64),
}

impl View {
    pub(super) fn new([yaw, pitch]: [f32; 2]) -> Self {
        Self {
            yaw: f64::from(yaw).to_radians().sin_cos(),
            pitch: f64::from(pitch).to_radians().sin_cos(),
        }
    }

    pub(super) fn visible(&self, world: &World, eye: [f64; 3], pos: [f64; 3], height: f64) -> bool {
        let [dx, dy, dz] = [
            pos[0] - eye[0],
            pos[1] + height * 0.5 - eye[1],
            pos[2] - eye[2],
        ];
        let (sy, cy) = self.yaw;
        let (sp, cp) = self.pitch;
        let horizontal = dz * cy - dx * sy;
        let depth = horizontal * cp - dy * sp;
        let side = dx * cy + dz * sy;
        let up = dy * cp + horizontal * sp;
        let margin = height.max(1.);
        if depth < -margin
            || side.abs() > (depth + margin) * 5.6713 + margin
            || up.abs() > (depth + margin) * 2.1445 + margin
        {
            return false;
        }
        let radius = (height * 0.3).clamp(0.125, 1.);
        [
            (0., height * 0.5, 0.),
            (-radius, height * 0.95, -radius),
            (radius, height * 0.95, radius),
            (-radius, height * 0.05, radius),
            (radius, height * 0.05, -radius),
        ]
        .into_iter()
        .any(|(x, y, z)| super::clear_ray(world, eye, [pos[0] + x, pos[1] + y, pos[2] + z]))
    }
}

pub(super) fn interval(d2: f64) -> u64 {
    if d2 < 16. * 16. {
        1
    } else if d2 < 48. * 48. {
        2
    } else {
        4
    }
}

pub(super) type Seen = HashMap<EntityId, ([i32; 3], Packet)>;

pub(super) fn empty_seen() -> Seen {
    HashMap::with_capacity(ENTITIES_PER_ROOM)
}

fn spawn(id: i32, e: &Entity, pos: [i32; 3]) -> (Packet, Option<Packet>) {
    let packet_id = match e.kind {
        EntityKind::Item(_)
        | EntityKind::Arrow
        | EntityKind::EndCrystal { .. }
        | EntityKind::EnderEye { .. } => 0x0e,
        EntityKind::Mob { .. } | EntityKind::Villager { .. } => 0x0f,
        EntityKind::Orb { .. } => 0x11,
    };
    let first = Packet::new(packet_id, |w| {
        w.var(id);
        match e.kind {
            EntityKind::Item(_) => w.put_u8(2),
            EntityKind::Arrow => w.put_u8(60),
            EntityKind::EnderEye { .. } => w.put_u8(72),
            EntityKind::EndCrystal { .. } => w.put_u8(51),
            EntityKind::Mob { kind } => w.put_u8(kind),
            EntityKind::Villager { .. } => w.put_u8(120),
            EntityKind::Orb { .. } => {}
        }
        for x in pos {
            w.put_i32(x);
        }
        match e.kind {
            EntityKind::Orb { value } => w.put_i16(value.min(i16::MAX as u16) as i16),
            EntityKind::Mob { .. } | EntityKind::Villager { .. } => {
                w.extend_from_slice(&[0; 3]);
                for v in e.velocity {
                    w.put_i16((v.clamp(-3.9, 3.9) * 8000.) as i16);
                }
                if let EntityKind::Villager { profession } = e.kind {
                    w.put_u8((2 << 5) | 16);
                    w.put_i32(profession);
                }
                w.put_u8(0x7f);
            }
            EntityKind::Arrow | EntityKind::Item(_) | EntityKind::EnderEye { .. } => {
                w.put_u16(0);
                w.put_i32(0);
            }
            EntityKind::EndCrystal { yaw } => {
                w.put_u8(0);
                w.put_u8(yaw);
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
    world: &World,
    tick: u64,
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
                    let d2 = distance(player.pos, entity.position);
                    if !tick.is_multiple_of(interval(d2)) {
                        continue;
                    }
                    let height = match entity.kind {
                        EntityKind::Mob { .. } | EntityKind::Villager { .. } => 2.,
                        EntityKind::Arrow => 0.5,
                        _ => 0.25,
                    };
                    if d2 >= 256.
                        && !player
                            .view
                            .visible(world, player.eye(), entity.position, height)
                    {
                        if let Some((_, packet)) = player.seen_entities.remove(&handle) {
                            player.send(packet);
                        }
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
    fn distant_entities_respect_view_and_walls() {
        let mut world = crate::arena::World::new(
            "test".into(),
            std::sync::Arc::new(crate::arena::Template::flat(2)),
        );
        let camera = View::new([0., 0.]);
        let eye = [0.5, 5.62, 0.5];
        assert!(camera.visible(&world, eye, [0.5, 4., 24.5], 1.8));
        assert!(!camera.visible(&world, eye, [0.5, 4., -24.5], 1.8));
        assert!(View::new([180., 0.]).visible(&world, eye, [0.5, 4., -24.5], 1.8));
        for x in -1..=1 {
            for y in 4..=7 {
                world.set((x, y, 12), 16).unwrap();
            }
        }
        assert!(!camera.visible(&world, eye, [0.5, 4., 24.5], 1.8));
        world.set((0, 5, 12), 0).unwrap();
        assert!(camera.visible(&world, eye, [0.5, 4., 24.5], 1.8));
        assert_eq!([interval(225.), interval(256.), interval(2304.)], [1, 2, 4]);
    }

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
        let world = World::new(
            "test".into(),
            std::sync::Arc::new(crate::arena::Template::flat(0)),
        );
        let orb = Entity {
            kind: EntityKind::Orb { value: 3 },
            position: [1., 0., 0.],
            velocity: [1., 0., 0.],
            age: 0,
        };
        let id = arena.insert(orb).unwrap();
        broadcast(&world, 0, &arena, &grid, &mut players);
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
        broadcast(&world, 0, &arena, &grid, &mut players);
        assert_eq!(receivers[0].try_recv().unwrap().body()[0], 0x18);
        assert_eq!(
            receivers[1].try_recv().unwrap().body(),
            &[0x13, 1, 255, 255, 255, 255, 15]
        );
        arena.remove(id);
        arena.insert(orb).unwrap();
        broadcast(&world, 0, &arena, &grid, &mut players);
        assert_eq!(receivers[0].try_recv().unwrap().body()[0], 0x13);
        assert_eq!(receivers[0].try_recv().unwrap().body(), first.body());
        assert!(receivers[2].try_recv().is_err());
    }
}
