use crate::{
    arena::{ColumnPos, column},
    packets::Stack,
};

/// A room-local handle. A removed entity's handle cannot address its replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityId {
    slot: usize,
    generation: u64,
}

impl EntityId {
    pub(crate) fn wire_id(self) -> i32 {
        -1 - self.slot as i32
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EntityKind {
    EnderEye { target: [f64; 3] },
    Item(Stack),
    Orb { value: u16 },
    Arrow,
    EndCrystal { yaw: u8 },
    Mob { kind: u8 },
    Villager { profession: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entity {
    pub kind: EntityKind,
    pub position: [f64; 3],
    /// Displacement in blocks per tick.
    pub velocity: [f64; 3],
    pub age: u64,
}

impl Entity {
    pub(crate) fn ender_eye(position: [f64; 3], mut target: [f64; 3]) -> Self {
        let dx = target[0] - position[0];
        let dz = target[2] - position[2];
        let range = dx.hypot(dz);
        if range > 12. {
            target = [
                position[0] + 12. * dx / range,
                position[1] + 8.,
                position[2] + 12. * dz / range,
            ];
        }
        Self {
            kind: EntityKind::EnderEye { target },
            position,
            velocity: [0.; 3],
            age: 0,
        }
    }
    pub fn column(&self) -> ColumnPos {
        column(self.position)
    }
}

enum SlotState {
    Occupied(usize),
    Free(Option<usize>),
    Retired,
}

struct Slot {
    generation: u64,
    state: SlotState,
}

/// Bounded storage with a dense tick order and stable handles across swap-removal.
pub struct EntityArena {
    dense: Vec<(EntityId, Entity)>,
    slots: Vec<Slot>,
    free: Option<usize>,
    limit: usize,
}

impl EntityArena {
    pub fn with_capacity(limit: usize) -> Self {
        Self {
            dense: Vec::with_capacity(limit),
            slots: Vec::with_capacity(limit),
            free: None,
            limit,
        }
    }

    pub fn insert(&mut self, entity: Entity) -> Result<EntityId, Entity> {
        let index = if let Some(index) = self.free {
            let SlotState::Free(next) = self.slots[index].state else {
                unreachable!("free list points to a live or retired slot");
            };
            self.free = next;
            index
        } else {
            if self.slots.len() == self.limit {
                return Err(entity);
            }
            let index = self.slots.len();
            self.slots.push(Slot {
                generation: 0,
                state: SlotState::Free(None),
            });
            index
        };
        let slot = &mut self.slots[index];
        let id = EntityId {
            slot: index,
            generation: slot.generation,
        };
        slot.state = SlotState::Occupied(self.dense.len());
        self.dense.push((id, entity));
        Ok(id)
    }

    fn dense_index(&self, id: EntityId) -> Option<usize> {
        let slot = self.slots.get(id.slot)?;
        if slot.generation == id.generation
            && let SlotState::Occupied(index) = slot.state
        {
            return Some(index);
        }
        None
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        Some(&self.dense[self.dense_index(id)?].1)
    }

    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        let index = self.dense_index(id)?;
        let (_, entity) = self.dense.swap_remove(index);
        if let Some((moved, _)) = self.dense.get(index) {
            self.slots[moved.slot].state = SlotState::Occupied(index);
        }
        let slot = &mut self.slots[id.slot];
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            slot.state = SlotState::Free(self.free);
            self.free = Some(id.slot);
        } else {
            slot.state = SlotState::Retired;
        }
        Some(entity)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (EntityId, &Entity)> {
        self.dense.iter().map(|(id, entity)| (*id, entity))
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(EntityId, &mut Entity) -> bool) {
        let mut index = 0;
        while index < self.dense.len() {
            let (id, entity) = &mut self.dense[index];
            let id = *id;
            if keep(id, entity) {
                index += 1;
            } else {
                self.remove(id);
            }
        }
    }

    pub fn tick(&mut self) {
        self.retain(|_, entity| {
            for (position, velocity) in entity.position.iter_mut().zip(entity.velocity) {
                *position += velocity;
            }
            entity.age = entity.age.saturating_add(1);
            if let EntityKind::EnderEye { target } = entity.kind {
                let dx = target[0] - entity.position[0];
                let dz = target[2] - entity.position[2];
                let range = dx.hypot(dz);
                let speed = entity.velocity[0].hypot(entity.velocity[2]);
                let mut speed = speed + (range - speed) * 0.0025;
                if range < 1. {
                    speed *= 0.8;
                    entity.velocity[1] *= 0.8;
                }
                let angle = dz.atan2(dx);
                entity.velocity[0] = speed * angle.cos();
                entity.velocity[2] = speed * angle.sin();
                let vertical = if entity.position[1] < target[1] {
                    1.
                } else {
                    -1.
                };
                entity.velocity[1] += (vertical - entity.velocity[1]) * 0.015;
                return entity.age <= 80;
            }
            true
        });
    }
}
