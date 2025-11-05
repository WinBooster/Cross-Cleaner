use criterion::{black_box, criterion_group, criterion_main, Criterion};
use database::cleaner_database::get_default_database;
use database::structures::CleanerData;
use std::collections::HashSet;
use std::time::Duration;

// Benchmark HashSet vs Vec for category lookups
fn bench_category_lookup(c: &mut Criterion) {
    let categories = vec![
        "Cache".to_string(),
        "Logs".to_string(),
        "Crashes".to_string(),
        "Documentation".to_string(),
        "Backups".to_string(),
    ];

    let mut group = c.benchmark_group("category_lookup");

    // Benchmark Vec::contains
    group.bench_function("vec_contains", |b| {
        b.iter(|| {
            for _ in 0..100 {
                black_box(categories.contains(&"Cache".to_string()));
                black_box(categories.contains(&"Logs".to_string()));
                black_box(categories.contains(&"Unknown".to_string()));
            }
        });
    });

    // Benchmark HashSet::contains
    let categories_set: HashSet<String> = categories.iter().cloned().collect();
    group.bench_function("hashset_contains", |b| {
        b.iter(|| {
            for _ in 0..100 {
                black_box(categories_set.contains("Cache"));
                black_box(categories_set.contains("Logs"));
                black_box(categories_set.contains("Unknown"));
            }
        });
    });

    group.finish();
}

// Benchmark database filtering operations
fn bench_database_filtering(c: &mut Criterion) {
    let database = get_default_database();
    let categories: HashSet<String> = vec!["Cache".to_string(), "Logs".to_string()]
        .into_iter()
        .collect();
    let disabled_programs: HashSet<String> = HashSet::new();

    let mut group = c.benchmark_group("database_filtering");

    // Benchmark with HashSet (current optimized version)
    group.bench_function("hashset_filter", |b| {
        b.iter(|| {
            let filtered: Vec<&CleanerData> = database
                .iter()
                .filter(|data| {
                    categories.contains(&data.category)
                        && !disabled_programs.contains(&data.program)
                })
                .collect();
            black_box(filtered.len());
        });
    });

    // Benchmark with Vec (old slow version)
    let categories_vec: Vec<String> = categories.iter().cloned().collect();
    let disabled_vec: Vec<String> = disabled_programs.iter().cloned().collect();
    group.bench_function("vec_filter", |b| {
        b.iter(|| {
            let filtered: Vec<&CleanerData> = database
                .iter()
                .filter(|data| {
                    categories_vec.contains(&data.category) && !disabled_vec.contains(&data.program)
                })
                .collect();
            black_box(filtered.len());
        });
    });

    group.finish();
}

// Benchmark string operations (lowercase conversions)
fn bench_string_operations(c: &mut Criterion) {
    let programs = vec![
        "Firefox",
        "Chrome",
        "VSCode",
        "Discord",
        "Spotify",
        "Slack",
        "Telegram",
        "Steam",
        "Epic Games",
        "Minecraft",
    ];

    let mut group = c.benchmark_group("string_operations");

    // Benchmark repeated lowercase
    group.bench_function("repeated_lowercase", |b| {
        b.iter(|| {
            for _ in 0..100 {
                for program in &programs {
                    black_box(program.to_lowercase());
                }
            }
        });
    });

    // Benchmark pre-computed lowercase with HashSet
    group.bench_function("precomputed_lowercase", |b| {
        let programs_lower: HashSet<String> = programs.iter().map(|s| s.to_lowercase()).collect();

        b.iter(|| {
            for _ in 0..100 {
                for program in &programs {
                    black_box(programs_lower.contains(&program.to_lowercase()));
                }
            }
        });
    });

    group.finish();
}

// Benchmark collection pre-allocation
fn bench_collection_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("collection_allocation");

    // Without capacity
    group.bench_function("vec_without_capacity", |b| {
        b.iter(|| {
            let mut v = Vec::new();
            for i in 0..1000 {
                v.push(black_box(i));
            }
            black_box(v.len());
        });
    });

    // With capacity
    group.bench_function("vec_with_capacity", |b| {
        b.iter(|| {
            let mut v = Vec::with_capacity(1000);
            for i in 0..1000 {
                v.push(black_box(i));
            }
            black_box(v.len());
        });
    });

    group.finish();
}

// Benchmark atomic operations vs mutex
fn bench_concurrent_counting(c: &mut Criterion) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    let mut group = c.benchmark_group("concurrent_counting");

    // Atomic operations (fast)
    group.bench_function("atomic_relaxed", |b| {
        let counter = Arc::new(AtomicU64::new(0));
        b.iter(|| {
            for _ in 0..1000 {
                counter.fetch_add(black_box(1), Ordering::Relaxed);
            }
        });
    });

    // Mutex operations (slower)
    group.bench_function("mutex", |b| {
        let counter = Arc::new(Mutex::new(0u64));
        b.iter(|| {
            for _ in 0..1000 {
                let mut c = counter.lock().unwrap();
                *c += black_box(1);
            }
        });
    });

    group.finish();
}

// Benchmark database loading
fn bench_database_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("database_loading");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("load_default_database", |b| {
        b.iter(|| {
            let db = get_default_database();
            black_box(db.len());
        });
    });

    group.finish();
}

// Benchmark sorting operations
fn bench_sorting(c: &mut Criterion) {
    let data: Vec<String> = (0..1000).map(|i| format!("Program_{}", i % 100)).collect();

    let mut group = c.benchmark_group("sorting");

    // Stable sort
    group.bench_function("stable_sort", |b| {
        b.iter(|| {
            let mut d = data.clone();
            d.sort();
            black_box(d.len());
        });
    });

    // Unstable sort (faster)
    group.bench_function("unstable_sort", |b| {
        b.iter(|| {
            let mut d = data.clone();
            d.sort_unstable();
            black_box(d.len());
        });
    });

    group.finish();
}

// Benchmark string cloning vs moving
fn bench_ownership(c: &mut Criterion) {
    let strings: Vec<String> = (0..100).map(|i| format!("String_{}", i)).collect();

    let mut group = c.benchmark_group("ownership");

    // Cloning (slower)
    group.bench_function("clone_strings", |b| {
        b.iter(|| {
            let mut result = Vec::new();
            for s in &strings {
                result.push(black_box(s.clone()));
            }
            black_box(result.len());
        });
    });

    // Moving (faster when possible)
    group.bench_function("move_strings", |b| {
        b.iter(|| {
            let local_strings = strings.clone(); // Setup
            let mut result = Vec::new();
            for s in local_strings {
                result.push(black_box(s));
            }
            black_box(result.len());
        });
    });

    group.finish();
}

// Benchmark program deduplication strategies
fn bench_deduplication(c: &mut Criterion) {
    let database = get_default_database();

    let mut group = c.benchmark_group("deduplication");

    // Using HashSet (fastest)
    group.bench_function("hashset_dedup", |b| {
        b.iter(|| {
            let programs: HashSet<&str> =
                database.iter().map(|data| data.program.as_str()).collect();
            black_box(programs.len());
        });
    });

    // Using Vec + dedup (slower)
    group.bench_function("vec_dedup", |b| {
        b.iter(|| {
            let mut programs: Vec<&str> =
                database.iter().map(|data| data.program.as_str()).collect();
            programs.sort_unstable();
            programs.dedup();
            black_box(programs.len());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_category_lookup,
    bench_database_filtering,
    bench_string_operations,
    bench_collection_allocation,
    bench_concurrent_counting,
    bench_database_loading,
    bench_sorting,
    bench_ownership,
    bench_deduplication
);

criterion_main!(benches);
