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

fn setup_git_repo_with_yaml_driver() -> (tempfile::TempDir, PathBuf) {
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

    fs::write(
        repo.join(".gitattributes"),
        "*.yaml merge=suture\n*.yml merge=suture\n",
    )
    .unwrap();
    git_success(&repo, &["add", ".gitattributes"]);
    git_success(&repo, &["commit", "-m", "add .gitattributes"]);

    (tmp, repo)
}

#[test]
#[ignore = "requires compiled suture binary + merge driver script; run with: cargo test -p suture-e2e --test merge_driver_yaml -- --ignored"]
fn test_merge_driver_yaml_non_overlapping_keys() {
    let (_tmp, repo) = setup_git_repo_with_yaml_driver();
    let suture = suture_bin();

    let base_yaml = "version: \"1.0\"\nport: 8080\nhost: localhost\n";
    fs::write(repo.join("config.yaml"), base_yaml).unwrap();
    git_ok(&repo, &["add", "config.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "base config"], &suture);

    git_ok(&repo, &["checkout", "-b", "feature-version"], &suture);
    fs::write(
        repo.join("config.yaml"),
        "version: \"1.1\"\nport: 8080\nhost: localhost\n",
    )
    .unwrap();
    git_ok(&repo, &["add", "config.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "bump version"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    git_ok(&repo, &["checkout", "-b", "feature-port"], &suture);
    fs::write(
        repo.join("config.yaml"),
        "version: \"1.0\"\nport: 9090\nhost: localhost\n",
    )
    .unwrap();
    git_ok(&repo, &["add", "config.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "change port"], &suture);

    git_ok(&repo, &["checkout", "feature-version"], &suture);
    let output = git_env(&repo, &["merge", "feature-port"], &suture);

    assert!(
        output.status.success(),
        "YAML merge should auto-resolve:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let merged = fs::read_to_string(repo.join("config.yaml")).unwrap();
    assert!(
        merged.contains("1.1"),
        "merged should contain version from feature-version: {}",
        merged
    );
    assert!(
        merged.contains("9090"),
        "merged should contain port from feature-port: {}",
        merged
    );
    assert!(
        merged.contains("localhost"),
        "merged should preserve unchanged host: {}",
        merged
    );
}

#[test]
#[ignore = "requires compiled suture binary + merge driver script; run with: cargo test -p suture-e2e --test merge_driver_yaml -- --ignored"]
fn test_merge_driver_yaml_same_key_conflict() {
    let (_tmp, repo) = setup_git_repo_with_yaml_driver();
    let suture = suture_bin();

    let base_yaml = "key: original\nother: base\n";
    fs::write(repo.join("data.yaml"), base_yaml).unwrap();
    git_ok(&repo, &["add", "data.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "base"], &suture);

    git_ok(&repo, &["checkout", "-b", "branch-a"], &suture);
    fs::write(repo.join("data.yaml"), "key: from-a\nother: base\n").unwrap();
    git_ok(&repo, &["add", "data.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "change key to from-a"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    fs::write(repo.join("data.yaml"), "key: from-b\nother: base\n").unwrap();
    git_ok(&repo, &["add", "data.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "change key to from-b"], &suture);

    let output = git_env(&repo, &["merge", "branch-a"], &suture);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let content = fs::read_to_string(repo.join("data.yaml")).unwrap_or_default();
    let combined = format!("{}{}{}", content, stdout, stderr);

    assert!(
        !output.status.success(),
        "same-key YAML conflict should fail (exit != 0):\n{}",
        combined
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "conflict should produce exit code 1"
    );
    assert!(
        combined.contains("CONFLICT")
            || combined.contains("conflict")
            || combined.contains("<<<<<<"),
        "conflict should produce markers or message: {}",
        combined
    );
}

#[test]
#[ignore = "requires compiled suture binary + merge driver script; run with: cargo test -p suture-e2e --test merge_driver_yaml -- --ignored"]
fn test_merge_driver_yaml_nested_keys() {
    let (_tmp, repo) = setup_git_repo_with_yaml_driver();
    let suture = suture_bin();

    let base_yaml = "server:\n  host: localhost\n  port: 8080\ndb:\n  name: mydb\n";
    fs::write(repo.join("config.yaml"), base_yaml).unwrap();
    git_ok(&repo, &["add", "config.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "base config"], &suture);

    git_ok(&repo, &["checkout", "-b", "feature-host"], &suture);
    fs::write(
        repo.join("config.yaml"),
        "server:\n  host: 0.0.0.0\n  port: 8080\ndb:\n  name: mydb\n",
    )
    .unwrap();
    git_ok(&repo, &["add", "config.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "change host"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    git_ok(&repo, &["checkout", "-b", "feature-db"], &suture);
    fs::write(
        repo.join("config.yaml"),
        "server:\n  host: localhost\n  port: 8080\ndb:\n  name: prod\n",
    )
    .unwrap();
    git_ok(&repo, &["add", "config.yaml"], &suture);
    git_ok(&repo, &["commit", "-m", "change db name"], &suture);

    git_ok(&repo, &["checkout", "feature-host"], &suture);
    let output = git_env(&repo, &["merge", "feature-db"], &suture);

    assert!(
        output.status.success(),
        "nested YAML merge should succeed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let merged = fs::read_to_string(repo.join("config.yaml")).unwrap();
    assert!(
        merged.contains("0.0.0.0"),
        "merged should contain host from feature-host: {}",
        merged
    );
    assert!(
        merged.contains("prod"),
        "merged should contain db name from feature-db: {}",
        merged
    );
}

#[test]
#[ignore = "requires compiled suture binary + merge driver script; run with: cargo test -p suture-e2e --test merge_driver_yaml -- --ignored"]
fn test_merge_driver_yaml_yml_extension() {
    let (_tmp, repo) = setup_git_repo_with_yaml_driver();
    let suture = suture_bin();

    let base_yaml = "key1: value1\nkey2: value2\n";
    fs::write(repo.join("config.yml"), base_yaml).unwrap();
    git_ok(&repo, &["add", "config.yml"], &suture);
    git_ok(&repo, &["commit", "-m", "base"], &suture);

    git_ok(&repo, &["checkout", "-b", "branch-a"], &suture);
    fs::write(repo.join("config.yml"), "key1: changed-a\nkey2: value2\n").unwrap();
    git_ok(&repo, &["add", "config.yml"], &suture);
    git_ok(&repo, &["commit", "-m", "change key1"], &suture);

    git_ok(&repo, &["checkout", "main"], &suture);
    git_ok(&repo, &["checkout", "-b", "branch-b"], &suture);
    fs::write(repo.join("config.yml"), "key1: value1\nkey2: changed-b\n").unwrap();
    git_ok(&repo, &["add", "config.yml"], &suture);
    git_ok(&repo, &["commit", "-m", "change key2"], &suture);

    git_ok(&repo, &["checkout", "branch-a"], &suture);
    let output = git_env(&repo, &["merge", "branch-b"], &suture);

    assert!(
        output.status.success(),
        ".yml merge should auto-resolve:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let merged = fs::read_to_string(repo.join("config.yml")).unwrap();
    assert!(
        merged.contains("changed-a"),
        "merged .yml should contain key1 change: {}",
        merged
    );
    assert!(
        merged.contains("changed-b"),
        "merged .yml should contain key2 change: {}",
        merged
    );
}
