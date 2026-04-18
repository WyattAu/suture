use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cli_bin() -> PathBuf {
    workspace_root().join("target").join("debug").join("suture")
}

fn suture(dir: &Path, args: &[&str]) -> std::process::Output {
    let bin = cli_bin();
    if !bin.exists() {
        panic!(
            "suture binary not found at {} (run `cargo build -p suture-cli` first)",
            bin.display()
        );
    }
    Command::new(&bin)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to execute suture")
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

fn init_repo(dir: &Path) {
    suture_success(dir, &["init", "."]);
    suture_success(dir, &["config", "user.name=Alice"]);
}

fn new_test_repo(name: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join(name);
    fs::create_dir_all(&repo).unwrap();
    init_repo(&repo);
    (tmp, repo)
}

#[test]
fn test_stash_and_reset() {
    let (_tmp, repo) = new_test_repo("stash_reset");

    // add file -> commit (initial)
    fs::write(repo.join("file.txt"), "original content\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);
    suture_success(&repo, &["commit", "Initial commit"]);

    // Create 4 more commits so we have 5 total
    for i in 1..=4 {
        let content = format!("content version {}\n", i);
        fs::write(repo.join(format!("v{}.txt", i)), &content).unwrap();
        suture_success(&repo, &["add", &format!("v{}.txt", i)]);
        suture_success(&repo, &["commit", &format!("Commit {}", i)]);
    }

    // modify file -> stage
    fs::write(repo.join("file.txt"), "modified content\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);

    // stash -> stash changes
    let out = suture_success(&repo, &["stash", "push", "-m", "work in progress"]);
    assert!(
        out.contains("Saved") || out.contains("stash"),
        "stash push: {}",
        out
    );

    // status -> verify clean (stashed changes hidden)
    let out = suture_success(&repo, &["status"]);
    assert!(
        !out.contains("Unstaged changes:") && !out.contains("Staged changes:"),
        "status should be clean after stash: {}",
        out
    );

    // stash pop -> restore changes
    let out = suture_success(&repo, &["stash", "pop"]);
    assert!(
        out.contains("Restored") || out.contains("Applied") || out.contains("pop"),
        "stash pop: {}",
        out
    );

    // status -> verify changes are back
    let out = suture_success(&repo, &["status"]);
    assert!(
        out.contains("Staged changes:") || out.contains("file.txt"),
        "status should show restored staged changes: {}",
        out
    );

    // reset HEAD~1 -> undo last commit
    let out = suture_success(&repo, &["reset", "HEAD~1"]);
    assert!(
        !out.contains("error") && !out.contains("Error"),
        "reset should succeed: {}",
        out
    );

    // log -> verify one fewer commit than before reset
    let log = suture_success(&repo, &["log", "--oneline"]);
    let lines: Vec<&str> = log.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        5,
        "expected 5 commits after reset HEAD~1, got {}: {}",
        lines.len(),
        log
    );
}
