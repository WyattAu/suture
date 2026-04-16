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

fn setup_git_repo() -> (tempfile::TempDir, PathBuf) {
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

    fs::write(repo.join(".gitattributes"), "*.json merge=suture\n").unwrap();
    git_success(&repo, &["add", ".gitattributes"]);
    git_success(&repo, &["commit", "-m", "add .gitattributes"]);

    (tmp, repo)
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

#[test]
fn test_git_merge_driver_clean_merge() {
    let (_tmp, repo) = setup_git_repo();
    let suture = suture_bin();

    let base_json = r#"{"port": 8080, "host": "localhost", "debug": false}"#;
    fs::write(repo.join("config.json"), base_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "base config"], &suture);

    git_ok(&repo, &["checkout", "-b", "feature-a"], &suture);
    let feature_a_json = r#"{"port": 9090, "host": "localhost", "debug": false}"#;
    fs::write(repo.join("config.json"), feature_a_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change port to 9090"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    let feature_b_json = r#"{"port": 8080, "host": "0.0.0.0", "debug": false}"#;
    fs::write(repo.join("config.json"), feature_b_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change host to 0.0.0.0"], &suture);

    let output = git_env(&repo, &["merge", "feature-a"], &suture);
    assert!(
        output.status.success(),
        "merge feature-a into main should succeed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let merged = fs::read_to_string(repo.join("config.json")).unwrap();
    assert!(
        merged.contains("9090"),
        "merged JSON should contain feature-a's port change, got: {}",
        merged
    );
    assert!(
        merged.contains("0.0.0.0"),
        "merged JSON should contain feature-b's host change, got: {}",
        merged
    );
}

#[test]
fn test_git_merge_driver_conflict_exit_code() {
    let (_tmp, repo) = setup_git_repo();
    let suture = suture_bin();

    let base_json = r#"{"key": "original"}"#;
    fs::write(repo.join("config.json"), base_json).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "base config"], &suture);

    git_ok(&repo, &["checkout", "-b", "conflict-a"], &suture);
    fs::write(repo.join("config.json"), r#"{"key": "from-a"}"#).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change key to from-a"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    fs::write(repo.join("config.json"), r#"{"key": "from-b"}"#).unwrap();
    git_ok(&repo, &["add", "config.json"], &suture);
    git_ok(&repo, &["commit", "-m", "change key to from-b"], &suture);

    let output = git_env(&repo, &["merge", "conflict-a"], &suture);
    assert!(
        !output.status.success(),
        "merge with same-key conflict should fail (exit != 0):\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "merge conflict should produce exit code 1, got {:?}",
        output.status.code()
    );

    let content = fs::read_to_string(repo.join("config.json")).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}{}", content, stdout, stderr);
    assert!(
        combined.contains("CONFLICT")
            || combined.contains("conflict")
            || combined.contains("<<<<<<")
            || combined.contains("merge conflict"),
        "merge conflict should produce conflict markers or message, got:\ncontent: {}\nstdout: {}\nstderr: {}",
        content,
        stdout,
        stderr
    );
}
