use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use suture_core::cas::store::BlobStore;
use suture_core::patch::types::{OperationType, Patch, TouchSet};
use tempfile::TempDir;

fn bench_cas_store_100_blobs(c: &mut Criterion) {
    let mut group = c.benchmark_group("cas_perf_store");

    let sizes: Vec<(String, usize)> = vec![
        ("1KB".to_string(), 1024),
        ("10KB".to_string(), 10_240),
        ("100KB".to_string(), 102_400),
        ("1MB".to_string(), 1_048_576),
    ];

    for (label, size) in &sizes {
        let data = vec![42u8; *size];
        group.bench_with_input(BenchmarkId::new("store_100_blobs", label), size, |b, _| {
            b.iter_with_setup(
                || {
                    let dir = TempDir::new().unwrap();
                    let cas = BlobStore::new_uncompressed(dir.path()).unwrap();
                    (dir, cas)
                },
                |(dir, cas)| {
                    for i in 0..100 {
                        let mut blob = data.clone();
                        blob[0] = (i % 256) as u8;
                        black_box(cas.put_blob(&blob).unwrap());
                    }
                    drop(dir);
                },
            );
        });
    }

    group.finish();
}

fn bench_cas_store_lookup_1000(c: &mut Criterion) {
    let mut group = c.benchmark_group("cas_perf_lookup");

    group.bench_function("store_1000_lookup_100", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let cas = BlobStore::new_uncompressed(dir.path()).unwrap();
                let mut hashes = Vec::with_capacity(1000);
                for i in 0..1000 {
                    let data = format!("blob content number {} with some padding", i);
                    let hash = cas.put_blob(data.as_bytes()).unwrap();
                    hashes.push(hash);
                }
                (dir, cas, hashes)
            },
            |(dir, cas, hashes)| {
                for i in 0..100 {
                    let idx = (i * 7) % 1000;
                    let blob = black_box(cas.get_blob(&hashes[idx]).unwrap());
                    assert!(!blob.is_empty());
                }
                drop(dir);
            },
        );
    });

    group.finish();
}

fn bench_patch_serialize_100(c: &mut Criterion) {
    let mut group = c.benchmark_group("patch_perf_serialize");

    let patches: Vec<Patch> = (0..100)
        .map(|i| {
            Patch::new(
                OperationType::Modify,
                TouchSet::single(format!("addr_{}", i)),
                Some(format!("file_{}", i)),
                vec![],
                vec![],
                "bench".to_string(),
                format!("patch {}", i),
            )
        })
        .collect();

    group.bench_function("serialize_100_patches", |b| {
        b.iter(|| {
            for patch in &patches {
                let json = black_box(serde_json::to_string(patch).unwrap());
                black_box(json);
            }
        });
    });

    group.finish();
}

fn bench_patch_deserialize_100(c: &mut Criterion) {
    let mut group = c.benchmark_group("patch_perf_deserialize");

    let serialized: Vec<String> = (0..100)
        .map(|i| {
            let patch = Patch::new(
                OperationType::Modify,
                TouchSet::single(format!("addr_{}", i)),
                Some(format!("file_{}", i)),
                vec![],
                vec![],
                "bench".to_string(),
                format!("patch {}", i),
            );
            serde_json::to_string(&patch).unwrap()
        })
        .collect();

    group.bench_function("deserialize_100_patches", |b| {
        b.iter(|| {
            for s in &serialized {
                let patch: Patch = black_box(serde_json::from_str(s).unwrap());
                black_box(patch);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cas_store_100_blobs,
    bench_cas_store_lookup_1000,
    bench_patch_serialize_100,
    bench_patch_deserialize_100,
);
criterion_main!(benches);
