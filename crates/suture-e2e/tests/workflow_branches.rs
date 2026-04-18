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
fn test_branch_operations() {
    let (_tmp, repo) = new_test_repo("branch_ops");

    // add file -> commit
    fs::write(repo.join("README.md"), "# My Project\n").unwrap();
    suture_success(&repo, &["add", "README.md"]);
    suture_success(&repo, &["commit", "Add README"]);

    // branch create feature1
    suture_success(&repo, &["branch", "feature1"]);

    // branch create feature2
    suture_success(&repo, &["branch", "feature2"]);

    // checkout feature1
    let out = suture_success(&repo, &["checkout", "feature1"]);
    assert!(out.contains("feature1"), "checkout feature1: {}", out);

    // add file -> commit on feature1
    fs::write(repo.join("feature.txt"), "feature work\n").unwrap();
    suture_success(&repo, &["add", "feature.txt"]);
    suture_success(&repo, &["commit", "Feature 1 work"]);

    // checkout main
    suture_success(&repo, &["checkout", "main"]);

    // branch create feature3
    suture_success(&repo, &["branch", "feature3"]);

    // branch delete feature3
    suture_success(&repo, &["branch", "--delete", "feature3"]);

    // branch list -> verify only feature1, feature2 remain (plus main)
    let out = suture_success(&repo, &["branch", "--list"]);
    assert!(
        out.contains("main"),
        "branch list should contain main: {}",
        out
    );
    assert!(
        out.contains("feature1"),
        "branch list should contain feature1: {}",
        out
    );
    assert!(
        out.contains("feature2"),
        "branch list should contain feature2: {}",
        out
    );
    assert!(
        !out.contains("feature3"),
        "branch list should NOT contain deleted feature3: {}",
        out
    );

    // checkout feature1
    let out = suture_success(&repo, &["checkout", "feature1"]);
    assert!(out.contains("feature1"), "checkout feature1: {}", out);

    // log -> verify commit history includes our feature commit
    let out = suture_success(&repo, &["log"]);
    assert!(
        out.contains("Feature 1 work"),
        "log on feature1 should contain feature commit: {}",
        out
    );
    assert!(
        out.contains("Add README"),
        "log on feature1 should contain base commit: {}",
        out
    );
}
