use bytes::BytesMut;
use coral_engine::{arena::Template, wire::{Packet,WriteWire}};
use criterion::{Criterion, criterion_group, criterion_main};
use std::{hint::black_box,sync::Arc};

fn measure(c:&mut Criterion){
    let packets:Vec<_>=(0..64).map(|i|Packet::new(0,|w|w.var(i))).collect();
    let mut dst=BytesMut::with_capacity(4096);
    for p in &packets{p.encoded(256).unwrap();}
    c.bench_function("batch/64_cached_keepalives",|b|b.iter(||{
        dst.clear();for p in &packets{dst.extend_from_slice(&p.encoded(256).unwrap());}black_box(&dst);
    }));
    let template=Arc::new(Template::flat(4));
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("arena.car");
    template.save(&path).unwrap();
    c.bench_function("arena/read_zstd_81_columns",|b|b.iter(||black_box(Template::load(&path).unwrap())));
    c.bench_function("arena/instantiate_warm_template",|b|b.iter(||black_box(coral_engine::arena::World::new("match".into(),template.clone()))));
}
criterion_group!(benches,measure);
criterion_main!(benches);
