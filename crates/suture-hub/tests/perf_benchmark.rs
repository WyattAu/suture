//! Performance benchmark for Suture Hub.
//! Tests repository operations at scale.

use std::time::Instant;

fn create_test_hub() -> suture_hub::storage::HubStorage {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    suture_hub::storage::HubStorage::open(&db_path).unwrap()
}

fn make_hash_proto(hex: &str) -> suture_hub::types::HashProto {
    suture_hub::types::HashProto {
        value: hex.to_string(),
    }
}

fn make_patch(
    id_hex: &str,
    op: &str,
    parents: &[&str],
    author: &str,
    path: &str,
) -> suture_hub::types::PatchProto {
    suture_hub::types::PatchProto {
        id: make_hash_proto(id_hex),
        operation_type: op.to_string(),
        touch_set: vec![path.to_string()],
        target_path: Some(path.to_string()),
        payload: String::new(),
        parent_ids: parents.iter().map(|p| make_hash_proto(p)).collect(),
        author: author.to_string(),
        message: format!("patch {}", id_hex),
        timestamp: 0,
    }
}

#[test]
fn test_benchmark_1000_patches() {
    let storage = create_test_hub();
    storage.ensure_repo("bench-repo").unwrap();

    let start = Instant::now();
    for i in 0..1000 {
        let hex = format!("{:064x}", i);
        let patch = make_patch(&hex, "Modify", &[], "author", &format!("file-{}", i % 100));
        storage.insert_patch("bench-repo", &patch).unwrap();
    }
    let elapsed = start.elapsed();

    println!("1000 patches inserted in {:?}", elapsed);
    println!("Per-patch: {:?}", elapsed / 1000);

    assert!(elapsed.as_secs() < 5, "1000 patches took {:?}", elapsed);
}

#[test]
fn test_benchmark_blob_storage() {
    let storage = create_test_hub();
    storage.ensure_repo("bench-blobs").unwrap();

    let data = vec![0xAB; 1024];
    let compressed = suture_protocol::compress(&data).unwrap();

    let start = Instant::now();
    for i in 0..500 {
        let hash = format!("{:064x}", i);
        storage
            .store_blob("bench-blobs", &hash, &compressed)
            .unwrap();
    }
    let elapsed = start.elapsed();

    println!("500 1KB blobs stored in {:?}", elapsed);
    println!("Per-blob: {:?}", elapsed / 500);

    assert!(elapsed.as_secs() < 5, "500 blobs took {:?}", elapsed);
}

#[test]
fn test_benchmark_branch_listing() {
    let storage = create_test_hub();
    storage.ensure_repo("bench-branches").unwrap();

    for i in 0..100 {
        let hash = format!("{:064x}", i);
        storage
            .set_branch("bench-branches", &format!("branch-{}", i), &hash)
            .unwrap();
    }

    let start = Instant::now();
    for _ in 0..1000 {
        let branches = storage.get_branches("bench-branches").unwrap();
        assert_eq!(branches.len(), 100);
    }
    let elapsed = start.elapsed();

    println!("1000x listing 100 branches in {:?}", elapsed);
    assert!(elapsed.as_secs() < 3, "Branch listing took {:?}", elapsed);
}

#[test]
fn test_benchmark_issue_creation() {
    let storage = create_test_hub();
    storage.ensure_repo("bench-issues").unwrap();

    let start = Instant::now();
    for i in 0..500 {
        let labels = if i % 10 == 0 {
            vec!["bug".to_string()]
        } else {
            vec![]
        };
        storage
            .create_issue(
                "bench-issues",
                &format!("Issue #{}", i),
                "body",
                "author",
                &labels,
            )
            .unwrap();
    }
    let elapsed = start.elapsed();

    println!("500 issues created in {:?}", elapsed);
    assert!(elapsed.as_secs() < 5, "500 issues took {:?}", elapsed);
}
