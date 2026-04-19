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
fn test_integrity_basic_diff() {
    let (_tmp, repo) = new_test_repo("integrity_basic_diff");

    fs::write(repo.join("file.txt"), "hello world\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);
    suture_success(&repo, &["commit", "initial"]);

    fs::write(repo.join("file.txt"), "hello universe\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);

    let out = suture_success(&repo, &["diff", "--cached", "--integrity"]);
    let out_lower = out.to_lowercase();
    assert!(
        out_lower.contains("integrity"),
        "output should contain 'integrity':\n{}",
        out
    );
    assert!(
        out.contains("file.txt"),
        "output should mention file.txt:\n{}",
        out
    );
    let has_entropy_value = out
        .split_whitespace()
        .any(|word| word.parse::<f64>().is_ok() && word.contains('.') && word.len() >= 3);
    assert!(
        has_entropy_value,
        "output should contain an entropy decimal value:\n{}",
        out
    );
}

#[test]
fn test_integrity_no_changes() {
    let (_tmp, repo) = new_test_repo("integrity_no_changes");

    fs::write(repo.join("file.txt"), "hello world\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);
    suture_success(&repo, &["commit", "initial"]);

    let out = suture_success(&repo, &["diff", "--integrity"]);
    assert!(
        out.contains("No differences"),
        "output should say 'No differences' when nothing changed:\n{}",
        out
    );
}

#[test]
fn test_integrity_detects_build_script() {
    let (_tmp, repo) = new_test_repo("integrity_build_script");

    fs::write(repo.join("configure"), "#!/bin/sh\n./build\n").unwrap();
    suture_success(&repo, &["add", "configure"]);
    suture_success(&repo, &["commit", "initial"]);

    fs::write(repo.join("configure"), "#!/bin/sh\n./build\necho done\n").unwrap();
    suture_success(&repo, &["add", "configure"]);

    let out = suture_success(&repo, &["diff", "--cached", "--integrity"]);
    let out_lower = out.to_lowercase();
    assert!(
        out.contains("configure"),
        "output should mention configure:\n{}",
        out
    );
    assert!(
        out_lower.contains("buildscript") || out_lower.contains("build script"),
        "output should mention build script:\n{}",
        out
    );
}

#[test]
fn test_integrity_detects_binary_file() {
    let (_tmp, repo) = new_test_repo("integrity_binary");

    let binary_data: Vec<u8> = (0..255u8).cycle().take(1024).collect();
    fs::write(repo.join("data.bin"), &binary_data).unwrap();
    suture_success(&repo, &["add", "data.bin"]);

    let out = suture_success(&repo, &["diff", "--cached", "--integrity"]);
    assert!(
        out.contains("data.bin"),
        "output should mention data.bin:\n{}",
        out
    );
    let out_upper = out.to_uppercase();
    assert!(
        out_upper.contains("HIGH")
            || out_upper.contains("MAXIMUM")
            || out.to_lowercase().contains("binary"),
        "output should mention high entropy or binary:\n{}",
        out
    );
}

#[test]
fn test_integrity_multiple_files() {
    let (_tmp, repo) = new_test_repo("integrity_multiple");

    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
    fs::write(repo.join("README.md"), "# Test\n").unwrap();
    suture_success(&repo, &["add", "main.rs"]);
    suture_success(&repo, &["add", "Cargo.toml"]);
    suture_success(&repo, &["add", "README.md"]);
    suture_success(&repo, &["commit", "initial"]);

    fs::write(repo.join("main.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.2\"\n",
    )
    .unwrap();
    fs::write(repo.join("README.md"), "# Test\nUpdated.\n").unwrap();
    suture_success(&repo, &["add", "main.rs"]);
    suture_success(&repo, &["add", "Cargo.toml"]);
    suture_success(&repo, &["add", "README.md"]);

    let out = suture_success(&repo, &["diff", "--cached", "--integrity"]);
    assert!(
        out.contains("main.rs"),
        "output should mention main.rs:\n{}",
        out
    );
    assert!(
        out.contains("Cargo.toml"),
        "output should mention Cargo.toml:\n{}",
        out
    );
    assert!(
        out.contains("README.md"),
        "output should mention README.md:\n{}",
        out
    );
    let out_upper = out.to_uppercase();
    assert!(
        out_upper.contains("RISK"),
        "output should contain risk assessment:\n{}",
        out
    );
}

#[test]
fn test_integrity_added_file() {
    let (_tmp, repo) = new_test_repo("integrity_added");

    fs::write(repo.join("base.txt"), "base\n").unwrap();
    suture_success(&repo, &["add", "base.txt"]);
    suture_success(&repo, &["commit", "initial"]);

    fs::write(repo.join("new_file.rs"), "fn main() {}\n").unwrap();
    suture_success(&repo, &["add", "new_file.rs"]);

    let out = suture_success(&repo, &["diff", "--cached", "--integrity"]);
    assert!(
        out.contains("new_file.rs"),
        "output should mention new_file.rs:\n{}",
        out
    );
    let out_lower = out.to_lowercase();
    assert!(
        out_lower.contains("size:") || out_lower.contains("bytes"),
        "output should show file details (size/bytes):\n{}",
        out
    );
}

#[test]
fn test_integrity_compressed_file() {
    let (_tmp, repo) = new_test_repo("integrity_compressed");

    fs::write(repo.join("base.txt"), "base\n").unwrap();
    suture_success(&repo, &["add", "base.txt"]);
    suture_success(&repo, &["commit", "initial"]);

    let compressed_data: Vec<u8> = (0..2000).map(|i| ((i * 37 + 13) % 256) as u8).collect();
    fs::write(repo.join("archive.gz"), &compressed_data).unwrap();
    suture_success(&repo, &["add", "archive.gz"]);

    let out = suture_success(&repo, &["diff", "--cached", "--integrity"]);
    let out_lower = out.to_lowercase();
    assert!(
        out_lower.contains("compress")
            || out_lower.contains("high")
            || out_lower.contains("entropy"),
        "output should mention compressed or high entropy risk:\n{}",
        out
    );
}

#[test]
fn test_integrity_works_with_from_to() {
    let (_tmp, repo) = new_test_repo("integrity_from_to");

    fs::write(repo.join("file.txt"), "hello world\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);
    suture_success(&repo, &["commit", "initial"]);

    suture_success(&repo, &["branch", "feature"]);

    suture_success(&repo, &["checkout", "feature"]);
    fs::write(repo.join("file.txt"), "hello feature branch\n").unwrap();
    suture_success(&repo, &["add", "file.txt"]);
    suture_success(&repo, &["commit", "feature change"]);

    let out = suture_success(
        &repo,
        &["diff", "--integrity", "--from", "main", "--to", "feature"],
    );
    assert!(
        out.contains("file.txt"),
        "output should mention file.txt:\n{}",
        out
    );
}
