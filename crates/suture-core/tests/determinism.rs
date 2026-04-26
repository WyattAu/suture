use std::fs;
use suture_core::cas::hasher::hash_bytes;
use suture_core::engine::diff::{DiffType, diff_trees};
use suture_core::engine::tree::FileTree;
use suture_core::repository::Repository;

fn tmp_repo(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join(name);
    fs::create_dir_all(&repo).unwrap();
    let mut r = Repository::init(&repo, "alice").expect("init failed");
    r.set_config("user.name", "alice").unwrap();
    (tmp, repo)
}

#[test]
fn test_commit_determinism() {
    let (_t1, repo1) = tmp_repo("det-1");
    let (_t2, repo2) = tmp_repo("det-2");

    fs::write(repo1.join("file.txt"), "hello world\n").unwrap();
    let mut r1 = Repository::open(&repo1).unwrap();
    r1.add("file.txt").unwrap();
    let id1 = r1.commit("first commit").unwrap();

    fs::write(repo2.join("file.txt"), "hello world\n").unwrap();
    let mut r2 = Repository::open(&repo2).unwrap();
    r2.add("file.txt").unwrap();
    let id2 = r2.commit("first commit").unwrap();

    assert_eq!(
        id1, id2,
        "same content + message + author should produce same patch hash"
    );
}

#[test]
fn test_patch_id_determinism() {
    let (_t, repo) = tmp_repo("patch-det");

    fs::write(repo.join("a.txt"), "content\n").unwrap();
    let mut r = Repository::open(&repo).unwrap();
    r.add("a.txt").unwrap();
    let id = r.commit("add a").unwrap();

    let log = r.log(None).unwrap();
    let patch = log.iter().find(|p| p.id == id).expect("patch not found");
    assert_eq!(patch.id, id);
    assert_eq!(patch.message, "add a");
    assert_eq!(patch.author, "alice");
}

#[test]
fn test_merge_determinism() {
    fn setup_merge_repo(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let (tmp, repo) = tmp_repo(name);
        let mut r = Repository::open(&repo).unwrap();

        fs::write(repo.join("base.txt"), "base\n").unwrap();
        r.add("base.txt").unwrap();
        r.commit("base").unwrap();

        r.create_branch("branch-a", None).unwrap();
        r.create_branch("branch-b", None).unwrap();

        r.checkout("branch-a").unwrap();
        fs::write(repo.join("file-a.txt"), "only in a\n").unwrap();
        r.add("file-a.txt").unwrap();
        r.commit("add file-a").unwrap();

        r.checkout("branch-b").unwrap();
        fs::write(repo.join("file-b.txt"), "only in b\n").unwrap();
        r.add("file-b.txt").unwrap();
        r.commit("add file-b").unwrap();

        r.checkout("main").unwrap();
        (tmp, repo)
    }

    let (_t1, repo1) = setup_merge_repo("merge-ab");
    let mut r1 = Repository::open(&repo1).unwrap();

    let preview_a = r1.preview_merge("branch-a").expect("preview a");
    assert!(preview_a.is_clean, "merge branch-a should be clean");

    let result_a = r1.execute_merge("branch-a").expect("merge a");
    assert!(result_a.is_clean, "merge a should succeed cleanly");
    assert!(result_a.merged_tree.contains("file-a.txt"));
    assert!(result_a.merged_tree.contains("base.txt"));
    assert!(
        repo1.join("file-a.txt").exists(),
        "file-a.txt should be on disk after merge"
    );

    let (_t2, repo2) = setup_merge_repo("merge-ba");
    let mut r2 = Repository::open(&repo2).unwrap();

    let result_b = r2.execute_merge("branch-b").expect("merge b");
    assert!(result_b.is_clean, "merge b should succeed cleanly");
    assert!(result_b.merged_tree.contains("file-b.txt"));
    assert!(result_b.merged_tree.contains("base.txt"));
    assert!(
        repo2.join("file-b.txt").exists(),
        "file-b.txt should be on disk after merge"
    );

    assert_eq!(
        result_a.merged_tree.get("base.txt"),
        result_b.merged_tree.get("base.txt"),
        "base.txt hash should be identical in both merge orders"
    );

    assert_ne!(
        result_a.merged_tree.get("file-a.txt"),
        result_b.merged_tree.get("file-a.txt"),
        "repo1 should have file-a.txt, repo2 should not"
    );
    assert_ne!(
        result_a.merged_tree.get("file-b.txt"),
        result_b.merged_tree.get("file-b.txt"),
        "repo2 should have file-b.txt, repo1 should not"
    );
}

#[test]
fn test_push_pull_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_dir = tmp.path().join("roundtrip-repo");
    fs::create_dir_all(&repo_dir).unwrap();
    let mut r = Repository::init(&repo_dir, "alice").unwrap();
    r.set_config("user.name", "alice").unwrap();

    fs::write(repo_dir.join("hello.txt"), "hello\n").unwrap();
    r.add("hello.txt").unwrap();
    r.commit("initial").unwrap();

    let tree = r.snapshot_head().unwrap();
    assert!(tree.contains("hello.txt"));

    let blob_hash = tree.get("hello.txt").unwrap();
    let content = fs::read_to_string(repo_dir.join("hello.txt")).unwrap();
    let expected_hash = suture_core::Hash::from_data(content.as_bytes());
    assert_eq!(
        *blob_hash, expected_hash,
        "blob hash should match file content hash"
    );

    let clone_dir = tmp.path().join("roundtrip-clone");
    fs::create_dir_all(&clone_dir).unwrap();
    let mut r2 = Repository::init(&clone_dir, "alice").unwrap();
    r2.set_config("user.name", "alice").unwrap();

    fs::write(clone_dir.join("hello.txt"), "hello\n").unwrap();
    r2.add("hello.txt").unwrap();
    r2.commit("initial").unwrap();

    let tree2 = r2.snapshot_head().unwrap();
    assert_eq!(
        tree.get("hello.txt"),
        tree2.get("hello.txt"),
        "roundtrip should preserve file content hashes"
    );
}

#[test]
fn test_blake3_determinism() {
    let data = b"suture determinism test data";
    let h1 = hash_bytes(data);
    let h2 = hash_bytes(data);
    assert_eq!(h1, h2, "BLAKE3 must be deterministic");

    let h3 = hash_bytes(b"different data");
    assert_ne!(h1, h3, "different data must produce different hashes");

    let h4 = hash_bytes(data);
    assert_eq!(h1, h4, "BLAKE3 must be deterministic across multiple calls");
}

#[test]
fn test_diff_symmetry() {
    let mut tree_a = FileTree::empty();
    tree_a.insert(
        "keep.txt".to_string(),
        suture_core::Hash::from_data(b"same"),
    );
    tree_a.insert(
        "modified.txt".to_string(),
        suture_core::Hash::from_data(b"v1"),
    );
    tree_a.insert(
        "deleted.txt".to_string(),
        suture_core::Hash::from_data(b"gone"),
    );

    let mut tree_b = FileTree::empty();
    tree_b.insert(
        "keep.txt".to_string(),
        suture_core::Hash::from_data(b"same"),
    );
    tree_b.insert(
        "modified.txt".to_string(),
        suture_core::Hash::from_data(b"v2"),
    );
    tree_b.insert(
        "added.txt".to_string(),
        suture_core::Hash::from_data(b"new"),
    );

    let diffs_ab = diff_trees(&tree_a, &tree_b);
    let diffs_ba = diff_trees(&tree_b, &tree_a);

    for d_ab in &diffs_ab {
        match &d_ab.diff_type {
            DiffType::Added => {
                let inv = diffs_ba.iter().find(|d| d.path == d_ab.path);
                assert!(
                    inv.is_some(),
                    "Added in A->B at {} should have inverse in B->A",
                    d_ab.path
                );
                if let Some(d_ba) = inv {
                    assert!(
                        d_ba.diff_type == DiffType::Deleted
                            || matches!(d_ba.diff_type, DiffType::Renamed { .. }),
                        "Added in A->B should be Deleted/Renamed in B->A, got {:?}",
                        d_ba.diff_type
                    );
                }
            }
            DiffType::Deleted => {
                let inv = diffs_ba.iter().find(|d| d.path == d_ab.path);
                assert!(
                    inv.is_some(),
                    "Deleted in A->B at {} should have inverse in B->A",
                    d_ab.path
                );
                if let Some(d_ba) = inv {
                    assert!(
                        d_ba.diff_type == DiffType::Added
                            || matches!(d_ba.diff_type, DiffType::Renamed { .. }),
                        "Deleted in A->B should be Added/Renamed in B->A, got {:?}",
                        d_ba.diff_type
                    );
                }
            }
            DiffType::Modified => {
                let inv = diffs_ba.iter().find(|d| d.path == d_ab.path);
                assert!(
                    inv.is_some(),
                    "Modified in A->B at {} should have inverse in B->A",
                    d_ab.path
                );
                if let Some(d_ba) = inv {
                    assert_eq!(
                        d_ba.diff_type,
                        DiffType::Modified,
                        "Modified should be symmetric"
                    );
                }
            }
            DiffType::Renamed { .. } => {}
        }
    }
}

#[test]
fn test_branch_creation_idempotent() {
    let (_t, repo) = tmp_repo("branch-idem");
    let mut r = Repository::open(&repo).unwrap();

    fs::write(repo.join("file.txt"), "content\n").unwrap();
    r.add("file.txt").unwrap();
    r.commit("initial commit").unwrap();

    let (branch_before, head_before) = r.head().unwrap();

    r.create_branch("feature", None).unwrap();
    let branches_after_first = r.list_branches();
    assert!(
        branches_after_first.iter().any(|(n, _)| n == "feature"),
        "feature branch should exist after first create"
    );

    let result_second = r.create_branch("feature", None);
    assert!(
        result_second.is_err(),
        "creating the same branch twice should fail (idempotent — branch already exists)"
    );

    let branches_after_second = r.list_branches();
    let feature_count = branches_after_second
        .iter()
        .filter(|(n, _)| n == "feature")
        .count();
    assert_eq!(
        feature_count, 1,
        "feature branch should still exist exactly once"
    );

    let (branch_after, head_after) = r.head().unwrap();
    assert_eq!(branch_before, branch_after, "HEAD branch should not change");
    assert_eq!(head_before, head_after, "HEAD pointer should not change");
}
