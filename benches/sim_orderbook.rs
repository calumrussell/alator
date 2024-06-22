use criterion::{criterion_group, criterion_main, Criterion};

use rotala::exchange::uist_v1::{Order, UistV1};
use rotala::input::penelope::PenelopeBuilder;

fn uist_core_loop_test() {
    let mut source_builder = PenelopeBuilder::new();
    source_builder.add_quote(100.00, 101.00, 100, "ABC");
    source_builder.add_quote(10.00, 11.00, 100, "BCD");
    source_builder.add_quote(100.00, 101.00, 101, "ABC");
    source_builder.add_quote(10.00, 11.00, 101, "BCD");
    source_builder.add_quote(104.00, 105.00, 102, "ABC");
    source_builder.add_quote(10.00, 11.00, 102, "BCD");
    source_builder.add_quote(104.00, 105.00, 103, "ABC");
    source_builder.add_quote(12.00, 13.00, 103, "BCD");

    let mut uist = UistV1::from_penelope_builder(&mut source_builder, "Fake", rotala::clock::Frequency::Second);

    uist.insert_order(Order::market_buy("ABC", 100.0));
    uist.insert_order(Order::market_buy("ABC", 100.0));

    uist.tick();
    uist.tick();
    uist.tick();
}

fn benchmarks(c: &mut Criterion) {
    c.bench_function("uist core loop", |b| b.iter(uist_core_loop_test));
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
