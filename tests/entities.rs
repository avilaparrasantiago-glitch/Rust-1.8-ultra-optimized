use coral_engine::entities::{Entity, EntityArena, Kind};
use coral_engine::packets::Stack;

#[test]
fn recycling_preserves_dense_survivors_and_rejects_stale_handles() {
    let mut arena = EntityArena::with_capacity(4);
    let kinds = [
        Kind::Item(Stack {
            id: 1,
            count: 3,
            damage: 0,
        }),
        Kind::Orb(2),
        Kind::Arrow,
        Kind::Mob(54),
    ];
    let ids = kinds.map(|kind| arena.spawn(Entity::new(kind, [1.; 3])).unwrap());
    assert!(arena.spawn(Entity::new(Kind::Arrow, [0.; 3])).is_err());
    assert_eq!(arena.remove(ids[1]).unwrap().kind, Kind::Orb(2));
    let replacement = arena.spawn(Entity::new(Kind::Orb(7), [5.; 3])).unwrap();
    assert_ne!(replacement, ids[1]);
    assert!(arena.remove(ids[1]).is_none());
    assert!(arena.get_mut(ids[1]).is_none());
    arena.get_mut(ids[3]).unwrap().velocity = [0., 1., -2.];
    arena.tick();
    assert_eq!(arena.get(ids[3]).unwrap().position, [1., 2., -1.]);
    assert_eq!(arena.get(replacement).unwrap().age, 1);
    assert_eq!(arena.iter().count(), 4);
    for (id, entity) in arena.iter() {
        assert_eq!(arena.get(id), Some(entity));
    }
    for id in [replacement, ids[0], ids[2], ids[3]] {
        assert!(arena.remove(id).is_some());
        assert!(arena.get(id).is_none());
    }
    assert_eq!(arena.iter().count(), 0);
}

#[path = "support/allocations.rs"]
mod allocations;
use allocations::ALLOCATIONS;

#[test]
fn full_arena_ticks_and_recycles_without_allocating() {
    let mut arena = EntityArena::with_capacity(4096);
    let arrow = Entity::new(Kind::Arrow, [0.; 3]);
    let mut recycled = arena.spawn(arrow).unwrap();
    for _ in 1..4096 {
        arena.spawn(arrow).unwrap();
    }
    arena.tick();
    let before = ALLOCATIONS.get();
    for _ in 0..200 {
        let removed = arena.remove(recycled).unwrap();
        recycled = arena.spawn(removed).unwrap();
        arena.tick();
    }
    assert_eq!(ALLOCATIONS.get(), before);
    assert_eq!(arena.get(recycled).unwrap().age, 201);
}
