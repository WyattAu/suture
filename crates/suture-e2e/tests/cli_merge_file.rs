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

#[test]
fn test_cli_merge_file_json_add_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    fs::write(dir.join("base.json"), r#"{"name": "test", "version": "1.0"}"#).unwrap();
    fs::write(dir.join("ours.json"), r#"{"name": "test", "version": "1.1"}"#).unwrap();
    fs::write(dir.join("theirs.json"), r#"{"name": "test", "license": "MIT"}"#).unwrap();

    let out = suture_success(
        dir,
        &[
            "merge-file",
            "--driver",
            "json",
            "base.json",
            "ours.json",
            "theirs.json",
        ],
    );

    assert!(out.contains("1.1"), "should have our version change: {}", out);
    assert!(out.contains("MIT"), "should have their new key: {}", out);
    assert!(out.contains("test"), "should preserve unchanged name: {}", out);
}

#[test]
fn test_cli_merge_file_json_to_output_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    fs::write(dir.join("base.json"), r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();
    fs::write(dir.join("ours.json"), r#"{"a": 10, "b": 2, "c": 3}"#).unwrap();
    fs::write(dir.join("theirs.json"), r#"{"a": 1, "b": 2, "c": 30}"#).unwrap();

    suture_success(
        dir,
        &[
            "merge-file",
            "--driver",
            "json",
            "base.json",
            "ours.json",
            "theirs.json",
            "-o",
            "merged.json",
        ],
    );

    let merged = fs::read_to_string(dir.join("merged.json")).unwrap();
    assert!(merged.contains("\"a\": 10"), "should have our a change: {}", merged);
    assert!(merged.contains("\"c\": 30"), "should have their c change: {}", merged);
}

#[test]
fn test_cli_merge_file_yaml_explicit_driver() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    fs::write(dir.join("base.yaml"), "host: localhost\nport: 8080\ndebug: false\n").unwrap();
    fs::write(dir.join("ours.yaml"), "host: 0.0.0.0\nport: 8080\ndebug: false\n").unwrap();
    fs::write(dir.join("theirs.yaml"), "host: localhost\nport: 9090\ndebug: false\n").unwrap();

    let out = suture_success(
        dir,
        &[
            "merge-file",
            "--driver",
            "yaml",
            "base.yaml",
            "ours.yaml",
            "theirs.yaml",
        ],
    );

    assert!(out.contains("0.0.0.0"), "should have our host change: {}", out);
    assert!(out.contains("9090"), "should have their port change: {}", out);
    assert!(out.contains("false"), "should preserve unchanged debug: {}", out);
}

#[test]
fn test_cli_merge_file_yaml_output_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    fs::write(dir.join("base.yaml"), "app: myapp\nenv: dev\n").unwrap();
    fs::write(dir.join("ours.yaml"), "app: myapp\nenv: staging\n").unwrap();
    fs::write(dir.join("theirs.yaml"), "app: myapp\nlog_level: info\n").unwrap();

    suture_success(
        dir,
        &[
            "merge-file",
            "--driver",
            "yaml",
            "base.yaml",
            "ours.yaml",
            "theirs.yaml",
            "-o",
            "merged.yaml",
        ],
    );

    let merged = fs::read_to_string(dir.join("merged.yaml")).unwrap();
    assert!(merged.contains("staging"), "should have our env change: {}", merged);
    assert!(merged.contains("info"), "should have their new key: {}", merged);
}

#[test]
fn test_cli_merge_file_json_identical_sides() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    fs::write(dir.join("base.json"), r#"{"key": "base"}"#).unwrap();
    fs::write(dir.join("ours.json"), r#"{"key": "changed"}"#).unwrap();
    fs::write(dir.join("theirs.json"), r#"{"key": "changed"}"#).unwrap();

    let out = suture_success(
        dir,
        &[
            "merge-file",
            "--driver",
            "json",
            "base.json",
            "ours.json",
            "theirs.json",
        ],
    );

    assert!(
        out.contains("changed"),
        "identical changes on both sides should produce 'changed': {}",
        out
    );
}

#[test]
fn test_cli_merge_file_json_empty_vs_content() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    fs::write(dir.join("base.json"), "{}").unwrap();
    fs::write(dir.join("ours.json"), r#"{"a": 1}"#).unwrap();
    fs::write(dir.join("theirs.json"), r#"{"b": 2}"#).unwrap();

    let out = suture_success(
        dir,
        &[
            "merge-file",
            "--driver",
            "json",
            "base.json",
            "ours.json",
            "theirs.json",
        ],
    );

    assert!(out.contains("\"a\": 1"), "should have our addition: {}", out);
    assert!(out.contains("\"b\": 2"), "should have their addition: {}", out);
}

#[test]
fn test_cli_merge_file_yaml_nested() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    fs::write(dir.join("base.yaml"), "top:\n  a: 1\n  b: 2\n").unwrap();
    fs::write(dir.join("ours.yaml"), "top:\n  a: 10\n  b: 2\n").unwrap();
    fs::write(dir.join("theirs.yaml"), "top:\n  a: 1\n  b: 20\n").unwrap();

    let out = suture_success(
        dir,
        &[
            "merge-file",
            "--driver",
            "yaml",
            "base.yaml",
            "ours.yaml",
            "theirs.yaml",
        ],
    );

    assert!(out.contains("a: 10"), "should have our nested change: {}", out);
    assert!(out.contains("b: 20"), "should have their nested change: {}", out);
}

#[test]
fn test_cli_merge_file_no_base_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    fs::write(dir.join("base.json"), r#"{"a": 1, "b": 2}"#).unwrap();
    fs::write(dir.join("ours.json"), r#"{"a": 1, "b": 2}"#).unwrap();
    fs::write(dir.join("theirs.json"), r#"{"a": 1, "b": 2}"#).unwrap();

    let out = suture_success(
        dir,
        &[
            "merge-file",
            "--driver",
            "json",
            "base.json",
            "ours.json",
            "theirs.json",
        ],
    );

    assert!(
        out.contains("\"a\": 1") && out.contains("\"b\": 2"),
        "no-change merge should produce original content: {}",
        out
    );
}
