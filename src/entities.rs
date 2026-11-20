use crate::packets::Stack;

/// A handle is meaningful only in the arena that issued it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId {
    slot: usize,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Item(Stack),
    Orb(u16),
    Arrow,
    Mob(u8),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entity {
    pub kind: Kind,
    pub position: [f64; 3],
    /// Displacement in blocks per simulation tick.
    pub velocity: [f64; 3],
    pub age: u64,
}

impl Entity {
    pub fn new(kind: Kind, position: [f64; 3]) -> Self {
        Self {
            kind,
            position,
            velocity: [0.; 3],
            age: 0,
        }
    }
}

struct Slot {
    generation: u64,
    dense: usize,
}

pub struct EntityArena {
    slots: Vec<Slot>,
    free: Vec<usize>,
    active: Vec<(EntityId, Entity)>,
}

impl EntityArena {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: (0..capacity)
                .map(|_| Slot {
                    generation: 0,
                    dense: usize::MAX,
                })
                .collect(),
            free: (0..capacity).rev().collect(),
            active: Vec::with_capacity(capacity),
        }
    }

    /// Returns the original entity if the room has exhausted its slots.
    pub fn spawn(&mut self, entity: Entity) -> Result<EntityId, Entity> {
        let slot = self.free.pop().ok_or(entity)?;
        let id = EntityId {
            slot,
            generation: self.slots[slot].generation,
        };
        self.slots[slot].dense = self.active.len();
        self.active.push((id, entity));
        Ok(id)
    }

    fn index(&self, id: EntityId) -> Option<usize> {
        let slot = self.slots.get(id.slot)?;
        (slot.generation == id.generation && slot.dense != usize::MAX).then_some(slot.dense)
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        Some(&self.active[self.index(id)?].1)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        let i = self.index(id)?;
        Some(&mut self.active[i].1)
    }

    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        let i = self.index(id)?;
        let (_, entity) = self.active.swap_remove(i);
        if let Some((moved, _)) = self.active.get(i) {
            self.slots[moved.slot].dense = i;
        }
        let slot = &mut self.slots[id.slot];
        slot.dense = usize::MAX;
        // Retire the slot on overflow so an old handle cannot become valid again.
        if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            self.free.push(id.slot);
        }
        Some(entity)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (EntityId, &Entity)> {
        self.active.iter().map(|(id, entity)| (*id, entity))
    }

    pub fn tick(&mut self) {
        for (_, entity) in &mut self.active {
            entity.age = entity.age.saturating_add(1);
            for (position, velocity) in entity.position.iter_mut().zip(entity.velocity) {
                *position += velocity;
            }
        }
    }
}
