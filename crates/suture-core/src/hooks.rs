//! Hook system for Suture — git-compatible hook execution.
//!
//! Hooks are executable scripts found in `.suture/hooks/<hook-name>` that are
//! run at specific points in the Suture workflow. A hook exits with 0 to allow
//! the operation to proceed, or non-zero to abort.
//!
//! # Supported Hooks
//!
//! | Hook            | When                                    | Description |
//! |-----------------|-----------------------------------------|-------------|
//! | `pre-commit`    | Before `suture commit` finalizes          | Validate staged content, run linters/tests |
//! | `post-commit`   | After `suture commit` succeeds            | Send notifications, trigger CI |
//! | `pre-push`      | Before `suture push` sends to hub        | Run tests, enforce policy |
//! | `post-push`     | After `suture push` succeeds             | Send notifications, trigger deployment |
//! | `pre-merge`     | Before `suture merge` finalizes          | Validate merge safety |
//! | `post-merge`    | After a clean `suture merge` succeeds       | Send notifications |
//! | `pre-rebase`    | Before `suture rebase` replays patches    | Validate rebase safety |
//! | `post-rebase`   | After `suture rebase` completes            | Send notifications |
//! | `pre-cherry-pick`| Before `suture cherry-pick` applies a patch| Validate cherry-pick safety |
//!
//! # Hook Configuration
//!
//! - Hooks directory: `.suture/hooks/` (or override via `core.hooksPath` in `.suture/config`)
//! - Hooks are executable files named exactly by their hook type (e.g., `pre-commit`)
//! - Non-executable files or missing hooks are silently skipped
//! - Hook scripts receive environment variables with context about the operation
//!
//! # Environment Variables
//!
//! | Variable              | Description                                    |
//! |-----------------------|------------------------------------------------|
//! | `SUTURE_HOOK`          | Name of the hook being run                     |
//! | `SUTURE_REPO`          | Absolute path to the repository root           |
//! | `SUTURE_AUTHOR`        | Current author name                           |
//! | `SUTURE_BRANCH`        | Current branch name                          |
//! | `SUTURE_HEAD`           | Full hash of the current HEAD patch            |
//! | `SUTURE_OPERATION`     | Operation being performed                     |
//! | `SUTURE_HOOK_DIR`      | Path to the hooks directory                   |
//! | `SUTURE_DIFF_FILES`    | Space-separated list of changed file paths (pre-commit) |
//! | `SUTURE_PUSH_REMOTE`   | Remote name (pre-push)                         |
//! | `SUTURE_PUSH_PATCHES`  | Number of patches being pushed (pre-push)     |
//! | `SUTURE_MERGE_SOURCE`   | Source branch name (pre-merge)                |
//! | `SUTURE_MERGE_HEAD`     | HEAD patch hash at merge time (pre-merge)   |
//! | `SUTURE_REVERT_TARGET`  | Patch being reverted (pre-revert)            |

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// The result of running a hook.
#[derive(Debug, Clone)]
pub struct HookResult {
    pub hook_name: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: std::time::Duration,
}

impl HookResult {
    #[must_use] 
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// A resolved hook script path.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedHook {
    path: PathBuf,
}

/// Find the hooks directory for a repository.
///
/// Priority:
/// 1. `core.hooksPath` from `.suture/config` (if set)
/// 2. `.suture/hooks/` (default)
#[must_use] 
pub fn hooks_dir(repo_root: &Path) -> PathBuf {
    // Try to read from repo config
    let config_path = repo_root.join(".suture").join("config");
    let Some(content) = std::fs::read_to_string(&config_path).ok() else {
        return repo_root.join(".suture").join("hooks");
    };
    let Ok(config) = toml::from_str::<HashMap<String, toml::Value>>(&content) else {
        return repo_root.join(".suture").join("hooks");
    };
    let Some(toml::Value::String(path)) = config.get("core").and_then(|c| c.get("hooksPath"))
    else {
        return repo_root.join(".suture").join("hooks");
    };

    let path = PathBuf::from(&path);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(&path)
    }
}

/// Find and resolve a hook script by name.
///
/// Returns `None` if the hook doesn't exist or isn't executable.
pub(crate) fn find_hook(repo_root: &Path, hook_name: &str) -> Option<ResolvedHook> {
    let dir = hooks_dir(repo_root);
    let path = dir.join(hook_name);

    // Must exist and be executable
    if !path.exists() {
        return None;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = path.metadata()
            && meta.is_file()
            && (meta.permissions().mode() & 0o111) != 0
        {
            return Some(ResolvedHook { path });
        }
    }

    #[cfg(not(unix))]
    {
        // On non-Unix, just check it exists and has content
        if path.is_file()
            && std::fs::metadata(&path)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
            return Some(ResolvedHook { path });
        }
    }

    None
}

/// Run a hook script and capture its output.
///
/// Returns `HookResult` with the exit code and captured stdout/stderr.
pub fn run_hook(
    repo_root: &Path,
    hook_name: &str,
    #[allow(clippy::implicit_hasher)] env: &HashMap<String, String>,
) -> Result<HookResult, HookError> {
    let hook = find_hook(repo_root, hook_name).ok_or_else(|| HookError::NotFound(hook_name.to_owned()))?;

    let start = std::time::Instant::now();

    let output = std::process::Command::new(&hook.path)
        .envs(env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| HookError::ExecFailed {
            hook: hook_name.to_owned(),
            path: hook.path.display().to_string(),
            error: e.to_string(),
        })?;

    let elapsed = start.elapsed();
    let exit_code = output.status.code();

    Ok(HookResult {
        hook_name: hook_name.to_owned(),
        exit_code,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        elapsed,
    })
}

/// Run all hooks of a given type from a directory (for extensibility).
///
/// If multiple scripts match (e.g., `pre-commit.d/` directory), all are run
/// in sorted order, and any failure aborts the chain.
pub fn run_hooks(
    repo_root: &Path,
    hook_name: &str,
    #[allow(clippy::implicit_hasher)] env: &HashMap<String, String>,
) -> Result<Vec<HookResult>, HookError> {
    let dir = hooks_dir(repo_root);
    let direct_hook = dir.join(hook_name);

    let mut results = Vec::new();

    if direct_hook.exists() {
        // Single hook file
        match run_hook(repo_root, hook_name, env) {
            Ok(result) => {
                results.push(result);
            }
            Err(HookError::NotFound(_)) => {
                // Not found, try directory
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    // Check for hook directory (e.g., pre-commit.d/)
    let hook_sub_dir = dir.join(format!("{hook_name}.d"));
    if hook_sub_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&hook_sub_dir)
            .map_err(|e| HookError::ExecFailed {
                hook: hook_name.to_owned(),
                path: hook_sub_dir.display().to_string(),
                error: e.to_string(),
            })?
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_file() { Some(path) } else { None }
            })
            .collect::<Vec<_>>();
        entries.sort();

        for path in entries {
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let sub_hook_name = format!("{hook_name}/{file_name}");

            // Check executable bit (same logic as find_hook)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let Ok(meta) = path.metadata() else {
                    continue;
                };
                if meta.is_file() && (meta.permissions().mode() & 0o111) == 0 {
                    continue; // Skip non-executable files
                }
            }

            let start = std::time::Instant::now();
            let output = std::process::Command::new(&path)
                .envs(env)
                .env("SUTURE_HOOK", &sub_hook_name)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| HookError::ExecFailed {
                    hook: sub_hook_name.clone(),
                    path: path.display().to_string(),
                    error: e.to_string(),
                })?;

            let elapsed = start.elapsed();
            let result = HookResult {
                hook_name: sub_hook_name,
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                elapsed,
            };
            if !result.success() {
                return Err(HookError::ExecFailed {
                    hook: result.hook_name,
                    path: path.display().to_string(),
                    error: format!("hook exited with code {:?}", result.exit_code),
                });
            }
            results.push(result);
        }
    }

    Ok(results)
}

/// Build the standard environment variables for a hook invocation.
///
/// The caller should provide `author`, `branch`, and `head_hash` from the
/// repository when available (e.g. via `repo.head()` and `repo.get_config()`).
#[must_use] 
pub fn build_env(
    repo_root: &Path,
    hook_name: &str,
    author: Option<&str>,
    branch: Option<&str>,
    head_hash: Option<&str>,
    #[allow(clippy::implicit_hasher)] extra: HashMap<String, String>,
) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Standard suture vars
    env.insert("SUTURE_HOOK".to_owned(), hook_name.to_owned());
    env.insert(
        "SUTURE_REPO".to_owned(),
        repo_root.to_string_lossy().to_string(),
    );
    env.insert(
        "SUTURE_HOOK_DIR".to_owned(),
        hooks_dir(repo_root).to_string_lossy().to_string(),
    );
    env.insert("SUTURE_OPERATION".to_owned(), hook_name.to_owned());

    // Author
    if let Some(a) = author {
        env.insert("SUTURE_AUTHOR".to_owned(), a.to_owned());
    }

    // Branch
    if let Some(b) = branch {
        env.insert("SUTURE_BRANCH".to_owned(), b.to_owned());
    }

    // HEAD hash
    if let Some(h) = head_hash {
        env.insert("SUTURE_HEAD".to_owned(), h.to_owned());
    }

    // Add any extra env vars
    for (k, v) in extra {
        env.insert(k, v);
    }

    env
}

/// Format hook results for display to the user.
#[must_use] 
pub fn format_hook_result(result: &HookResult) -> String {
    let status = if result.success() { "passed" } else { "FAILED" };
    format!(
        "{}: {} ({})",
        result.hook_name,
        status,
        result.exit_code.unwrap_or(-1)
    )
}

/// Errors that can occur during hook execution.
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("hook not found: {0}")]
    NotFound(String),
    #[error("hook '{hook}' exec failed: {path}: {error}")]
    ExecFailed {
        hook: String,
        path: String,
        error: String,
    },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_hook(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        // Write to a temporary file first, sync it, then rename.
        // This avoids "Text file busy" (ETXTBSY) on Linux where the kernel
        // may still be flushing the file when another thread tries to exec it.
        let tmp_path = path.with_extension("tmp");
        {
            use std::io::Write;
            let mut f = fs::File::create(&tmp_path).unwrap();
            f.write_all(content.as_bytes()).unwrap();
            f.sync_all().unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::rename(&tmp_path, &path).unwrap();
        path
    }

    #[test]
    fn test_find_hook_exists_and_executable() {
        let tmp = tempfile::tempdir().unwrap();
        let hook_dir = tmp.path().join(".suture").join("hooks");
        fs::create_dir_all(&hook_dir).unwrap();
        make_hook(&hook_dir, "pre-commit", "#!/bin/sh\nexit 0");

        let hook = find_hook(tmp.path(), "pre-commit");
        assert!(hook.is_some());
        assert_eq!(hook.unwrap().path, hook_dir.join("pre-commit"));
    }

    #[test]
    fn test_find_hook_not_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let hook = find_hook(tmp.path(), "pre-commit");
        assert!(hook.is_none());
    }

    #[test]
    fn test_find_hook_not_executable() {
        let tmp = tempfile::tempdir().unwrap();
        let hook_dir = tmp.path().join(".suture").join("hooks");
        fs::create_dir_all(&hook_dir).unwrap();
        let path = hook_dir.join("pre-commit");
        fs::write(&path, "#!/bin/sh\nexit 0").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }

        let hook = find_hook(tmp.path(), "pre-commit");
        #[cfg(unix)]
        {
            assert!(hook.is_none());
        }
        #[cfg(not(unix))]
        {
            assert!(hook.is_some());
        }
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_run_hook_success() {
        let tmp = tempfile::tempdir().unwrap();
        let hook_dir = tmp.path().join(".suture").join("hooks");
        fs::create_dir_all(&hook_dir).unwrap();
        make_hook(
            &hook_dir,
            "pre-commit",
            "#!/bin/sh\necho 'hook ran'\nexit 0",
        );

        let env = build_env(tmp.path(), "pre-commit", None, None, None, HashMap::new());
        let result = run_hook(tmp.path(), "pre-commit", &env).unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "hook ran");
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_run_hook_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let hook_dir = tmp.path().join(".suture").join("hooks");
        fs::create_dir_all(&hook_dir).unwrap();
        make_hook(
            &hook_dir,
            "pre-commit",
            "#!/bin/sh\necho 'failing' >&2\nexit 1",
        );

        let env = build_env(tmp.path(), "pre-commit", None, None, None, HashMap::new());
        let result = run_hook(tmp.path(), "pre-commit", &env).unwrap();
        assert!(!result.success());
        assert_eq!(result.exit_code, Some(1));
        assert!(result.stderr.contains("failing"));
    }

    #[test]
    fn test_run_hook_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let env = build_env(tmp.path(), "pre-commit", None, None, None, HashMap::new());
        let err = run_hook(tmp.path(), "pre-commit", &env);
        assert!(matches!(err, Err(HookError::NotFound(_))));
    }

    #[test]
    fn test_build_env_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let env = build_env(tmp.path(), "pre-commit", None, None, None, HashMap::new());

        assert_eq!(env.get("SUTURE_HOOK").unwrap(), "pre-commit");
        assert!(
            env.get("SUTURE_REPO")
                .unwrap()
                .contains(tmp.path().to_str().unwrap())
        );
        assert_eq!(env.get("SUTURE_OPERATION").unwrap(), "pre-commit");
        // No author/branch/head when not provided
        assert!(!env.contains_key("SUTURE_AUTHOR"));
        assert!(!env.contains_key("SUTURE_BRANCH"));
        assert!(!env.contains_key("SUTURE_HEAD"));
    }

    #[test]
    fn test_build_env_with_author_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let env = build_env(
            tmp.path(),
            "pre-commit",
            Some("Alice"),
            Some("main"),
            Some("abc123"),
            HashMap::new(),
        );

        assert_eq!(env.get("SUTURE_AUTHOR").unwrap(), "Alice");
        assert_eq!(env.get("SUTURE_BRANCH").unwrap(), "main");
        assert_eq!(env.get("SUTURE_HEAD").unwrap(), "abc123");
    }

    #[test]
    fn test_build_env_with_extras() {
        let tmp = tempfile::tempdir().unwrap();
        let mut extras = HashMap::new();
        extras.insert("CUSTOM_VAR".to_string(), "value".to_string());
        let env = build_env(tmp.path(), "pre-push", None, None, None, extras);

        assert_eq!(env.get("CUSTOM_VAR").unwrap(), "value");
        assert_eq!(env.get("SUTURE_HOOK").unwrap(), "pre-push");
    }

    #[test]
    fn test_format_hook_result() {
        let result = HookResult {
            hook_name: "pre-commit".to_string(),
            exit_code: Some(0),
            stdout: "all good".to_string(),
            stderr: String::new(),
            elapsed: std::time::Duration::from_millis(5),
        };
        let formatted = format_hook_result(&result);
        assert!(formatted.contains("passed"));
    }

    #[test]
    fn test_format_hook_result_failure() {
        let result = HookResult {
            hook_name: "pre-commit".to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "error!".to_string(),
            elapsed: std::time::Duration::from_millis(3),
        };
        let formatted = format_hook_result(&result);
        assert!(formatted.contains("FAILED"));
    }

    #[test]
    fn test_hooks_dir_default() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = hooks_dir(tmp.path());
        assert!(dir.to_string_lossy().contains(".suture"));
        assert!(dir.to_string_lossy().contains("hooks"));
    }

    #[test]
    fn test_hooks_dir_from_config() {
        let tmp = tempfile::tempdir().unwrap();
        let suture_dir = tmp.path().join(".suture");
        fs::create_dir_all(&suture_dir).unwrap();

        let config = r#"
[core]
hooksPath = "my-hooks"
"#;
        fs::write(suture_dir.join("config"), config).unwrap();

        let dir = hooks_dir(tmp.path());
        assert!(dir.to_string_lossy().contains("my-hooks"));
    }

    #[test]
    fn test_hooks_dir_from_config_absolute() {
        let tmp = tempfile::tempdir().unwrap();
        let suture_dir = tmp.path().join(".suture");
        fs::create_dir_all(&suture_dir).unwrap();

        let config = r#"
[core]
hooksPath = "/tmp/custom-hooks"
"#;
        fs::write(suture_dir.join("config"), config).unwrap();

        let dir = hooks_dir(tmp.path());
        assert!(dir.to_string_lossy().contains("/tmp/custom-hooks"));
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_run_hooks_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let hook_dir = tmp.path().join(".suture").join("hooks");
        fs::create_dir_all(&hook_dir).unwrap();
        let hook_subdir = hook_dir.join("pre-commit.d");
        fs::create_dir_all(&hook_subdir).unwrap();

        make_hook(&hook_subdir, "01-check", "#!/bin/sh\nexit 0");
        make_hook(&hook_subdir, "02-lint", "#!/bin/sh\nexit 0");
        make_hook(&hook_subdir, "03-test", "#!/bin/sh\nexit 0");

        let env = build_env(tmp.path(), "pre-commit", None, None, None, HashMap::new());
        let results = run_hooks(tmp.path(), "pre-commit", &env).unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success()));
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_run_hooks_directory_failure_stops() {
        let tmp = tempfile::tempdir().unwrap();
        let hook_dir = tmp.path().join(".suture").join("hooks");
        fs::create_dir_all(&hook_dir).unwrap();
        let hook_subdir = hook_dir.join("pre-commit.d");
        fs::create_dir_all(&hook_subdir).unwrap();

        make_hook(&hook_subdir, "01-pass", "#!/bin/sh\nexit 0");
        make_hook(&hook_subdir, "02-fail", "#!/bin/sh\nexit 1");

        let env = build_env(tmp.path(), "pre-commit", None, None, None, HashMap::new());
        let err = run_hooks(tmp.path(), "pre-commit", &env);
        assert!(err.is_err());
    }
}
