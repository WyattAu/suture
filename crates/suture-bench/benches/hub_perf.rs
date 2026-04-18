use criterion::{black_box, criterion_group, criterion_main, Criterion};
use suture_common::Hash;
use suture_hub::HubStorage;
use suture_protocol::{HashProto, PatchProto};

fn make_hub_patch(i: usize, parent_hex: Option<&str>) -> PatchProto {
    let id_hex = Hash::from_data(format!("hub_patch_{}", i).as_bytes()).to_hex();
    PatchProto {
        id: HashProto { value: id_hex },
        operation_type: "Create".to_string(),
        touch_set: vec![format!("file_{}", i)],
        target_path: Some(format!("file_{}", i)),
        payload: String::new(),
        parent_ids: parent_hex
            .map(|h| {
                vec![HashProto {
                    value: h.to_string(),
                }]
            })
            .unwrap_or_default(),
        author: "bench".to_string(),
        message: format!("patch {}", i),
        timestamp: i as u64,
    }
}

fn insert_50_patches(store: &HubStorage) {
    store.ensure_repo("bench-repo").unwrap();
    for i in 0..50 {
        let parent = if i > 0 {
            Some(Hash::from_data(format!("hub_patch_{}", i - 1).as_bytes()).to_hex())
        } else {
            None
        };
        let patch = make_hub_patch(i, parent.as_deref());
        store.insert_patch("bench-repo", &patch).unwrap();
    }
}

fn bench_hub_create_repo_100(c: &mut Criterion) {
    let mut group = c.benchmark_group("hub_perf_repo");

    group.bench_function("create_100_repos", |b| {
        b.iter_with_setup(
            || HubStorage::open_in_memory().unwrap(),
            |store| {
                for i in 0..100 {
                    let name = format!("bench-repo-{}", i);
                    black_box(store.ensure_repo(&name).unwrap());
                }
            },
        );
    });

    group.finish();
}

fn bench_hub_push_pull_50_patches(c: &mut Criterion) {
    let mut group = c.benchmark_group("hub_perf_push_pull");

    group.bench_function("push_50_patches", |b| {
        b.iter_with_setup(
            || HubStorage::open_in_memory().unwrap(),
            |store| {
                insert_50_patches(&store);
            },
        );
    });

    group.bench_function("pull_50_patches", |b| {
        b.iter_with_setup(
            || {
                let store = HubStorage::open_in_memory().unwrap();
                insert_50_patches(&store);
                store
            },
            |store| {
                let patches = black_box(store.get_all_patches("bench-repo").unwrap());
                assert_eq!(patches.len(), 50);
            },
        );
    });

    group.bench_function("push_pull_roundtrip_50", |b| {
        b.iter_with_setup(
            || HubStorage::open_in_memory().unwrap(),
            |store| {
                insert_50_patches(&store);
                let patches = black_box(store.get_all_patches("bench-repo").unwrap());
                assert_eq!(patches.len(), 50);
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_hub_create_repo_100,
    bench_hub_push_pull_50_patches,
);
criterion_main!(benches);
