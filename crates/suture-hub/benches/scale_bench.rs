use criterion::{Criterion, criterion_group, criterion_main};
use suture_hub::storage::HubStorage;
use suture_protocol::{HashProto, PatchProto};

fn make_hash_proto(hex: &str) -> HashProto {
    HashProto {
        value: hex.to_string(),
    }
}

fn make_patch(id_hex: &str, parent_hex: Option<&str>, author: &str) -> PatchProto {
    PatchProto {
        id: make_hash_proto(id_hex),
        operation_type: "Create".to_string(),
        touch_set: vec![format!("file_{id_hex}")],
        target_path: Some(format!("file_{id_hex}")),
        payload: String::new(),
        parent_ids: parent_hex
            .map(|p| vec![make_hash_proto(p)])
            .unwrap_or_default(),
        author: author.to_string(),
        message: format!("patch {id_hex}"),
        timestamp: 0,
    }
}

fn bench_create_many_repos(c: &mut Criterion) {
    c.bench_function("create_1000_repos", |b| {
        b.iter(|| {
            let store = HubStorage::open_in_memory().unwrap();
            for i in 0..1000u32 {
                let repo_id = format!("bench-repo-{i}");
                store.ensure_repo(&repo_id).unwrap();
            }
        });
    });
}

fn bench_push_many_patches(c: &mut Criterion) {
    let store = HubStorage::open_in_memory().unwrap();
    store.ensure_repo("bench-repo").unwrap();

    let patches: Vec<PatchProto> = (0..100)
        .map(|i| {
            let hex = format!("{i:064x}");
            if i > 0 {
                let p = format!("{:064x}", i - 1);
                make_patch(&hex, Some(&p), "alice")
            } else {
                make_patch(&hex, None, "alice")
            }
        })
        .collect();

    let last_hex = format!("{:064x}", 99);

    c.bench_function("push_100_patches", |b| {
        b.iter(|| {
            for patch in &patches {
                let _ = store.insert_patch("bench-repo", patch).unwrap();
            }
            store.set_branch("bench-repo", "main", &last_hex).unwrap();
        });
    });
}

fn bench_store_many_blobs(c: &mut Criterion) {
    c.bench_function("store_100_blobs_varied_size", |b| {
        b.iter(|| {
            let store = HubStorage::open_in_memory().unwrap();
            store.ensure_repo("blob-bench-repo").unwrap();
            for i in 0..100usize {
                let hash_hex = format!("{i:064x}");
                let size = 64 + i * 1024;
                let data = vec![i as u8; size];
                store
                    .store_blob("blob-bench-repo", &hash_hex, &data)
                    .unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_create_many_repos,
    bench_push_many_patches,
    bench_store_many_blobs,
);
criterion_main!(benches);
