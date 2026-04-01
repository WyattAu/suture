use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cli_bin() -> PathBuf {
    workspace_root()
        .join("target")
        .join("debug")
        .join("suture")
}

fn suture(dir: &Path, args: &[&str]) -> std::process::Output {
    let bin = cli_bin();
    if !bin.exists() {
        panic!(
            "suture-cli not found at {} (run `cargo build` first)",
            bin.display()
        );
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
fn test_init_commit_status() {
    let (_tmp, repo) = new_test_repo("repo");

    let out = suture_success(&repo, &["status"]);
    assert!(
        out.contains("On branch main"),
        "status after init: {}",
        out
    );
    assert!(
        !out.contains("Unstaged changes:"),
        "status should be clean: {}",
        out
    );

    fs::write(repo.join("hello.txt"), "Hello, World!\n").unwrap();
    suture_success(&repo, &["add", "hello.txt"]);
    let out = suture_success(&repo, &["commit", "Initial file"]);
    assert!(out.contains("Committed:"), "commit output: {}", out);

    let out = suture_success(&repo, &["status"]);
    assert!(
        !out.contains("Unstaged changes:") && !out.contains("Staged changes:"),
        "status should be clean: {}",
        out
    );

    let out = suture_success(&repo, &["log"]);
    assert!(out.contains("Initial file"), "log: {}", out);
}

#[test]
fn test_branch_merge() {
    let (_tmp, repo) = new_test_repo("repo");

    suture_success(&repo, &["branch", "feature"]);

    fs::write(repo.join("main.txt"), "main content\n").unwrap();
    suture_success(&repo, &["add", "main.txt"]);
    suture_success(&repo, &["commit", "main change"]);

    suture_success(&repo, &["checkout", "feature"]);

    fs::write(repo.join("feature.txt"), "feature content\n").unwrap();
    suture_success(&repo, &["add", "feature.txt"]);
    suture_success(&repo, &["commit", "feature change"]);

    suture_success(&repo, &["checkout", "main"]);

    let out = suture_success(&repo, &["merge", "feature"]);
    assert!(out.contains("Merge successful"), "merge output: {}", out);

    assert!(repo.join("main.txt").exists());
    assert!(repo.join("feature.txt").exists());
}

#[test]
fn test_gc() {
    let (_tmp, repo) = new_test_repo("repo");

    suture_success(&repo, &["branch", "temp-branch"]);
    suture_success(&repo, &["checkout", "temp-branch"]);
    fs::write(repo.join("temp.txt"), "temporary\n").unwrap();
    suture_success(&repo, &["add", "temp.txt"]);
    suture_success(&repo, &["commit", "temp commit"]);
    suture_success(&repo, &["checkout", "main"]);
    suture_success(&repo, &["branch", "--delete", "temp-branch"]);

    let out = suture_success(&repo, &["gc"]);
    assert!(
        out.contains("Garbage collection complete"),
        "gc output: {}",
        out
    );
}

#[test]
fn test_fsck() {
    let (_tmp, repo) = new_test_repo("repo");

    let out = suture_success(&repo, &["fsck"]);
    assert!(
        out.contains("integrity check complete"),
        "fsck output: {}",
        out
    );
}

#[test]
fn test_bisect() {
    let (_tmp, repo) = new_test_repo("repo");

    for i in 1..=5 {
        fs::write(
            repo.join(format!("bisect_{}.txt", i)),
            format!("content {}\n", i),
        )
        .unwrap();
        suture_success(&repo, &["add", &format!("bisect_{}.txt", i)]);
        suture_success(&repo, &["commit", &format!("bisect commit {}", i)]);
    }

    let log = suture_success(&repo, &["log", "--oneline"]);
    let lines: Vec<&str> = log.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        lines.len() >= 5,
        "need >= 5 commits, got {}: {}",
        lines.len(),
        log
    );

    let newest = lines[0].split_whitespace().next().unwrap();
    let oldest = lines.last().unwrap().split_whitespace().next().unwrap();

    let out = suture_success(&repo, &["bisect", "start", oldest, newest]);
    assert!(out.contains("Bisecting:"), "bisect output: {}", out);
}

#[test]
fn test_tag() {
    let (_tmp, repo) = new_test_repo("repo");

    suture_success(&repo, &["tag", "v1.0"]);

    let out = suture_success(&repo, &["tag", "--list"]);
    assert!(out.contains("v1.0"), "tag list: {}", out);

    suture_success(&repo, &["tag", "-a", "-m", "release v2.0", "v2.0"]);

    let out = suture_success(&repo, &["tag", "--list"]);
    assert!(out.contains("v2.0"), "tag list after annotated: {}", out);
}

#[test]
fn test_stash() {
    let (_tmp, repo) = new_test_repo("repo");

    fs::write(repo.join("stash_test.txt"), "dirty\n").unwrap();
    suture_success(&repo, &["add", "stash_test.txt"]);

    let out = suture_success(&repo, &["stash", "push", "-m", "test stash"]);
    assert!(out.contains("Saved as stash"), "stash push: {}", out);

    let out = suture_success(&repo, &["stash", "pop"]);
    assert!(out.contains("Stash popped"), "stash pop: {}", out);

    assert!(repo.join("stash_test.txt").exists());
}

async fn handshake_get() -> axum::Json<suture_hub::types::HandshakeResponse> {
    axum::Json(suture_hub::types::HandshakeResponse {
        server_version: suture_hub::types::PROTOCOL_VERSION,
        server_name: "suture-hub".to_string(),
        compatible: true,
    })
}

async fn start_test_hub() -> String {
    let mut hub = suture_hub::SutureHubServer::new_in_memory();
    hub.set_no_auth(true);
    let hub = Arc::new(hub);

    let app = axum::Router::new()
        .route(
            "/push",
            axum::routing::post(suture_hub::server::push_handler),
        )
        .route(
            "/pull",
            axum::routing::post(suture_hub::server::pull_handler),
        )
        .route("/handshake", axum::routing::get(handshake_get))
        .route(
            "/handshake",
            axum::routing::post(suture_hub::server::handshake_handler),
        )
        .with_state(hub);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let client = reqwest::Client::new();
    for _ in 0..50 {
        if client
            .get(format!("{}/handshake", &url))
            .send()
            .await
            .is_ok()
        {
            return url;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("Hub did not start in time");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_push_pull_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_dir = tmp.path().join("repo");
    fs::create_dir_all(&repo_dir).unwrap();
    init_repo(&repo_dir);

    let hub_url = start_test_hub().await;

    suture_success(&repo_dir, &["remote", "add", "origin", &hub_url]);

    fs::write(repo_dir.join("file.txt"), "hello").unwrap();
    suture_success(&repo_dir, &["add", "file.txt"]);
    suture_success(&repo_dir, &["commit", "initial"]);

    let out = suture_success(&repo_dir, &["push", "origin"]);
    assert!(out.contains("Push successful"), "push output: {}", out);

    let clone_dir = tmp.path().join("clone");
    let out = suture_success(
        tmp.path(),
        &["clone", &hub_url, clone_dir.to_str().unwrap()],
    );
    assert!(out.contains("Cloned into"), "clone output: {}", out);

    assert!(clone_dir.join("file.txt").exists());
    let content = fs::read_to_string(clone_dir.join("file.txt")).unwrap();
    assert_eq!(content, "hello");

    suture_success(&clone_dir, &["config", "user.name=Bob"]);
    fs::write(clone_dir.join("file.txt"), "world").unwrap();
    suture_success(&clone_dir, &["add", "file.txt"]);
    suture_success(&clone_dir, &["commit", "change"]);

    let out = suture_success(&clone_dir, &["push", "origin"]);
    assert!(
        out.contains("Push successful"),
        "push from clone: {}",
        out
    );

    let out = suture_success(&repo_dir, &["pull", "origin"]);
    assert!(out.contains("Pull successful"), "pull output: {}", out);

    let content = fs::read_to_string(repo_dir.join("file.txt")).unwrap();
    assert_eq!(content, "world");
}
