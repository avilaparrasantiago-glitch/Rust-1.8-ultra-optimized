use super::*;
use crate::entities::{Entity, Kind};
use crate::packets::Stack;

#[test]
fn spawn_body_matches_protocol_47() {
    let mut buffer = BytesMut::with_capacity(48);
    let item = Kind::Item(Stack {
        id: 5,
        count: 3,
        damage: 2,
    });
    let pos = [32, -16, 8];
    let cases: &[(Kind, &[u8])] = &[
        (
            item,
            &[
                14, 7, 2, 0, 0, 0, 32, 255, 255, 255, 240, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0,
            ],
        ),
        (
            Kind::Arrow,
            &[
                14, 7, 60, 0, 0, 0, 32, 255, 255, 255, 240, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0,
            ],
        ),
        (
            Kind::Orb(513),
            &[17, 7, 0, 0, 0, 32, 255, 255, 255, 240, 0, 0, 0, 8, 2, 1],
        ),
        (
            Kind::Mob(54),
            &[
                15, 7, 54, 0, 0, 0, 32, 255, 255, 255, 240, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                127,
            ],
        ),
    ];
    for &(kind, expected) in cases {
        buffer.clear();
        spawn_body(&mut buffer, 7, kind, pos);
        assert_eq!(&buffer[..], expected);
    }
}

#[path = "../../tests/support/allocations.rs"]
mod allocations;
use crate::{arena::Template, identity::offline};
use crossbeam_channel::{Receiver, bounded};
use std::{net::Ipv4Addr, sync::Arc};

fn player() -> (Player, Receiver<Packet>) {
    let (tx, rx) = bounded(192);
    let mut p = Player::with_queue(
        1,
        offline("Alice", Ipv4Addr::LOCALHOST.into()),
        bounded(1).1,
        tx,
    );
    p.pos = [0., 10., 0.];
    p.loaded.insert((0, 0));
    (p, rx)
}

#[test]
fn shared_frames_follow_visibility_and_slot_reuse() {
    let world = World::new("test".into(), Arc::new(Template::flat(1)));
    let mut arena = EntityArena::with_capacity(1);
    let mut view = EntityView::new(1);
    let stack = Stack {
        id: 5,
        count: 3,
        damage: 2,
    };
    let id = arena
        .spawn(Entity::new(Kind::Item(stack), [1., 10., 0.]))
        .unwrap();
    let (mut a, qa) = player();
    let (mut b, qb) = player();
    b.loaded.clear();
    view.index(&arena);
    view.update(&arena, &world, &mut a, 1);
    view.update(&arena, &world, &mut b, 1);
    assert!(qb.is_empty());
    b.loaded.insert((0, 0));
    view.update(&arena, &world, &mut b, 2);
    let first = qa.try_recv().unwrap();
    let second = qb.try_recv().unwrap();
    assert_eq!(first.body().as_ptr(), second.body().as_ptr());
    for q in [&qa, &qb] {
        assert_eq!(
            q.try_recv().unwrap().body(),
            &[0x1c, 128, 128, 128, 128, 4, 0xaa, 0, 5, 3, 0, 2, 0, 127]
        );
    }
    drop((first, second));
    arena.get_mut(id).unwrap().position[0] = 2.;
    view.index(&arena);
    view.update(&arena, &world, &mut a, 3);
    let pending = qa.try_recv().unwrap();
    assert_eq!(
        pending.body(),
        &[
            0x18, 128, 128, 128, 128, 4, 0, 0, 0, 64, 0, 0, 1, 64, 0, 0, 0, 0, 0, 0, 0
        ]
    );
    arena.get_mut(id).unwrap().position[0] = 3.;
    view.index(&arena);
    view.update(&arena, &world, &mut a, 4);
    assert!(qa.is_empty());
    drop(pending);
    view.update(&arena, &world, &mut a, 5);
    assert_eq!(qa.try_recv().unwrap().body()[9], 96);
    arena.remove(id);
    let orb = arena
        .spawn(Entity::new(Kind::Orb(2), [0., 10., 0.]))
        .unwrap();
    view.index(&arena);
    view.update(&arena, &world, &mut a, 6);
    assert_eq!(
        qa.try_recv().unwrap().body(),
        &[0x13, 1, 128, 128, 128, 128, 4]
    );
    assert_eq!(qa.try_recv().unwrap().body()[0], 0x11);
    arena.remove(orb);
    view.index(&arena);
    view.update(&arena, &world, &mut a, 7);
    assert_eq!(qa.try_recv().unwrap().body()[0], 0x13);
    view.update(&arena, &world, &mut a, 8);
    assert!(qa.is_empty());
}

#[test]
fn warmed_visibility_allocates_nothing_on_the_simulation_thread() {
    let world = World::new("test".into(), Arc::new(Template::flat(1)));
    let mut arena = EntityArena::with_capacity(32);
    let mut view = EntityView::new(32);
    let (mut viewer, q) = player();
    for _ in 0..32 {
        let mut mob = Entity::new(Kind::Mob(54), [1., 10., 0.]);
        mob.velocity[0] = 0.03125;
        arena.spawn(mob).unwrap();
    }
    view.index(&arena);
    view.update(&arena, &world, &mut viewer, 1);
    assert_eq!(q.try_iter().count(), 32);
    let before = allocations::ALLOCATIONS.get();
    for tick in 2..202 {
        arena.tick();
        view.index(&arena);
        view.update(&arena, &world, &mut viewer, tick);
        assert_eq!(q.try_iter().count(), 32);
    }
    assert_eq!(allocations::ALLOCATIONS.get(), before);
}

#[test]
fn crowded_column_converges_across_bounded_batches() {
    let world = World::new("test".into(), Arc::new(Template::flat(1)));
    let mut arena = EntityArena::with_capacity(400);
    let mut view = EntityView::new(400);
    let (mut viewer, q) = player();
    for _ in 0..400 {
        let mut mob = Entity::new(Kind::Mob(54), [1., 10., 0.]);
        mob.velocity[0] = 0.03125;
        arena.spawn(mob).unwrap();
    }
    let mut spawns = 0;
    for tick in 0..24 {
        arena.tick();
        view.index(&arena);
        view.update(&arena, &world, &mut viewer, tick);
        spawns += q.try_iter().filter(|p| p.body()[0] == 0x0f).count();
    }
    assert_eq!(spawns, 400);
    assert!(!viewer.failed.load(std::sync::atomic::Ordering::Relaxed));
    viewer.loaded.clear();
    let mut destroys = 0;
    for tick in 24..30 {
        view.update(&arena, &world, &mut viewer, tick);
        destroys += q.try_iter().filter(|p| p.body()[0] == 0x13).count();
    }
    assert_eq!(destroys, 400);
}
