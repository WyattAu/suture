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
fn test_semantic_merge_non_overlapping() {
    let (_tmp, repo) = new_test_repo("semantic_merge");

    // add initial file -> stage
    fs::write(repo.join("base.txt"), "shared base content\n").unwrap();
    suture_success(&repo, &["add", "base.txt"]);

    // commit -> first version
    let out = suture_success(&repo, &["commit", "Initial file"]);
    assert!(out.contains("Committed:"), "commit output: {}", out);

    // branch -> create branch
    suture_success(&repo, &["branch", "feature"]);

    // add a new file on branch (non-overlapping with main's change)
    fs::write(repo.join("version.txt"), "version: 2.0\n").unwrap();
    suture_success(&repo, &["add", "version.txt"]);

    // commit -> version A
    let out = suture_success(&repo, &["commit", "Add version info"]);
    assert!(out.contains("Committed:"), "branch commit output: {}", out);

    // checkout main -> back to main
    suture_success(&repo, &["checkout", "main"]);

    // add a different new file on main (non-overlapping with branch's change)
    fs::write(repo.join("author.txt"), "author: Bob\n").unwrap();
    suture_success(&repo, &["add", "author.txt"]);

    // commit -> version B
    let out = suture_success(&repo, &["commit", "Add author info"]);
    assert!(out.contains("Committed:"), "main commit output: {}", out);

    // merge branch -> should auto-merge (non-overlapping changes)
    let out = suture_success(&repo, &["merge", "feature"]);
    assert!(
        out.contains("Merge successful") || out.contains("merged") || out.contains("Merged"),
        "merge should succeed for non-overlapping changes: {}",
        out
    );

    // verify result contains both branch A file (version.txt) and branch B file (author.txt)
    assert!(
        repo.join("version.txt").exists(),
        "merged tree should contain version.txt from branch"
    );
    assert!(
        repo.join("author.txt").exists(),
        "merged tree should contain author.txt from main"
    );
    assert!(
        repo.join("base.txt").exists(),
        "merged tree should still contain base.txt"
    );

    let version_content = fs::read_to_string(repo.join("version.txt")).unwrap();
    assert_eq!(version_content, "version: 2.0\n");

    let author_content = fs::read_to_string(repo.join("author.txt")).unwrap();
    assert_eq!(author_content, "author: Bob\n");
}
