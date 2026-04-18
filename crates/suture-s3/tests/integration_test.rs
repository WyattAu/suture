//! Integration tests for suture-s3 against MinIO or any S3-compatible endpoint.
//!
//! To run these tests:
//! 1. Start MinIO: `docker run -p 9000:9000 minio/minio server /data --console-address ":9001"`
//! 2. Set environment variables:
//!    export S3_TEST_ENDPOINT=http://localhost:9000
//!    export S3_TEST_ACCESS_KEY=minioadmin
//!    export S3_TEST_SECRET_KEY=minioadmin
//!    export S3_TEST_BUCKET=suture-test
//! 3. Run: cargo test -p suture-s3 --test integration_test

use suture_common::Hash;
use suture_s3::{S3BlobStore, S3Config};

fn init() {
    let _ = env_logger::try_init();
}

fn test_config() -> Option<S3Config> {
    let endpoint = std::env::var("S3_TEST_ENDPOINT").ok()?;
    let access_key = std::env::var("S3_TEST_ACCESS_KEY").ok()?;
    let secret_key = std::env::var("S3_TEST_SECRET_KEY").ok()?;
    let bucket = std::env::var("S3_TEST_BUCKET").ok()?;

    Some(S3Config {
        endpoint,
        bucket,
        region: "us-east-1".to_string(),
        access_key,
        secret_key,
        prefix: "suture-test/".to_string(),
        force_path_style: true,
    })
}

fn test_store() -> Option<S3BlobStore> {
    test_config().map(S3BlobStore::new)
}

fn make_hash(bytes: [u8; 32]) -> Hash {
    Hash::from(bytes)
}

async fn cleanup_prefix(store: &S3BlobStore) {
    if let Ok(blobs) = store.list_blobs().await {
        for hash in blobs {
            let _ = store.delete_blob(&hash).await;
        }
    }
}

#[tokio::test]
async fn test_put_and_get_blob() {
    let store = match test_store() {
        Some(s) => s,
        None => return,
    };

    cleanup_prefix(&store).await;

    let data = b"hello s3 integration test";
    let hash = Hash::from_data(data);

    store.put_blob(&hash, data).await.expect("put_blob failed");
    let retrieved = store.get_blob(&hash).await.expect("get_blob failed");

    assert_eq!(retrieved, data);
}

#[tokio::test]
async fn test_has_blob() {
    let store = match test_store() {
        Some(s) => s,
        None => return,
    };

    cleanup_prefix(&store).await;

    let data = b"check existence";
    let hash = Hash::from_data(data);
    let missing_hash = make_hash([0xFF; 32]);

    store.put_blob(&hash, data).await.expect("put_blob failed");

    assert!(store.has_blob(&hash).await.expect("has_blob failed"));
    assert!(!store.has_blob(&missing_hash).await.expect("has_blob (missing) failed"));
}

#[tokio::test]
async fn test_delete_blob() {
    let store = match test_store() {
        Some(s) => s,
        None => return,
    };

    cleanup_prefix(&store).await;

    let data = b"delete me";
    let hash = Hash::from_data(data);

    store.put_blob(&hash, data).await.expect("put_blob failed");
    assert!(store.has_blob(&hash).await.expect("has_blob before delete failed"));

    store.delete_blob(&hash).await.expect("delete_blob failed");
    assert!(!store.has_blob(&hash).await.expect("has_blob after delete failed"));
}

#[tokio::test]
async fn test_list_blobs() {
    let store = match test_store() {
        Some(s) => s,
        None => return,
    };

    cleanup_prefix(&store).await;

    let data1 = b"first blob";
    let data2 = b"second blob";
    let data3 = b"third blob";
    let hash1 = Hash::from_data(data1);
    let hash2 = Hash::from_data(data2);
    let hash3 = Hash::from_data(data3);

    store.put_blob(&hash1, data1).await.expect("put 1 failed");
    store.put_blob(&hash2, data2).await.expect("put 2 failed");
    store.put_blob(&hash3, data3).await.expect("put 3 failed");

    let mut blobs = store.list_blobs().await.expect("list_blobs failed");
    blobs.sort_by(|a, b| a.cmp(b));

    let mut expected = vec![hash1, hash2, hash3];
    expected.sort_by(|a, b| a.cmp(b));

    assert_eq!(blobs, expected);
}

#[tokio::test]
async fn test_overwrite_blob() {
    let store = match test_store() {
        Some(s) => s,
        None => return,
    };

    cleanup_prefix(&store).await;

    let data_a = b"version one";
    let data_b = b"version two";
    let hash = Hash::from_data(data_a);

    store.put_blob(&hash, data_a).await.expect("put first version failed");
    store.put_blob(&hash, data_b).await.expect("put second version failed");

    let retrieved = store.get_blob(&hash).await.expect("get_blob failed");
    assert_eq!(retrieved, data_b, "S3 PUT overwrites, so second version should be returned");
}

#[tokio::test]
async fn test_empty_bucket_list() {
    let store = match test_store() {
        Some(s) => s,
        None => return,
    };

    cleanup_prefix(&store).await;

    let blobs = store.list_blobs().await.expect("list_blobs failed");
    assert!(blobs.is_empty());
}

#[tokio::test]
async fn test_nonexistent_bucket() {
    let config = match test_config() {
        Some(c) => c,
        None => return,
    };

    let mut bad_config = config;
    bad_config.bucket = "nonexistent-bucket-xyzzy".to_string();
    let store = S3BlobStore::new(bad_config);

    let hash = make_hash([0xAB; 32]);
    let result = store.get_blob(&hash).await;

    assert!(result.is_err(), "expected error for nonexistent bucket");
}
