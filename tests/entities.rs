use syrust_engine::{
    entities::{Entity, EntityArena, EntityKind},
    packets::Stack,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;

thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

fn count_allocation() {
    let _ = ALLOCATIONS.try_with(|count| {
        if let Some(n) = count.get() {
            count.set(Some(n + 1));
        }
    });
}

// SAFETY: Each operation delegates to System with the original pointer and layout.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn entity(kind: EntityKind) -> Entity {
    Entity {
        kind,
        position: [-0.25, 64.0, 15.75],
        velocity: [0.5, -0.25, 0.5],
        age: 0,
    }
}

#[test]
fn reuses_slots_without_reusing_handles_or_invalidating_the_moved_entity() {
    let mut arena = EntityArena::with_capacity(2);
    let item = entity(EntityKind::Item(Stack {
        id: 1,
        count: 2,
        damage: 0,
    }));
    let arrow = entity(EntityKind::Arrow);
    let orb = entity(EntityKind::Orb { value: 3 });
    let removed = arena.insert(item).unwrap();
    let moved = arena.insert(arrow).unwrap();
    assert_eq!(arena.insert(orb), Err(orb));

    assert_eq!(arena.remove(removed), Some(item));
    assert_eq!(arena.get(moved), Some(&arrow));
    let replacement = arena.insert(orb).unwrap();
    assert_ne!(replacement, removed);
    assert_eq!(arena.get(removed), None);
    assert_eq!(arena.remove(removed), None);
    assert_eq!(arena.get(replacement), Some(&orb));
    assert_eq!(arena.remove(moved), Some(arrow));
    assert_eq!(arena.remove(replacement), Some(orb));
    assert_eq!(arena.iter().len(), 0);
}

#[test]
fn tick_advances_entities_across_columns_without_allocating_after_warmup() {
    let mut arena = EntityArena::with_capacity(4);
    let arrow = arena.insert(entity(EntityKind::Arrow)).unwrap();
    assert_eq!(arena.get(arrow).unwrap().column(), (-1, 0));
    arena.tick();
    assert_eq!(arena.get(arrow).unwrap().column(), (0, 1));
    assert_eq!(arena.get(arrow).unwrap().position, [0.25, 63.75, 16.25]);

    ALLOCATIONS.with(|count| count.set(Some(0)));
    for _ in 0..1024 {
        let mob = arena.insert(entity(EntityKind::Mob { kind: 54 })).unwrap();
        arena.tick();
        assert_eq!(arena.remove(mob).unwrap().age, 1);
    }
    let allocations = ALLOCATIONS.with(|count| count.replace(None));

    assert_eq!(allocations, Some(0));
    assert_eq!(arena.get(arrow).unwrap().age, 1025);
}

#[test]
fn cached_column_packet_needs_no_new_allocation() {
    let template = syrust_engine::arena::Template::flat(0);
    let first = template.packet((0, 0)).unwrap();
    ALLOCATIONS.with(|count| count.set(Some(0)));
    for _ in 0..1024 {
        let cached = template.packet((0, 0)).unwrap();
        assert_eq!(cached.body().as_ptr(), first.body().as_ptr());
    }
    let allocations = ALLOCATIONS.with(|count| count.replace(None));
    assert_eq!(allocations, Some(0));
}
