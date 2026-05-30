use std::fs;
use std::time::Instant;
use suture_core::repository::Repository;
use tempfile::TempDir;

fn create_repo_with_files(file_count: usize) -> (TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = Repository::init(dir.path(), "bench-user").unwrap();

    let batch_size = 1000;
    let mut created = 0;

    while created < file_count {
        let batch_end = (created + batch_size).min(file_count);
        for i in created..batch_end {
            let dir_name = format!("dir_{}", i / 100);
            let file_dir = dir.path().join(&dir_name);
            fs::create_dir_all(&file_dir).unwrap();
            let file_name = format!("file_{:06}.txt", i);
            fs::write(file_dir.join(&file_name), format!("content of file {}", i)).unwrap();
        }

        for i in created..batch_end {
            let dir_name = format!("dir_{}", i / 100);
            let file_name = format!("{}/file_{:06}.txt", dir_name, i);
            repo.add(&file_name).unwrap();
        }
        repo.commit(&format!("batch {}-{}", created, batch_end - 1))
            .unwrap();
        created = batch_end;
    }

    (dir, repo)
}

#[test]
fn test_scale_10k_files_commit() {
    let start = Instant::now();
    let (_dir, repo) = create_repo_with_files(10_000);
    let elapsed = start.elapsed();

    println!("10K files committed in {:?}", elapsed);

    let (branch, _id) = repo.head().unwrap();
    assert_eq!(branch, "main");

    // Windows CI runners are significantly slower; allow 300s there.
    // Local machines should complete well under 60s.
    let max_secs: u64 = if cfg!(target_os = "windows") { 300 } else { 60 };
    assert!(elapsed.as_secs() < max_secs, "10K files took {:?}", elapsed);
}

#[test]
fn test_scale_10k_files_status() {
    let (_dir, repo) = create_repo_with_files(10_000);

    let start = Instant::now();
    let status = repo.status().unwrap();
    let elapsed = start.elapsed();

    println!("10K files status in {:?}", elapsed);
    assert!(status.staged_files.is_empty());
    assert!(elapsed.as_secs() < 5, "10K status took {:?}", elapsed);
}

#[test]
fn test_scale_10k_files_diff() {
    let (dir, repo) = create_repo_with_files(10_000);

    fs::write(dir.path().join("dir_5/file_000500.txt"), "modified content").unwrap();
    let mut repo = repo;
    repo.add("dir_5/file_000500.txt").unwrap();
    repo.commit("modify one file").unwrap();

    let start = Instant::now();
    let patches = repo.all_patches();
    let elapsed = start.elapsed();

    println!("10K files diff query in {:?}", elapsed);
    assert!(!patches.is_empty());
    assert!(elapsed.as_secs() < 5, "10K diff took {:?}", elapsed);
}

#[test]
fn test_scale_10k_files_log() {
    let (_dir, repo) = create_repo_with_files(10_000);

    let start = Instant::now();
    let log = repo.log(None).unwrap();
    let elapsed = start.elapsed();

    println!("10K files log in {:?}", elapsed);
    assert!(log.len() >= 10);
    assert!(elapsed.as_secs() < 5, "10K log took {:?}", elapsed);
}
