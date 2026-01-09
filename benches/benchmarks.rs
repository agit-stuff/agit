//! Benchmarks for AGIT.
//!
//! Run with: cargo bench

use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_placeholder(c: &mut Criterion) {
    c.bench_function("placeholder", |b| {
        b.iter(|| {
            // Placeholder benchmark
            let x: u64 = (0..100).sum();
            x
        });
    });
}

criterion_group!(benches, benchmark_placeholder);
criterion_main!(benches);
