use super::{Player, column, distance, line_clear};
use crate::{
    arena::{ColumnPos, World},
    entities::{EntityArena, EntityId, Kind},
    packets::{self, Stack},
    wire::{Packet, WriteWire},
};
use bytes::{BufMut, BytesMut};

pub(super) const CAPACITY: usize = 4096;

fn wire_id(slot: usize) -> i32 {
    super::FIRST_ROOM_ENTITY + slot as i32
}

fn spawn_body(w: &mut BytesMut, id: i32, kind: Kind, pos: [i32; 3]) {
    w.var(match kind {
        Kind::Mob(_) => 0x0f,
        Kind::Orb(_) => 0x11,
        _ => 0x0e,
    });
    w.var(id);
    match kind {
        Kind::Mob(t) => w.put_u8(t),
        Kind::Arrow => w.put_u8(60),
        Kind::Item(_) => w.put_u8(2),
        Kind::Orb(_) => {}
    }
    for n in pos {
        w.put_i32(n);
    }
    match kind {
        Kind::Mob(_) => {
            w.put_slice(&[0; 9]);
            w.put_u8(0x7f);
        }
        Kind::Orb(value) => w.put_i16(i16::try_from(value).unwrap_or(i16::MAX)),
        _ => w.put_slice(&[0; 6]),
    }
}

fn item_body(w: &mut BytesMut, id: i32, stack: Stack) {
    w.var(0x1c);
    w.var(id);
    w.put_u8(0xaa); // Slot metadata at index 10
    w.put_i16(stack.id);
    w.put_u8(stack.count);
    w.put_i16(stack.damage);
    w.put_slice(&[0, 0x7f]);
}

struct Cached<K> {
    key: Option<K>,
    packet: Packet,
}
impl<K: Copy + Eq> Cached<K> {
    fn new() -> Self {
        Self {
            key: None,
            packet: Packet::new(0, |_| {}),
        }
    }
    fn get(&mut self, key: K, write: impl FnOnce(&mut BytesMut)) -> Option<Packet> {
        if self.key != Some(key) {
            if !self.packet.rewrite(write) {
                return None;
            }
            self.key = Some(key);
        }
        Some(self.packet.clone())
    }
}

struct Frames {
    spawn: Cached<(Kind, [i32; 3])>,
    item: Cached<Stack>,
    movement: Cached<[i32; 3]>,
    destroy: Packet,
}
#[derive(Clone, Copy)]
pub(super) struct Seen {
    id: EntityId,
    kind: Kind,
    pos: [i32; 3],
    tick: u64,
}
pub(super) struct EntityView {
    columns: Vec<(ColumnPos, EntityId)>,
    frames: Vec<Frames>,
}
impl EntityView {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            columns: Vec::with_capacity(capacity),
            frames: (0..capacity)
                .map(|slot| Frames {
                    spawn: Cached::new(),
                    item: Cached::new(),
                    movement: Cached::new(),
                    destroy: packets::destroy(wire_id(slot)),
                })
                .collect(),
        }
    }

    pub(super) fn index(&mut self, arena: &EntityArena) {
        self.columns.clear();
        self.columns
            .extend(arena.iter().map(|(id, e)| (column(e.position), id)));
        self.columns.sort_unstable_by_key(|entry| entry.0);
    }

    pub(super) fn update(
        &mut self,
        arena: &EntityArena,
        world: &World,
        viewer: &mut Player,
        tick: u64,
    ) {
        if self.columns.is_empty() && viewer.seen_entities.is_empty() {
            return;
        }
        let center = column(viewer.pos);
        // Rotate the first candidate so dense columns also progress under backpressure.
        let split = (tick as usize)
            .wrapping_mul(31)
            .checked_rem(self.columns.len())
            .unwrap_or(0);
        let (head, tail) = self.columns.split_at(split);
        for columns in [tail, head] {
            for &col in &viewer.loaded {
                if center.0.abs_diff(col.0) > 5 || center.1.abs_diff(col.1) > 5 {
                    continue;
                }
                let start = columns.partition_point(|entry| entry.0 < col);
                for &(_, id) in columns[start..].iter().take_while(|entry| entry.0 == col) {
                    let Some(entity) = arena.get(id) else {
                        continue;
                    };
                    let d = distance(viewer.pos, entity.position);
                    if d > 80. * 80. {
                        continue;
                    }
                    let interval = if d < 256. {
                        1
                    } else if d < 48. * 48. {
                        2
                    } else {
                        4
                    };
                    let seen = viewer.seen_entities.get_mut(&id.slot);
                    let same = seen
                        .as_ref()
                        .is_some_and(|s| s.id == id && s.kind == entity.kind);
                    if same && !tick.is_multiple_of(interval) {
                        if let Some(s) = seen {
                            s.tick = tick;
                        }
                        continue;
                    }
                    if d >= 256. && !line_clear(world, viewer.pos, entity.position) {
                        continue;
                    }
                    let pos = entity.position.map(|n| (n * 32.).floor() as i32);
                    let frames = &mut self.frames[id.slot];
                    let net = wire_id(id.slot);
                    if same {
                        if let Some(s) = seen {
                            s.tick = tick;
                            if s.pos != pos
                                && viewer.output.remaining() > 32
                                && let Some(packet) = frames.movement.get(pos, |w| {
                                    w.var(0x18);
                                    w.var(net);
                                    for n in pos {
                                        w.put_i32(n);
                                    }
                                    w.put_slice(&[0; 3]);
                                })
                            {
                                s.pos = pos;
                                viewer.send(packet);
                            }
                        }
                        continue;
                    }
                    // Leave space for other gameplay packets and keep item spawn + metadata together.
                    if viewer.output.remaining() < 32 {
                        continue;
                    }
                    let Some(spawn) = frames
                        .spawn
                        .get((entity.kind, pos), |w| spawn_body(w, net, entity.kind, pos))
                    else {
                        continue;
                    };
                    let metadata = if let Kind::Item(stack) = entity.kind {
                        let Some(packet) = frames.item.get(stack, |w| item_body(w, net, stack))
                        else {
                            continue;
                        };
                        Some(packet)
                    } else {
                        None
                    };
                    if seen.is_some() {
                        viewer.send(frames.destroy.clone());
                    }
                    viewer.send(spawn);
                    if let Some(packet) = metadata {
                        viewer.send(packet);
                    }
                    viewer.seen_entities.insert(
                        id.slot,
                        Seen {
                            id,
                            kind: entity.kind,
                            pos,
                            tick,
                        },
                    );
                }
            }
        }
        let out = &viewer.output;
        let failed = &viewer.failed;
        viewer.seen_entities.retain(|slot, s| {
            if s.tick == tick {
                return true;
            }
            if out.remaining() < 16 {
                return true;
            }
            out.send(self.frames[*slot].destroy.clone(), failed);
            false
        });
    }
}

#[cfg(test)]
#[path = "entity_view_tests.rs"]
mod tests;
