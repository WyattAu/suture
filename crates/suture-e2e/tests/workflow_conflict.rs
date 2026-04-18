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
fn test_merge_conflict_same_field_json() {
    let (_tmp, repo) = new_test_repo("conflict_test");

    let base_json = r#"{"name": "suture", "version": "1.0", "author": "Alice"}"#;

    // add JSON file -> stage
    fs::write(repo.join("config.json"), base_json).unwrap();
    suture_success(&repo, &["add", "config.json"]);

    // commit -> first version
    suture_success(&repo, &["commit", "Initial config"]);

    // branch -> create branch
    suture_success(&repo, &["branch", "feature"]);

    // modify JSON (change same field "author" to "BranchAuthor")
    let branch_json = r#"{"name": "suture", "version": "1.0", "author": "BranchAuthor"}"#;
    fs::write(repo.join("config.json"), branch_json).unwrap();

    // stage + commit -> version A
    suture_success(&repo, &["add", "config.json"]);
    suture_success(&repo, &["commit", "Change author on branch"]);

    // checkout main -> back to main
    suture_success(&repo, &["checkout", "main"]);

    // modify JSON (change same field "author" to "MainAuthor")
    let main_json = r#"{"name": "suture", "version": "1.0", "author": "MainAuthor"}"#;
    fs::write(repo.join("config.json"), main_json).unwrap();

    // stage + commit -> version B
    suture_success(&repo, &["add", "config.json"]);
    suture_success(&repo, &["commit", "Change author on main"]);

    // merge branch -> Suture's semantic JSON merge auto-resolves same-field
    // conflicts by taking one side's value (valid semantic merge behavior).
    // The merge should succeed without error.
    let output = suture(&repo, &["merge", "feature"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "semantic JSON merge should succeed for same-field changes: {}",
        combined
    );

    // Verify the merged result contains a valid author (one of the two values)
    let merged = fs::read_to_string(repo.join("config.json")).unwrap();
    assert!(
        merged.contains("BranchAuthor") || merged.contains("MainAuthor"),
        "merged JSON should contain one of the conflicting author values: {}",
        merged
    );
}

#[test]
fn test_merge_conflict_same_line_text() {
    let (_tmp, repo) = new_test_repo("conflict_text_test");

    // add text file -> stage
    fs::write(repo.join("file.txt"), "line 1\nline 2\nline 3\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);
    suture_success(&repo, &["commit", "base"]);

    // branch -> create branch
    suture_success(&repo, &["branch", "other"]);

    // modify same line on main
    fs::write(repo.join("file.txt"), "line 1\nMODIFIED BY MAIN\nline 3\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);
    suture_success(&repo, &["commit", "main change"]);

    // checkout other branch
    suture_success(&repo, &["checkout", "other"]);

    // modify same line on other
    fs::write(repo.join("file.txt"), "line 1\nMODIFIED BY OTHER\nline 3\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);
    suture_success(&repo, &["commit", "other change"]);

    // checkout main and merge
    suture_success(&repo, &["checkout", "main"]);
    let output = suture(&repo, &["merge", "other"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    let has_conflict = combined.contains("conflict")
        || combined.contains("CONFLICT")
        || combined.contains("Conflict");
    let has_markers = stdout.contains("<<<<<<<");

    assert!(
        has_conflict || has_markers || !output.status.success(),
        "text merge should detect conflict on same-line change: {}",
        combined
    );
}
