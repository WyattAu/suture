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

fn suture_bin() -> PathBuf {
    workspace_root().join("target").join("debug").join("suture")
}

fn driver_script() -> PathBuf {
    workspace_root()
        .join("contrib")
        .join("git-merge-driver")
        .join("suture-merge-driver")
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to execute git")
}

fn git_success(dir: &Path, args: &[&str]) -> String {
    let output = git(dir, args);
    if !output.status.success() {
        panic!(
            "git {:?} failed:\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn git_env(dir: &Path, args: &[&str], suture_path: &Path) -> std::process::Output {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    cmd.env("SUTURE_PATH", suture_path);
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            suture_path.parent().unwrap().display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    cmd.output().expect("failed to execute git")
}

fn git_ok(dir: &Path, args: &[&str], suture_path: &Path) -> String {
    let output = git_env(dir, args, suture_path);
    if !output.status.success() {
        panic!(
            "git {:?} failed:\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn setup_git_repo_with_driver(attributes: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();

    git_success(&repo, &["init", "-b", "main"]);
    git_success(&repo, &["config", "user.email", "test@example.com"]);
    git_success(&repo, &["config", "user.name", "Test User"]);

    let driver_path = driver_script();
    let driver_str = format!("{} %O %A %B %P", driver_path.display());
    git_success(
        &repo,
        &["config", "merge.suture.name", "Suture semantic merge"],
    );
    git_success(&repo, &["config", "merge.suture.driver", &driver_str]);

    fs::write(repo.join(".gitattributes"), attributes).unwrap();
    git_success(&repo, &["add", ".gitattributes"]);
    git_success(&repo, &["commit", "-m", "add .gitattributes"]);

    (tmp, repo)
}

#[test]
#[ignore = "requires compiled suture binary + merge driver script; run with: cargo test -p suture-e2e --test merge_driver_json -- --ignored"]
fn test_merge_driver_json_non_overlapping_keys() {
    let (_tmp, repo) = setup_git_repo_with_driver("*.json merge=suture\n");
    let suture = suture_bin();

    let base_json = r#"{"version": "1.0", "port": 8080}"#;
    fs::write(repo.join("config.json"), base_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "base config"], &suture);

    git_ok(&repo, &["checkout", "-b", "feature-a"], &suture);
    fs::write(repo.join("config.json"), r#"{"version": "1.1", "port": 8080}"#).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change version"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    git_ok(&repo, &["checkout", "-b", "feature-b"], &suture);
    fs::write(repo.join("config.json"), r#"{"version": "1.0", "port": 9090}"#).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change port"], &suture);

    git_ok(&repo, &["checkout", "feature-a"], &suture);
    let output = git_env(&repo, &["merge", "feature-b"], &suture);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "merge should auto-resolve:\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    let merged = fs::read_to_string(repo.join("config.json")).unwrap();
    assert!(
        merged.contains("\"version\": \"1.1\""),
        "merged should contain version from feature-a: {}",
        merged
    );
    assert!(
        merged.contains("\"port\": 9090"),
        "merged should contain port from feature-b: {}",
        merged
    );

    assert!(
        combined.contains("Fast-forward") || !combined.contains("CONFLICT"),
        "merge should fast-forward or auto-resolve without conflict"
    );
}

#[test]
#[ignore = "requires compiled suture binary + merge driver script; run with: cargo test -p suture-e2e --test merge_driver_json -- --ignored"]
fn test_merge_driver_json_same_key_conflict() {
    let (_tmp, repo) = setup_git_repo_with_driver("*.json merge=suture\n");
    let suture = suture_bin();

    let base_json = r#"{"key": "original", "other": "base"}"#;
    fs::write(repo.join("config.json"), base_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "base config"], &suture);

    git_ok(&repo, &["checkout", "-b", "conflict-a"], &suture);
    fs::write(repo.join("config.json"), r#"{"key": "from-a", "other": "base"}"#).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change key to from-a"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    fs::write(repo.join("config.json"), r#"{"key": "from-b", "other": "base"}"#).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change key to from-b"], &suture);

    let output = git_env(&repo, &["merge", "conflict-a"], &suture);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let content = fs::read_to_string(repo.join("config.json")).unwrap_or_default();
    let combined = format!("{}{}{}", content, stdout, stderr);

    assert!(
        !output.status.success(),
        "merge with same-key conflict should fail (exit != 0):\n{}",
        combined
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "conflict should produce exit code 1, got {:?}",
        output.status.code()
    );
    assert!(
        combined.contains("CONFLICT")
            || combined.contains("conflict")
            || combined.contains("<<<<<<")
            || combined.contains("merge conflict"),
        "conflict should produce markers or message: {}",
        combined
    );
}

#[test]
#[ignore = "requires compiled suture binary + merge driver script; run with: cargo test -p suture-e2e --test merge_driver_json -- --ignored"]
fn test_merge_driver_json_nested_fields() {
    let (_tmp, repo) = setup_git_repo_with_driver("*.json merge=suture\n");
    let suture = suture_bin();

    let base_json = r#"{"server": {"host": "localhost", "port": 8080}, "db": {"name": "mydb"}}"#;
    fs::write(repo.join("config.json"), base_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "base config"], &suture);

    git_ok(&repo, &["checkout", "-b", "feature-host"], &suture);
    let branch_json = r#"{"server": {"host": "0.0.0.0", "port": 8080}, "db": {"name": "mydb"}}"#;
    fs::write(repo.join("config.json"), branch_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change host"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    git_ok(&repo, &["checkout", "-b", "feature-port"], &suture);
    let main_json = r#"{"server": {"host": "localhost", "port": 9090}, "db": {"name": "mydb"}}"#;
    fs::write(repo.join("config.json"), main_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change port"], &suture);

    git_ok(&repo, &["checkout", "feature-host"], &suture);
    let output = git_env(&repo, &["merge", "feature-port"], &suture);

    assert!(
        output.status.success(),
        "nested JSON merge should succeed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let merged = fs::read_to_string(repo.join("config.json")).unwrap();
    assert!(
        merged.contains("0.0.0.0"),
        "merged should contain host from feature-host: {}",
        merged
    );
    assert!(
        merged.contains("9090"),
        "merged should contain port from feature-port: {}",
        merged
    );
}

#[test]
#[ignore = "requires compiled suture binary + merge driver script; run with: cargo test -p suture-e2e --test merge_driver_json -- --ignored"]
fn test_merge_driver_json_array_changes() {
    let (_tmp, repo) = setup_git_repo_with_driver("*.json merge=suture\n");
    let suture = suture_bin();

    let base_json = r#"{"items": [1, 2, 3]}"#;
    fs::write(repo.join("data.json"), base_json).unwrap();
    git_ok(&repo, &["add", "data.json"], &suture);
    git_ok(&repo, &["commit", "-m", "base data"], &suture);

    git_ok(&repo, &["checkout", "-b", "branch-a"], &suture);
    fs::write(repo.join("data.json"), r#"{"items": [10, 2, 3]}"#).unwrap();
    git_ok(&repo, &["add", "data.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change first item"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    git_ok(&repo, &["checkout", "-b", "branch-b"], &suture);
    fs::write(repo.join("data.json"), r#"{"items": [1, 2, 30]}"#).unwrap();
    git_ok(&repo, &["add", "data.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change last item"], &suture);

    git_ok(&repo, &["checkout", "branch-a"], &suture);
    let output = git_env(&repo, &["merge", "branch-b"], &suture);

    assert!(
        output.status.success(),
        "array merge should succeed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let merged = fs::read_to_string(repo.join("data.json")).unwrap();
    assert!(
        merged.contains("10") && merged.contains("30"),
        "merged should contain changes from both branches: {}",
        merged
    );
}
