use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> std::path::PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cli_bin() -> std::path::PathBuf {
    workspace_root()
        .join("target")
        .join("debug")
        .join("suture-cli")
}

fn suture(dir: &Path, args: &[&str]) -> std::process::Output {
    let bin = cli_bin();
    if !bin.exists() {
        eprintln!(
            "Skipping: {} not found (run `cargo build` first)",
            bin.display()
        );
        std::process::exit(0);
    }
    Command::new(&bin)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to execute suture-cli")
}

fn suture_success(dir: &Path, args: &[&str]) -> String {
    let output = suture(dir, args);
    if !output.status.success() {
        panic!(
            "suture {:?} failed:\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn main() {
    println!("Running Suture end-to-end integration tests...");

    if !cli_bin().exists() {
        println!("SKIP: suture-cli not found (run `cargo build` first)");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("test-repo");
    fs::create_dir_all(&repo).unwrap();

    test_init_commit_status(&repo);
    test_branch_merge(&repo);
    test_gc(&repo);
    test_fsck(&repo);
    test_bisect(&repo);
    test_tag(&repo);
    test_stash(&repo);

    println!("\nAll integration tests passed!");
}

fn test_init_commit_status(repo: &Path) {
    println!("\n=== test_init_commit_status ===");

    let out = suture_success(repo, &["init", "."]);
    assert!(
        out.contains("Initialized empty Suture repository"),
        "init output: {}",
        out
    );

    suture_success(repo, &["config", "user.name=Alice"]);

    let out = suture_success(repo, &["status"]);
    assert!(out.contains("On branch main"), "status after init: {}", out);
    assert!(
        !out.contains("Unstaged changes:"),
        "status should be clean after init: {}",
        out
    );

    fs::write(repo.join("hello.txt"), "Hello, World!\n").unwrap();
    suture_success(repo, &["add", "hello.txt"]);
    let out = suture_success(repo, &["commit", "Initial file"]);
    assert!(out.contains("Committed:"), "commit output: {}", out);

    let out = suture_success(repo, &["status"]);
    assert!(
        !out.contains("Unstaged changes:") && !out.contains("Staged changes:"),
        "status should be clean after commit: {}",
        out
    );

    let out = suture_success(repo, &["log"]);
    assert!(out.contains("Initial file"), "log: {}", out);

    println!("  PASS");
}

fn test_branch_merge(repo: &Path) {
    println!("\n=== test_branch_merge ===");

    suture_success(repo, &["branch", "feature"]);

    fs::write(repo.join("main.txt"), "main content\n").unwrap();
    suture_success(repo, &["add", "main.txt"]);
    suture_success(repo, &["commit", "main change"]);

    suture_success(repo, &["checkout", "feature"]);

    fs::write(repo.join("feature.txt"), "feature content\n").unwrap();
    suture_success(repo, &["add", "feature.txt"]);
    suture_success(repo, &["commit", "feature change"]);

    suture_success(repo, &["checkout", "main"]);

    let out = suture_success(repo, &["merge", "feature"]);
    assert!(out.contains("Merge successful"), "merge output: {}", out);

    assert!(
        repo.join("main.txt").exists(),
        "main.txt should exist after merge"
    );
    assert!(
        repo.join("feature.txt").exists(),
        "feature.txt should exist after merge"
    );

    println!("  PASS");
}

fn test_gc(repo: &Path) {
    println!("\n=== test_gc ===");

    suture_success(repo, &["branch", "temp-branch"]);
    suture_success(repo, &["checkout", "temp-branch"]);
    fs::write(repo.join("temp.txt"), "temporary\n").unwrap();
    suture_success(repo, &["add", "temp.txt"]);
    suture_success(repo, &["commit", "temp commit"]);
    suture_success(repo, &["checkout", "main"]);
    suture_success(repo, &["branch", "--delete", "temp-branch"]);

    let out = suture_success(repo, &["gc"]);
    assert!(
        out.contains("Garbage collection complete"),
        "gc output: {}",
        out
    );

    println!("  PASS");
}

fn test_fsck(repo: &Path) {
    println!("\n=== test_fsck ===");

    let out = suture_success(repo, &["fsck"]);
    assert!(
        out.contains("integrity check complete"),
        "fsck output: {}",
        out
    );

    println!("  PASS");
}

fn test_bisect(repo: &Path) {
    println!("\n=== test_bisect ===");

    for i in 1..=5 {
        fs::write(
            repo.join(format!("bisect_{}.txt", i)),
            format!("content {}\n", i),
        )
        .unwrap();
        suture_success(repo, &["add", &format!("bisect_{}.txt", i)]);
        suture_success(repo, &["commit", &format!("bisect commit {}", i)]);
    }

    let log = suture_success(repo, &["log", "--oneline"]);
    let lines: Vec<&str> = log.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        lines.len() >= 5,
        "need at least 5 commits for bisect, got {}: {}",
        lines.len(),
        log
    );

    let newest_hash = lines[0].split_whitespace().next().unwrap();
    let oldest_hash = lines.last().unwrap().split_whitespace().next().unwrap();

    let out = suture_success(repo, &["bisect", newest_hash, oldest_hash]);
    assert!(out.contains("Bisecting:"), "bisect output: {}", out);

    println!("  PASS");
}

fn test_tag(repo: &Path) {
    println!("\n=== test_tag ===");

    suture_success(repo, &["tag", "v1.0"]);

    let out = suture_success(repo, &["tag", "--list"]);
    assert!(out.contains("v1.0"), "tag list: {}", out);

    suture_success(repo, &["tag", "-a", "-m", "release v2.0", "v2.0"]);

    let out = suture_success(repo, &["tag", "--list"]);
    assert!(out.contains("v2.0"), "tag list after annotated: {}", out);

    println!("  PASS");
}

fn test_stash(repo: &Path) {
    println!("\n=== test_stash ===");

    fs::write(repo.join("stash_test.txt"), "dirty\n").unwrap();
    suture_success(repo, &["add", "stash_test.txt"]);

    let out = suture_success(repo, &["stash", "push", "-m", "test stash"]);
    assert!(out.contains("Saved as stash"), "stash push: {}", out);

    let out = suture_success(repo, &["stash", "pop"]);
    assert!(out.contains("Stash popped"), "stash pop: {}", out);

    assert!(
        repo.join("stash_test.txt").exists(),
        "file should still exist after stash pop"
    );

    println!("  PASS");
}
