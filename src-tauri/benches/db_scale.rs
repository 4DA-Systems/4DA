//! Database scale benchmarks for 4DA
//!
//! Measures insert and query performance at various scales.
//! Run with: `cargo bench --bench db_scale`

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use fourda_lib::test_utils::{seed_embedding, test_db};

/// Pre-populate a database with N items and return it
fn populated_db(n: usize) -> fourda_lib::db::Database {
    let db = test_db();
    for i in 0..n {
        let emb = seed_embedding(&format!("bench-{}", i));
        db.upsert_source_item(
            "hackernews",
            &format!("hn_{}", i),
            Some(&format!("https://example.com/{}", i)),
            &format!("Benchmark Item {}: Rust performance", i),
            &format!(
                "Content about topic {} for benchmarking database operations",
                i
            ),
            &emb,
        )
        .expect("upsert");
    }
    db
}

fn bench_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_insert");
    group.sample_size(10); // Fewer samples for expensive benchmarks

    for count in [100, 1000, 5000] {
        group.bench_with_input(BenchmarkId::new("items", count), &count, |b, &n| {
            b.iter(|| {
                register_vec();
                let db =
                    fourda_lib::db::Database::new(Path::new(":memory:")).expect("in-memory DB");
                for i in 0..n {
                    let emb = seed_embedding(&format!("ins-{}", i));
                    db.upsert_source_item(
                        "hackernews",
                        &format!("hn_{}", i),
                        None,
                        &format!("Item {}", i),
                        "Content",
                        &emb,
                    )
                    .expect("upsert");
                }
            });
        });
    }

    group.finish();
}

fn bench_knn_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_knn_query");

    for db_size in [100, 1000, 5000] {
        let db = populated_db(db_size);
        let query_emb = seed_embedding("query-vector");

        group.bench_with_input(BenchmarkId::new("k5_from", db_size), &db_size, |b, _| {
            b.iter(|| {
                db.find_similar_source_items(&query_emb, 5)
                    .expect("knn query")
            });
        });
    }

    group.finish();
}

fn bench_upsert_existing(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_upsert_existing");
    group.sample_size(20);

    let db = populated_db(1000);
    let emb = seed_embedding("update-emb");

    group.bench_function("update_1000_existing", |b| {
        b.iter(|| {
            // Re-upsert existing items (triggers UPDATE path)
            for i in 0..100 {
                db.upsert_source_item(
                    "hackernews",
                    &format!("hn_{}", i),
                    None,
                    &format!("Updated Item {}", i),
                    "Updated content",
                    &emb,
                )
                .expect("upsert");
            }
        });
    });

    group.finish();
}

fn bench_stats_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_stats");

    for db_size in [100, 1000, 5000] {
        let db = populated_db(db_size);

        group.bench_with_input(
            BenchmarkId::new("get_stats_from", db_size),
            &db_size,
            |b, _| {
                b.iter(|| db.get_db_stats().expect("stats"));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_inserts,
    bench_knn_query,
    bench_upsert_existing,
    bench_stats_query
);
criterion_main!(benches);
