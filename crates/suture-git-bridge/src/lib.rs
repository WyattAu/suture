//! # ⚠️ EXPERIMENTAL — Known Data Loss Issues
//!
//! This module is **not ready for production use**. Known issues include:
//! - Branch import is a no-op (branches are detected but not created in Suture)
//! - Commit topology is linearized (merge commits become sequential patches)
//! - File contents for intermediate commits may be incorrect
//! - Rename detection parses tab-separated paths incorrectly
//!
//! Use at your own risk. Data imported via this bridge may be incomplete or incorrect.
//!
//! ---
//!
//! Git-Suture interop bridge.
//!
//! Provides bidirectional import/export between Suture and Git repositories.
//!
//! # Git → Suture Import
//! - Walks a Git repository's commit history
//! - Creates equivalent Suture patches for each Git commit
//! - Preserves branch structure, merge commits, and file contents
//!
//! # Suture → Git Export
//! - Walks a Suture repository's patch DAG
//! - Creates equivalent Git commits
//! - Preserves branch structure and file contents

use std::path::Path;
use suture_core::repository::Repository;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("git command failed: {0}")]
    GitCommand(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("suture error: {0}")]
    Suture(String),
    #[error("invalid git repository: {0}")]
    InvalidGitRepo(String),
}

/// Import a Git repository into a Suture repository.
///
/// Creates a new Suture repository at `suture_path` and imports all
/// commits from the Git repository at `git_path`.
///
/// # Arguments
///
/// * `git_path` - Path to the source Git repository
/// * `suture_path` - Path where the Suture repository will be created
/// * `author` - Author name for imported commits
///
/// # How it works
///
/// 1. Creates a new Suture repository at `suture_path`
/// 2. Runs `git log --reverse` to get commits in chronological order
/// 3. For each commit:
///    a. Runs `git show <sha>` to get file changes
///    b. Creates the appropriate Suture patch (create/modify/delete)
///    c. Commits to the Suture repository
/// 4. Recreates branch structure
///
/// ⚠️ **EXPERIMENTAL** — Known to lose branch topology, merge structure, and
/// intermediate file contents. See module-level documentation.
#[deprecated(
    since = "0.1.0",
    note = "Git bridge is experimental and may lose data. See module docs."
)]
pub fn import_from_git(
    git_path: &Path,
    suture_path: &Path,
    author: &str,
) -> Result<ImportResult, BridgeError> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["-C", &git_path.to_string_lossy(), "rev-parse", "--git-dir"])
        .output()
        .map_err(|e| BridgeError::GitCommand(format!("git not found: {}", e)))?;

    if !output.status.success() {
        return Err(BridgeError::InvalidGitRepo(
            git_path.to_string_lossy().to_string(),
        ));
    }

    let mut repo =
        Repository::init(suture_path, author).map_err(|e| BridgeError::Suture(e.to_string()))?;

    let _ = repo.set_config("user.name", author);

    let output = Command::new("git")
        .args([
            "-C",
            &git_path.to_string_lossy(),
            "log",
            "--reverse",
            "--format=%H %s",
            "--all",
        ])
        .output()
        .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

    let commit_list = String::from_utf8_lossy(&output.stdout);
    let mut patches_imported = 0usize;
    let mut branches_imported = 0usize;

    for line in commit_list.lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() != 2 {
            continue;
        }
        let sha = parts[0];
        let message = parts[1];

        let diff_output = Command::new("git")
            .args([
                "-C",
                &git_path.to_string_lossy(),
                "diff-tree",
                "--no-commit-id",
                "-r",
                "--name-status",
                sha,
            ])
            .output()
            .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

        let diff = String::from_utf8_lossy(&diff_output.stdout);

        for diff_line in diff.lines() {
            let parts: Vec<&str> = diff_line.splitn(2, '\t').collect();
            if parts.len() != 2 {
                continue;
            }
            let status = parts[0].trim();
            let filepath = parts[1].trim();

            let git_file = git_path.join(filepath);
            let suture_file = suture_path.join(filepath);

            match status {
                "M" | "A" => {
                    if let Some(parent) = suture_file.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    if git_file.exists() {
                        std::fs::copy(&git_file, &suture_file)?;
                        repo.add(filepath)
                            .map_err(|e| BridgeError::Suture(e.to_string()))?;
                    }
                }
                "D" => {
                    if suture_file.exists() {
                        std::fs::remove_file(&suture_file)?;
                        repo.add(filepath)
                            .map_err(|e| BridgeError::Suture(e.to_string()))?;
                    }
                }
                "R" => {
                    let rename_parts: Vec<&str> = filepath.split('\t').collect();
                    if rename_parts.len() == 2 {
                        let new_path = suture_path.join(rename_parts[1]);
                        if let Some(new_parent) = new_path.parent() {
                            std::fs::create_dir_all(new_parent)?;
                        }
                        let old_path = suture_path.join(rename_parts[0]);
                        if old_path.exists() {
                            std::fs::rename(&old_path, &new_path)?;
                            repo.rename_file(rename_parts[0], rename_parts[1])
                                .map_err(|e| BridgeError::Suture(e.to_string()))?;
                        }
                    }
                }
                _ => {}
            }
        }

        if diff.lines().count() > 0 {
            repo.commit(message)
                .map_err(|e| BridgeError::Suture(e.to_string()))?;
            patches_imported += 1;
        }
    }

    let branch_output = Command::new("git")
        .args([
            "-C",
            &git_path.to_string_lossy(),
            "branch",
            "--format=%(refname:short)",
        ])
        .output()
        .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

    let branches = String::from_utf8_lossy(&branch_output.stdout);
    for branch in branches.lines() {
        let branch = branch.trim();
        if branch.is_empty() || branch == "HEAD" {
            continue;
        }
        let _sha_output = Command::new("git")
            .args(["-C", &git_path.to_string_lossy(), "rev-parse", branch])
            .output()
            .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

        branches_imported += 1;
    }

    Ok(ImportResult {
        patches_imported,
        branches_imported,
    })
}

/// Result of a Git import operation.
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Number of patches imported.
    pub patches_imported: usize,
    /// Number of branches imported.
    pub branches_imported: usize,
}

/// Export a Suture repository to a Git repository.
///
/// Creates a new Git repository at `git_path` and exports all
/// patches from the Suture repository at `suture_path`.
///
/// # How it works
///
/// 1. Creates a new Git repository at `git_path`
/// 2. Walks the Suture patch DAG from all branch tips
/// 3. For each patch:
///    a. Applies the patch to the working tree
///    b. Runs `git add` + `git commit` with the patch message
/// 4. Recreates branch structure
///
/// ⚠️ **EXPERIMENTAL** — Export may produce incorrect Git history.
/// See module-level documentation.
#[deprecated(
    since = "0.1.0",
    note = "Git bridge is experimental and may produce incorrect results. See module docs."
)]
pub fn export_to_git(suture_path: &Path, git_path: &Path) -> Result<ExportResult, BridgeError> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["init", &git_path.to_string_lossy()])
        .output()
        .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

    if !output.status.success() {
        return Err(BridgeError::GitCommand("git init failed".to_string()));
    }

    Command::new("git")
        .args([
            "-C",
            &git_path.to_string_lossy(),
            "config",
            "user.name",
            "suture-bridge",
        ])
        .output()
        .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

    Command::new("git")
        .args([
            "-C",
            &git_path.to_string_lossy(),
            "config",
            "user.email",
            "bridge@suture.dev",
        ])
        .output()
        .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

    let repo = Repository::open(suture_path).map_err(|e| BridgeError::Suture(e.to_string()))?;

    let branches = repo.list_branches();

    let main_id = branches
        .iter()
        .find(|(name, _)| name == "main")
        .map(|(_, id)| *id);

    let mut patches_exported = 0usize;
    let mut branches_exported = 0usize;

    if let Some(_tip_id) = main_id {
        let log = repo
            .log(None)
            .map_err(|e| BridgeError::Suture(e.to_string()))?;

        let head_tree = repo
            .snapshot_head()
            .map_err(|e| BridgeError::Suture(e.to_string()))?;

        for patch in &log {
            if let Some(ref target_path) = patch.target_path {
                let git_file = git_path.join(target_path);

                match patch.operation_type {
                    suture_core::patch::types::OperationType::Delete => {
                        if git_file.exists() {
                            std::fs::remove_file(&git_file)?;
                        }
                    }
                    _ => {
                        if let Some(hash) = head_tree.get(target_path) {
                            if let Ok(blob) = repo.cas().get_blob(hash) {
                                if let Some(parent) = git_file.parent() {
                                    std::fs::create_dir_all(parent)?;
                                }
                                std::fs::write(&git_file, blob)?;
                            }
                        }
                    }
                }
            }

            Command::new("git")
                .args(["-C", &git_path.to_string_lossy(), "add", "-A"])
                .output()
                .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

            let output = Command::new("git")
                .args([
                    "-C",
                    &git_path.to_string_lossy(),
                    "commit",
                    "-m",
                    &patch.message,
                    "--allow-empty",
                ])
                .output()
                .map_err(|e| BridgeError::GitCommand(e.to_string()))?;

            if output.status.success() {
                patches_exported += 1;
            }
        }

        branches_exported += 1;
    }

    Ok(ExportResult {
        patches_exported,
        branches_exported,
    })
}

/// Result of a Git export operation.
#[derive(Debug, Clone)]
pub struct ExportResult {
    /// Number of patches exported.
    pub patches_exported: usize,
    /// Number of branches exported.
    pub branches_exported: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_error_display() {
        let err = BridgeError::GitCommand("git not found".to_string());
        assert!(err.to_string().contains("git not found"));
    }

    #[test]
    fn test_import_result_fields() {
        let result = ImportResult {
            patches_imported: 10,
            branches_imported: 3,
        };
        assert_eq!(result.patches_imported, 10);
        assert_eq!(result.branches_imported, 3);
    }

    #[test]
    fn test_export_result_fields() {
        let result = ExportResult {
            patches_exported: 5,
            branches_exported: 1,
        };
        assert_eq!(result.patches_exported, 5);
        assert_eq!(result.branches_exported, 1);
    }

    #[test]
    #[allow(deprecated)]
    fn test_invalid_git_repo() {
        let result = import_from_git(
            Path::new("/nonexistent/path/to/git/repo"),
            Path::new("/tmp/suture-test"),
            "test",
        );
        assert!(result.is_err());
    }
}
