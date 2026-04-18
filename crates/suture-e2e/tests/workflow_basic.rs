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
fn test_basic_workflow() {
    let (_tmp, repo) = new_test_repo("basic_workflow");

    // add file -> stage a file
    fs::write(repo.join("hello.txt"), "Hello, World!\n").unwrap();
    suture_success(&repo, &["add", "hello.txt"]);

    // commit -> create first patch
    let out = suture_success(&repo, &["commit", "Initial commit"]);
    assert!(out.contains("Committed:"), "commit output: {}", out);

    // status -> verify clean state
    let out = suture_success(&repo, &["status"]);
    assert!(out.contains("On branch main"), "status: {}", out);
    assert!(
        !out.contains("Unstaged changes:") && !out.contains("Staged changes:"),
        "status should be clean: {}",
        out
    );

    // branch create -> create feature branch
    suture_success(&repo, &["branch", "feature"]);

    // checkout -> switch to feature branch
    let out = suture_success(&repo, &["checkout", "feature"]);
    assert!(out.contains("feature"), "checkout output: {}", out);

    // modify file -> change content
    fs::write(repo.join("hello.txt"), "Hello, Feature!\n").unwrap();

    // add -> stage change
    suture_success(&repo, &["add", "hello.txt"]);

    // commit -> second patch
    let out = suture_success(&repo, &["commit", "Feature change"]);
    assert!(out.contains("Committed:"), "second commit output: {}", out);

    // log -> verify 2 commits
    let out = suture_success(&repo, &["log"]);
    assert!(
        out.contains("Initial commit"),
        "log should contain initial commit: {}",
        out
    );
    assert!(
        out.contains("Feature change"),
        "log should contain feature commit: {}",
        out
    );

    // branch -> list branches
    let out = suture_success(&repo, &["branch", "--list"]);
    assert!(
        out.contains("main"),
        "branch list should contain main: {}",
        out
    );
    assert!(
        out.contains("feature"),
        "branch list should contain feature: {}",
        out
    );

    // checkout main -> back to main
    let out = suture_success(&repo, &["checkout", "main"]);
    assert!(out.contains("main"), "checkout main output: {}", out);

    // merge -> merge feature branch
    let out = suture_success(&repo, &["merge", "feature"]);
    assert!(
        out.contains("Merge successful")
            || out.contains("merged")
            || out.contains("Already up to date"),
        "merge output: {}",
        out
    );

    // log -> verify merge commit or at least both commits present
    let out = suture_success(&repo, &["log"]);
    assert!(
        out.contains("Initial commit"),
        "log after merge should contain initial commit: {}",
        out
    );
    assert!(
        out.contains("Feature change"),
        "log after merge should contain feature commit: {}",
        out
    );

    // verify working tree content reflects the merge
    let content = fs::read_to_string(repo.join("hello.txt")).unwrap();
    assert_eq!(
        content, "Hello, Feature!\n",
        "file should have feature branch content after merge"
    );
}
