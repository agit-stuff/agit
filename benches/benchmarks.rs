//! Benchmarks for AGIT.
//!
//! Run with: cargo bench
//!
//! Performance targets:
//! - Object store write: < 10ms per object
//! - Index append: < 5ms per entry
//! - Synthesize summary: < 1ms for 100 entries

use std::fs;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tempfile::TempDir;

use agit::core::SynthesizeSummary;
use agit::domain::{BlobContent, IndexEntry, WrappedBlob};
use agit::storage::{FileIndexStore, FileObjectStore, IndexStore, ObjectStore};

/// Setup a temporary AGIT directory for benchmarks.
fn setup_agit_dir() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let agit_dir = temp.path().join(".agit");
    fs::create_dir_all(agit_dir.join("objects")).unwrap();
    fs::write(agit_dir.join("index"), "").unwrap();
    (temp, agit_dir)
}

/// Benchmark object store write operations.
fn bench_object_store_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("object_store_write");

    for size in [100, 1000, 10000].iter() {
        let content = "x".repeat(*size);
        let blob = BlobContent::trace(&content);
        let wrapped = WrappedBlob::wrap(blob);
        let json = serde_json::to_vec(&wrapped).unwrap();

        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let (_temp, agit_dir) = setup_agit_dir();
            let store = FileObjectStore::new(&agit_dir);

            b.iter(|| store.save(black_box(&json)).unwrap());
        });
    }

    group.finish();
}

/// Benchmark object store read operations.
fn bench_object_store_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("object_store_read");

    for size in [100, 1000, 10000].iter() {
        let content = "x".repeat(*size);
        let blob = BlobContent::trace(&content);
        let wrapped = WrappedBlob::wrap(blob);
        let json = serde_json::to_vec(&wrapped).unwrap();

        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let (_temp, agit_dir) = setup_agit_dir();
            let store = FileObjectStore::new(&agit_dir);
            let hash = store.save(&json).unwrap();

            b.iter(|| store.load(black_box(&hash)).unwrap());
        });
    }

    group.finish();
}

/// Benchmark index append operations.
fn bench_index_append(c: &mut Criterion) {
    c.bench_function("index_append", |b| {
        let (_temp, agit_dir) = setup_agit_dir();
        let store = FileIndexStore::new(&agit_dir);
        let entry = IndexEntry::user_intent("Fix the authentication bug in the login flow");

        b.iter(|| store.append(black_box(&entry)).unwrap());
    });
}

/// Benchmark index read operations.
fn bench_index_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_read");

    for count in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let (_temp, agit_dir) = setup_agit_dir();
            let store = FileIndexStore::new(&agit_dir);

            // Pre-populate index
            for i in 0..count {
                let entry = IndexEntry::user_intent(&format!("Task number {}", i));
                store.append(&entry).unwrap();
            }

            b.iter(|| store.read_all().unwrap());
        });
    }

    group.finish();
}

/// Benchmark summary synthesis.
fn bench_synthesize_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("synthesize_summary");

    for count in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            // Create entries
            let mut entries = Vec::with_capacity(count);
            for i in 0..count {
                if i % 2 == 0 {
                    entries.push(IndexEntry::user_intent(&format!("Intent {}", i)));
                } else {
                    entries.push(IndexEntry::ai_reasoning(&format!("Reasoning {}", i)));
                }
            }

            b.iter(|| SynthesizeSummary::synthesize(black_box(&entries)));
        });
    }

    group.finish();
}

/// Benchmark trace formatting.
fn bench_format_trace(c: &mut Criterion) {
    let mut group = c.benchmark_group("format_trace");

    for count in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let mut entries = Vec::with_capacity(count);
            for i in 0..count {
                entries.push(IndexEntry::user_intent(&format!(
                    "Entry {} with some content",
                    i
                )));
            }

            b.iter(|| SynthesizeSummary::format_trace(black_box(&entries)));
        });
    }

    group.finish();
}

/// Benchmark SHA-256 hashing (used for content addressing).
fn bench_hash_computation(c: &mut Criterion) {
    use sha2::{Digest, Sha256};

    let mut group = c.benchmark_group("hash_computation");

    for size in [100, 1000, 10000, 100000].iter() {
        let data = vec![0u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut hasher = Sha256::new();
                hasher.update(black_box(&data));
                hasher.finalize()
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_object_store_write,
    bench_object_store_read,
    bench_index_append,
    bench_index_read,
    bench_synthesize_summary,
    bench_format_trace,
    bench_hash_computation,
);

criterion_main!(benches);
