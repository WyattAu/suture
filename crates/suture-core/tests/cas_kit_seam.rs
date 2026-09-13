//! Integration tests for the `suture_core::cas` facade over the `cas-kit`
//! estate crate (`cas_kit::BlobStore`).
//!
//! These lock the seam contract suture's storage layer depends on:
//! content-addressed round-trips, deduplication, CAS-on-write enforcement,
//! and delete/count/list behavior across the facade's `suture_common::Hash`
//! conversion boundary.

use suture_core::cas::store::BlobStore;

fn store_in(tmp: &tempfile::TempDir) -> BlobStore {
    BlobStore::new(tmp.path()).expect("store creation")
}

#[test]
fn put_then_get_round_trips_content() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = store_in(&tmp);
    let payload = "suture blob payload 🐚".as_bytes().to_vec();

    let hash = store.put_blob(&payload).expect("put");
    assert!(store.has_blob(&hash));
    assert_eq!(store.get_blob(&hash).expect("get"), payload);
}

#[test]
fn identical_content_deduplicates_to_one_blob() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = store_in(&tmp);
    let payload = b"duplicate me".to_vec();

    let first = store.put_blob(&payload).expect("first put");
    let second = store.put_blob(&payload).expect("second put");
    assert_eq!(first, second, "same content => same address");
    assert_eq!(store.blob_count().expect("count"), 1);
}

#[test]
fn put_blob_new_rejects_existing_content() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = store_in(&tmp);
    let payload = b"unique once".to_vec();

    store.put_blob(&payload).expect("first put");
    assert!(
        store.put_blob_new(&payload).is_err(),
        "second put_blob_new with identical content must fail"
    );
}

#[test]
fn put_blob_with_hash_enforces_address_match() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = store_in(&tmp);
    let payload = b"address must match";
    let wrong_address = suture_common::Hash([0u8; 32]);

    assert!(store.put_blob_with_hash(payload, &wrong_address).is_err());
    assert_eq!(
        store.blob_count().expect("count"),
        0,
        "mismatched write must not persist"
    );
}

#[test]
fn verify_on_read_flags_survive_toggle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut store = store_in(&tmp);
    assert!(store.verify_on_read(), "verification defaults on");

    store.set_verify_on_read(false);
    assert!(!store.verify_on_read());

    let hash = store.put_blob(b"unverified read").expect("put");
    assert_eq!(store.get_blob(&hash).expect("get"), b"unverified read");
}

#[test]
fn delete_removes_blob_only_at_that_address() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = store_in(&tmp);

    let gone = store.put_blob(b"to be deleted").expect("put");
    let kept = store.put_blob(b"to be kept").expect("put");

    store.delete_blob(&gone).expect("delete");
    assert!(!store.has_blob(&gone));
    assert!(store.has_blob(&kept));

    let listed = store.list_blobs().expect("list");
    assert_eq!(listed.len(), 1);
}

#[test]
fn uncompressed_store_round_trips_too() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BlobStore::new_uncompressed(tmp.path()).expect("uncompressed store");
    let payload = b"raw bytes, no zstd".to_vec();

    let hash = store.put_blob(&payload).expect("put");
    assert_eq!(store.get_blob(&hash).expect("get"), payload);
    assert!(store.total_size().expect("size") > 0);
}
