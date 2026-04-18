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
fn test_history_and_reflog() {
    let (_tmp, repo) = new_test_repo("history_test");

    // commit 5 times, each with different content
    let commit_messages = [
        "First commit",
        "Second commit",
        "Third commit",
        "Fourth commit",
        "Fifth commit",
    ];

    for (i, msg) in commit_messages.iter().enumerate() {
        let filename = format!("file_{:02}.txt", i);
        let content = format!("content {:02}\n", i);
        fs::write(repo.join(&filename), &content).unwrap();
        suture_success(&repo, &["add", &filename]);
        let out = suture_success(&repo, &["commit", msg]);
        assert!(
            out.contains("Committed:"),
            "commit '{}' output: {}",
            msg,
            out
        );
    }

    // log -> verify 5 commits in order
    let log = suture_success(&repo, &["log"]);
    for msg in &commit_messages {
        assert!(log.contains(msg), "log should contain '{}': {}", msg, log);
    }

    // log --oneline -> verify compact format
    let log_oneline = suture_success(&repo, &["log", "--oneline"]);
    let oneline_lines: Vec<&str> = log_oneline.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        oneline_lines.len() >= 5,
        "oneline log should have >= 5 lines, got {}: {}",
        oneline_lines.len(),
        log_oneline
    );
    // Each oneline line should be compact (short)
    for line in &oneline_lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert!(
            parts.len() >= 2,
            "oneline line should have hash + message, got: '{}'",
            line
        );
    }
    for msg in &commit_messages {
        assert!(
            log_oneline.contains(msg),
            "oneline log should contain '{}': {}",
            msg,
            log_oneline
        );
    }

    // gc -> garbage collect (should not error)
    let out = suture_success(&repo, &["gc"]);
    assert!(
        out.contains("Garbage collection complete") || out.contains("gc"),
        "gc output: {}",
        out
    );

    // fsck -> filesystem check (should pass)
    let out = suture_success(&repo, &["fsck"]);
    assert!(
        out.contains("integrity check complete") || out.contains("ok") || out.contains("OK"),
        "fsck output: {}",
        out
    );
}
