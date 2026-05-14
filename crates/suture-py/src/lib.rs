// SPDX-License-Identifier: MIT OR Apache-2.0
//! Python bindings for Suture version control.
//!
//! This module provides a Python-accessible interface to the Suture
//! version control system, including repository operations, branching,
//! committing, merging, rebasing, and more.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::path::PathBuf;
use suture_common::Hash;
use suture_core::repository::{BlameEntry, Repository, WorktreeEntry};

/// A Suture repository.
///
/// Provides methods for creating, opening, and manipulating Suture
/// version control repositories.
///
/// Example:
///     ```python
///     repo = suture.SutureRepo.init("/path/to/repo", author="Alice")
///     ```
#[pyclass(unsendable)]
struct SutureRepo {
    repo: Repository,
}

#[pymethods]
impl SutureRepo {
    /// Initialize a new Suture repository at the given path.
    ///
    /// Args:
    ///     path: Directory path where the repository will be created.
    ///     author: Author name (defaults to "unknown").
    ///
    /// Returns:
    ///     A new `SutureRepo` instance.
    ///
    /// Raises:
    ///     RuntimeError: If a repository already exists at the path.
    #[staticmethod]
    fn init(path: &str, author: Option<&str>) -> PyResult<Self> {
        let author = author.unwrap_or("unknown");
        let repo = Repository::init(PathBuf::from(path).as_path(), author)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { repo })
    }

    /// Open an existing Suture repository.
    ///
    /// Args:
    ///     path: Directory path containing the `.suture/` directory.
    ///
    /// Returns:
    ///     A `SutureRepo` instance connected to the repository.
    ///
    /// Raises:
    ///     RuntimeError: If the path is not a Suture repository.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let repo = Repository::open(PathBuf::from(path).as_path())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { repo })
    }

    /// Get the current branch name and head patch ID.
    ///
    /// Returns:
    ///     A tuple of (branch_name, patch_id_hex).
    #[pyo3(signature = ())]
    fn head(&self) -> PyResult<(String, String)> {
        let (branch, patch_id) = self
            .repo
            .head()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok((branch, patch_id.to_hex()))
    }

    /// List all branches.
    ///
    /// Returns:
    ///     A list of (name, patch_id_hex) pairs.
    fn list_branches(&self) -> Vec<(String, String)> {
        self.repo
            .list_branches()
            .into_iter()
            .map(|(name, id)| (name, id.to_hex()))
            .collect()
    }

    /// Create a new branch.
    ///
    /// Args:
    ///     name: Name of the new branch.
    ///     target: Optional target commit hash or branch name.
    ///
    /// Raises:
    ///     RuntimeError: If the branch name is invalid or target is not found.
    fn create_branch(&mut self, name: &str, target: Option<&str>) -> PyResult<()> {
        self.repo
            .create_branch(name, target)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Delete a branch.
    ///
    /// Args:
    ///     name: Name of the branch to delete.
    ///
    /// Raises:
    ///     RuntimeError: If the branch is the current branch or not found.
    fn delete_branch(&mut self, name: &str) -> PyResult<()> {
        self.repo
            .delete_branch(name)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Switch to a different branch.
    ///
    /// Args:
    ///     branch: Name of the branch to switch to.
    ///
    /// Raises:
    ///     RuntimeError: If the branch is not found.
    fn checkout(&mut self, branch: &str) -> PyResult<()> {
        self.repo
            .checkout(branch)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    /// Stage a single file for commit.
    ///
    /// Args:
    ///     path: Relative path of the file to stage.
    ///
    /// Raises:
    ///     RuntimeError: If the file is not found.
    fn add(&self, path: &str) -> PyResult<()> {
        self.repo
            .add(path)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Stage all changed files.
    ///
    /// Returns:
    ///     The number of files staged.
    fn add_all(&self) -> PyResult<usize> {
        self.repo
            .add_all()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Commit staged changes.
    ///
    /// Args:
    ///     message: Commit message.
    ///
    /// Returns:
    ///     The hex patch ID of the new commit.
    ///
    /// Raises:
    ///     RuntimeError: If there are no staged changes.
    fn commit(&mut self, message: &str) -> PyResult<String> {
        let patch_id = self
            .repo
            .commit(message)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(patch_id.to_hex())
    }

    /// Get repository status.
    ///
    /// Returns:
    ///     A `RepoStatus` object with branch, staged files, and patch count.
    fn status(&self) -> PyResult<RepoStatus> {
        let status = self
            .repo
            .status()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(RepoStatus {
            head_branch: status.head_branch,
            head_patch: status.head_patch.map(|h| h.to_hex()),
            branch_count: status.branch_count,
            staged_files: status
                .staged_files
                .into_iter()
                .map(|(path, fs)| (path, format!("{:?}", fs)))
                .collect(),
            unstaged_files: Vec::new(),
            untracked_files: Vec::new(),
            patch_count: status.patch_count,
        })
    }

    /// Get commit log for a branch (or HEAD if None).
    ///
    /// Args:
    ///     branch: Optional branch name. Defaults to HEAD.
    ///
    /// Returns:
    ///     A list of `PyLogEntry` objects.
    fn log(&self, branch: Option<&str>) -> PyResult<Vec<PyLogEntry>> {
        let patches = self
            .repo
            .log(branch)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(patches
            .into_iter()
            .map(|p| PyLogEntry {
                id: p.id.to_hex(),
                message: p.message,
                author: p.author,
                timestamp: p.timestamp as i64,
                parents: p.parent_ids.into_iter().map(|pid| pid.to_hex()).collect(),
            })
            .collect())
    }

    /// Read a file from the repository at HEAD.
    ///
    /// Args:
    ///     path: Relative path of the file to read.
    ///
    /// Returns:
    ///     The file contents as a string.
    ///
    /// Raises:
    ///     RuntimeError: If the file is not found in HEAD or is not valid UTF-8.
    fn read_file(&self, path: &str) -> PyResult<String> {
        let tree = self
            .repo
            .snapshot_head()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let hash = tree
            .get(path)
            .ok_or_else(|| PyRuntimeError::new_err(format!("file not found in HEAD: {}", path)))?;
        let blob = self
            .repo
            .cas()
            .get_blob(hash)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        String::from_utf8(blob)
            .map_err(|e| PyRuntimeError::new_err(format!("invalid UTF-8: {}", e)))
    }

    /// Get the diff between two commits or branches.
    ///
    /// Args:
    ///     from: Optional source ref (commit hash, branch name, or "HEAD").
    ///     to: Optional target ref. If both are None, shows uncommitted changes.
    ///
    /// Returns:
    ///     A list of `PyDiffEntry` objects.
    fn diff(&self, from: Option<&str>, to: Option<&str>) -> PyResult<Vec<PyDiffEntry>> {
        let entries = self
            .repo
            .diff(from, to)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(entries
            .into_iter()
            .map(|e| PyDiffEntry {
                path: e.path,
                diff_type: e.diff_type.to_string(),
                old_hash: e.old_hash.map(|h| h.to_hex()),
                new_hash: e.new_hash.map(|h| h.to_hex()),
            })
            .collect())
    }

    /// Reset HEAD to a target (hex hash or branch name).
    ///
    /// Args:
    ///     target: Hex hash or branch name to reset to.
    ///     mode: "soft", "mixed", or "hard".
    ///
    /// Returns:
    ///     The hex patch ID of the reset target.
    ///
    /// Raises:
    ///     RuntimeError: If the target is not found or mode is invalid.
    fn reset(&mut self, target: &str, mode: &str) -> PyResult<String> {
        let reset_mode = match mode {
            "soft" => suture_core::repository::ResetMode::Soft,
            "mixed" => suture_core::repository::ResetMode::Mixed,
            "hard" => suture_core::repository::ResetMode::Hard,
            _ => {
                return Err(PyRuntimeError::new_err(format!(
                    "invalid reset mode: {}",
                    mode
                )));
            }
        };
        let patch_id = self
            .repo
            .reset(target, reset_mode)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(patch_id.to_hex())
    }

    /// Revert a commit by its hex patch ID.
    ///
    /// Args:
    ///     patch_id_hex: Hex patch ID of the commit to revert.
    ///     message: Optional commit message (defaults to "Revert <patch_id>").
    ///
    /// Returns:
    ///     The hex patch ID of the new revert commit.
    ///
    /// Raises:
    ///     RuntimeError: If the patch is not found or cannot be reverted.
    fn revert(&mut self, patch_id_hex: &str, message: Option<&str>) -> PyResult<String> {
        let patch_id =
            Hash::from_hex(patch_id_hex).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let new_id = self
            .repo
            .revert(&patch_id, message)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(new_id.to_hex())
    }

    /// Merge a branch into the current HEAD.
    ///
    /// Args:
    ///     source_branch: Name of the branch to merge from.
    ///
    /// Returns:
    ///     A `PyMergeResult` object with merge details.
    ///
    /// Raises:
    ///     RuntimeError: If the branch is not found or a merge is already in progress.
    fn merge(&mut self, source_branch: &str) -> PyResult<PyMergeResult> {
        let result = self
            .repo
            .execute_merge(source_branch)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyMergeResult {
            is_clean: result.is_clean,
            merge_patch_id: result.merge_patch_id.map(|id| id.to_hex()),
            patches_applied: result.patches_applied,
            conflicts: result
                .unresolved_conflicts
                .into_iter()
                .map(|c| PyConflictInfo {
                    path: c.path,
                    our_patch_id: c.our_patch_id.to_hex(),
                    their_patch_id: c.their_patch_id.to_hex(),
                })
                .collect(),
        })
    }

    /// Rebase the current branch onto a target branch.
    ///
    /// Args:
    ///     target_branch: Name of the branch to rebase onto.
    ///
    /// Returns:
    ///     A `PyRebaseResult` object with rebase details.
    ///
    /// Raises:
    ///     RuntimeError: If the target branch is not found.
    fn rebase(&mut self, target_branch: &str) -> PyResult<PyRebaseResult> {
        let result = self
            .repo
            .rebase(target_branch)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyRebaseResult {
            patches_replayed: result.patches_replayed,
            new_tip: result.new_tip.to_hex(),
        })
    }

    /// Cherry-pick a commit by its hex patch ID.
    ///
    /// Args:
    ///     patch_id_hex: Hex patch ID of the commit to cherry-pick.
    ///
    /// Returns:
    ///     The hex patch ID of the new cherry-picked commit.
    ///
    /// Raises:
    ///     RuntimeError: If the patch is not found or cannot be cherry-picked.
    fn cherry_pick(&mut self, patch_id_hex: &str) -> PyResult<String> {
        let patch_id =
            Hash::from_hex(patch_id_hex).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let new_id = self
            .repo
            .cherry_pick(&patch_id)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(new_id.to_hex())
    }

    /// Run garbage collection.
    ///
    /// Removes unreachable patches from the repository.
    ///
    /// Returns:
    ///     A `PyGcResult` object with the number of patches removed.
    fn gc(&self) -> PyResult<PyGcResult> {
        let result = self
            .repo
            .gc()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyGcResult {
            patches_removed: result.patches_removed,
        })
    }

    /// Verify repository integrity.
    ///
    /// Checks DAG consistency, branch integrity, blob references, and HEAD.
    ///
    /// Returns:
    ///     A `PyFsckResult` object with check details.
    fn fsck(&self) -> PyResult<PyFsckResult> {
        let result = self
            .repo
            .fsck(false)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyFsckResult {
            checks_passed: result.checks_passed,
            warnings: result.warnings,
            errors: result.errors,
        })
    }

    /// Get a config value.
    ///
    /// Args:
    ///     key: Configuration key.
    ///
    /// Returns:
    ///     The config value, or None if not set.
    fn get_config(&self, key: &str) -> PyResult<Option<String>> {
        self.repo
            .get_config(key)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Set a config value.
    ///
    /// Args:
    ///     key: Configuration key.
    ///     value: Configuration value.
    fn set_config(&mut self, key: &str, value: &str) -> PyResult<()> {
        self.repo
            .set_config(key, value)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List all config key-value pairs.
    ///
    /// Returns:
    ///     A list of (key, value) tuples.
    fn list_config(&self) -> PyResult<Vec<(String, String)>> {
        self.repo
            .list_config()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Create a tag pointing to a patch ID (or HEAD).
    ///
    /// Args:
    ///     name: Tag name.
    ///     target: Optional target (hex hash, branch name, or "HEAD").
    fn create_tag(&mut self, name: &str, target: Option<&str>) -> PyResult<()> {
        self.repo
            .create_tag(name, target)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Delete a tag.
    ///
    /// Args:
    ///     name: Tag name to delete.
    ///
    /// Raises:
    ///     RuntimeError: If the tag does not exist.
    fn delete_tag(&mut self, name: &str) -> PyResult<()> {
        self.repo
            .delete_tag(name)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List all tags.
    ///
    /// Returns:
    ///     A list of (name, patch_id_hex) pairs.
    fn list_tags(&self) -> PyResult<Vec<(String, String)>> {
        let tags = self
            .repo
            .list_tags()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(tags
            .into_iter()
            .map(|(name, id)| (name, id.to_hex()))
            .collect())
    }

    /// Stash current changes.
    ///
    /// Args:
    ///     message: Optional stash message (defaults to "WIP").
    ///
    /// Returns:
    ///     The stash index.
    ///
    /// Raises:
    ///     RuntimeError: If there are no changes to stash.
    fn stash_push(&mut self, message: Option<&str>) -> PyResult<usize> {
        self.repo
            .stash_push(message)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Pop the most recent stash.
    ///
    /// Raises:
    ///     RuntimeError: If there are no stashes.
    fn stash_pop(&mut self) -> PyResult<()> {
        self.repo
            .stash_pop()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List all stashes.
    ///
    /// Returns:
    ///     A list of `PyStashEntry` objects.
    fn stash_list(&self) -> PyResult<Vec<PyStashEntry>> {
        let entries = self
            .repo
            .stash_list()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(entries
            .into_iter()
            .map(|e| PyStashEntry {
                index: e.index,
                message: e.message,
                branch: e.branch,
                head_id: e.head_id,
            })
            .collect())
    }

    /// Drop a stash by index.
    ///
    /// Args:
    ///     index: Stash index to drop.
    ///
    /// Raises:
    ///     RuntimeError: If the stash index is not found.
    fn stash_drop(&mut self, index: usize) -> PyResult<()> {
        self.repo
            .stash_drop(index)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Add a note to a commit.
    ///
    /// Args:
    ///     patch_id_hex: Hex patch ID of the commit.
    ///     message: Note text to attach.
    ///
    /// Raises:
    ///     RuntimeError: If the patch ID is invalid.
    fn add_note(&self, patch_id_hex: &str, message: &str) -> PyResult<()> {
        let patch_id =
            Hash::from_hex(patch_id_hex).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.repo
            .add_note(&patch_id, message)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List notes for a commit.
    ///
    /// Args:
    ///     patch_id_hex: Hex patch ID of the commit.
    ///
    /// Returns:
    ///     A list of note strings.
    fn list_notes(&self, patch_id_hex: &str) -> PyResult<Vec<String>> {
        let patch_id =
            Hash::from_hex(patch_id_hex).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.repo
            .list_notes(&patch_id)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Remove a note from a commit by index.
    ///
    /// Args:
    ///     patch_id_hex: Hex patch ID of the commit.
    ///     index: Zero-based index of the note to remove.
    ///
    /// Raises:
    ///     RuntimeError: If the index is out of range.
    fn remove_note(&self, patch_id_hex: &str, index: usize) -> PyResult<()> {
        let patch_id =
            Hash::from_hex(patch_id_hex).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.repo
            .remove_note(&patch_id, index)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Add a worktree.
    ///
    /// Creates a new directory linked to this repo's data, checked out
    /// on the specified or main branch.
    ///
    /// Args:
    ///     name: Worktree name (no slashes or special characters).
    ///     path: Path where the worktree will be created.
    ///     branch: Optional branch name (defaults to "main").
    ///
    /// Raises:
    ///     RuntimeError: If the name is invalid, path exists, or this is a linked worktree.
    fn add_worktree(&mut self, name: &str, path: &str, branch: Option<&str>) -> PyResult<()> {
        self.repo
            .add_worktree(name, PathBuf::from(path).as_path(), branch)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List all worktrees.
    ///
    /// Returns the main worktree plus any linked worktrees.
    ///
    /// Returns:
    ///     A list of `PyWorktreeEntry` objects.
    fn list_worktrees(&self) -> PyResult<Vec<PyWorktreeEntry>> {
        let entries = self
            .repo
            .list_worktrees()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(entries.into_iter().map(|e| e.into()).collect())
    }

    /// Remove a worktree by name.
    ///
    /// Deletes the worktree directory and cleans up config entries.
    ///
    /// Args:
    ///     name: Worktree name to remove.
    ///
    /// Raises:
    ///     RuntimeError: If the worktree is not found.
    fn remove_worktree(&mut self, name: &str) -> PyResult<()> {
        self.repo
            .remove_worktree(name)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Show per-line commit attribution for a file.
    ///
    /// Returns a list of blame entries, one per line in the file at HEAD.
    ///
    /// Args:
    ///     path: Relative path of the file to blame.
    ///
    /// Returns:
    ///     A list of `PyBlameEntry` objects.
    ///
    /// Raises:
    ///     RuntimeError: If the file is not found in HEAD.
    fn blame(&self, path: &str, at: Option<&str>) -> PyResult<Vec<PyBlameEntry>> {
        let entries = self
            .repo
            .blame(path, at)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(entries.into_iter().map(|e| e.into()).collect())
    }

    /// Start a bisect session.
    ///
    /// Args:
    ///     good: Hex patch ID of a known-good commit.
    ///     bad: Hex patch ID of a known-bad commit.
    ///
    /// Raises:
    ///     RuntimeError: If either patch ID is invalid.
    fn bisect_start(&mut self, good: &str, bad: &str) -> PyResult<()> {
        let good_id = Hash::from_hex(good).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let bad_id = Hash::from_hex(bad).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.repo
            .set_config("bisect.good", &good_id.to_hex())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.repo
            .set_config("bisect.bad", &bad_id.to_hex())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Mark a commit as good during bisect.
    ///
    /// Args:
    ///     patch_id_hex: Hex patch ID of the commit to mark as good.
    ///
    /// Returns:
    ///     The hex patch ID of the suggested next commit to test, or None if done.
    fn bisect_good(&mut self, patch_id_hex: &str) -> PyResult<Option<String>> {
        let _patch_id =
            Hash::from_hex(patch_id_hex).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.repo
            .set_config("bisect.last_good", patch_id_hex)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(None)
    }

    /// Mark a commit as bad during bisect.
    ///
    /// Args:
    ///     patch_id_hex: Hex patch ID of the commit to mark as bad.
    ///
    /// Returns:
    ///     The hex patch ID of the suggested next commit to test, or None if done.
    fn bisect_bad(&mut self, patch_id_hex: &str) -> PyResult<Option<String>> {
        let _patch_id =
            Hash::from_hex(patch_id_hex).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.repo
            .set_config("bisect.last_bad", patch_id_hex)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(None)
    }

    /// Add a remote.
    ///
    /// Args:
    ///     name: Remote name.
    ///     url: Remote URL.
    fn add_remote(&self, name: &str, url: &str) -> PyResult<()> {
        self.repo
            .add_remote(name, url)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Remove a remote.
    ///
    /// Args:
    ///     name: Remote name to remove.
    ///
    /// Raises:
    ///     RuntimeError: If the remote is not found.
    fn remove_remote(&self, name: &str) -> PyResult<()> {
        self.repo
            .remove_remote(name)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List configured remotes.
    ///
    /// Returns:
    ///     A list of (name, url) tuples.
    fn list_remotes(&self) -> PyResult<Vec<(String, String)>> {
        self.repo
            .list_remotes()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Get the URL for a remote.
    ///
    /// Args:
    ///     name: Remote name.
    ///
    /// Returns:
    ///     The remote URL.
    ///
    /// Raises:
    ///     RuntimeError: If the remote is not found.
    fn get_remote_url(&self, name: &str) -> PyResult<String> {
        self.repo
            .get_remote_url(name)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
}

/// Repository status information.
///
/// Contains the current HEAD branch, staged files, and patch count.
#[pyclass]
#[derive(Clone)]
struct RepoStatus {
    /// Current HEAD branch name.
    #[pyo3(get)]
    head_branch: Option<String>,
    /// Current HEAD patch ID (hex string).
    #[pyo3(get)]
    head_patch: Option<String>,
    /// Total number of branches.
    #[pyo3(get)]
    branch_count: usize,
    /// Staged files as (path, status) pairs.
    #[pyo3(get)]
    staged_files: Vec<(String, String)>,
    /// Unstaged modified files as (path, status) pairs.
    #[pyo3(get)]
    unstaged_files: Vec<(String, String)>,
    /// Untracked files (relative paths).
    #[pyo3(get)]
    untracked_files: Vec<String>,
    /// Total number of patches in the DAG.
    #[pyo3(get)]
    patch_count: usize,
}

#[pymethods]
impl RepoStatus {
    /// Return a string representation of the status.
    fn __repr__(&self) -> String {
        format!(
            "RepoStatus(branch={:?}, patches={})",
            self.head_branch, self.patch_count
        )
    }
}

/// A log entry from the commit history.
#[pyclass]
#[derive(Clone)]
struct PyLogEntry {
    /// Patch ID (hex string).
    #[pyo3(get)]
    id: String,
    /// Commit message.
    #[pyo3(get)]
    message: String,
    /// Author name.
    #[pyo3(get)]
    author: String,
    /// Unix timestamp.
    #[pyo3(get)]
    timestamp: i64,
    /// Parent patch IDs (hex strings).
    #[pyo3(get)]
    parents: Vec<String>,
}

#[pymethods]
impl PyLogEntry {
    /// Return a string representation of the log entry.
    fn __repr__(&self) -> String {
        let short_id = if self.id.len() >= 12 {
            &self.id[..12]
        } else {
            &self.id
        };
        format!("LogEntry({}...: {})", short_id, self.message)
    }
}

/// A diff entry describing a file change.
#[pyclass]
#[derive(Clone)]
struct PyDiffEntry {
    /// File path.
    #[pyo3(get)]
    path: String,
    /// Type of change (e.g., "Added", "Modified", "Deleted").
    #[pyo3(get)]
    diff_type: String,
    /// Old blob hash (hex), if applicable.
    #[pyo3(get)]
    old_hash: Option<String>,
    /// New blob hash (hex), if applicable.
    #[pyo3(get)]
    new_hash: Option<String>,
}

#[pymethods]
impl PyDiffEntry {
    /// Return a string representation of the diff entry.
    fn __repr__(&self) -> String {
        format!("DiffEntry({} {})", self.diff_type, self.path)
    }
}

/// Result of a merge operation.
#[pyclass]
struct PyMergeResult {
    /// Whether the merge completed without conflicts.
    #[pyo3(get)]
    is_clean: bool,
    /// Merge commit patch ID (hex), or None if conflicts remain.
    #[pyo3(get)]
    merge_patch_id: Option<String>,
    /// Number of patches applied from the source branch.
    #[pyo3(get)]
    patches_applied: usize,
    /// List of unresolved conflicts, if any.
    #[pyo3(get)]
    conflicts: Vec<PyConflictInfo>,
}

#[pymethods]
impl PyMergeResult {
    /// Return a string representation of the merge result.
    fn __repr__(&self) -> String {
        if self.is_clean {
            format!(
                "MergeResult(clean, {} patches applied)",
                self.patches_applied
            )
        } else {
            format!(
                "MergeResult(conflicts, {} patches applied, {} conflicts)",
                self.patches_applied,
                self.conflicts.len()
            )
        }
    }
}

/// Information about an unresolved merge conflict.
#[pyclass]
#[derive(Clone)]
struct PyConflictInfo {
    /// File path where the conflict occurs.
    #[pyo3(get)]
    path: String,
    /// Patch ID from the current branch (hex).
    #[pyo3(get)]
    our_patch_id: String,
    /// Patch ID from the source branch (hex).
    #[pyo3(get)]
    their_patch_id: String,
}

#[pymethods]
impl PyConflictInfo {
    /// Return a string representation of the conflict info.
    fn __repr__(&self) -> String {
        format!("ConflictInfo({})", self.path)
    }
}

/// Result of a rebase operation.
#[pyclass]
struct PyRebaseResult {
    /// Number of patches replayed.
    #[pyo3(get)]
    patches_replayed: usize,
    /// New tip patch ID (hex) after rebase.
    #[pyo3(get)]
    new_tip: String,
}

#[pymethods]
impl PyRebaseResult {
    /// Return a string representation of the rebase result.
    fn __repr__(&self) -> String {
        format!(
            "RebaseResult({} patches, new_tip={})",
            self.patches_replayed, self.new_tip
        )
    }
}

/// Result of a garbage collection pass.
#[pyclass]
struct PyGcResult {
    /// Number of unreachable patches removed.
    #[pyo3(get)]
    patches_removed: usize,
}

#[pymethods]
impl PyGcResult {
    /// Return a string representation of the GC result.
    fn __repr__(&self) -> String {
        format!("GcResult({} patches removed)", self.patches_removed)
    }
}

/// Result of a filesystem check (fsck).
#[pyclass]
struct PyFsckResult {
    /// Number of checks that passed.
    #[pyo3(get)]
    checks_passed: usize,
    /// Non-fatal warnings encountered.
    #[pyo3(get)]
    warnings: Vec<String>,
    /// Fatal errors encountered.
    #[pyo3(get)]
    errors: Vec<String>,
}

#[pymethods]
impl PyFsckResult {
    /// Return a string representation of the fsck result.
    fn __repr__(&self) -> String {
        format!(
            "FsckResult({} checks passed, {} warnings, {} errors)",
            self.checks_passed,
            self.warnings.len(),
            self.errors.len()
        )
    }
}

/// A stash entry.
#[pyclass]
#[derive(Clone)]
struct PyStashEntry {
    /// Stash index.
    #[pyo3(get)]
    index: usize,
    /// Stash message.
    #[pyo3(get)]
    message: String,
    /// Branch name when the stash was created.
    #[pyo3(get)]
    branch: String,
    /// HEAD patch ID when the stash was created (hex).
    #[pyo3(get)]
    head_id: String,
}

#[pymethods]
impl PyStashEntry {
    /// Return a string representation of the stash entry.
    fn __repr__(&self) -> String {
        format!("StashEntry(@{{{}}}: {})", self.index, self.message)
    }
}

/// Information about a worktree.
#[pyclass]
#[derive(Clone)]
struct PyWorktreeEntry {
    /// Worktree name (empty string for the main worktree).
    #[pyo3(get)]
    name: String,
    /// Filesystem path of the worktree.
    #[pyo3(get)]
    path: String,
    /// Branch checked out in the worktree.
    #[pyo3(get)]
    branch: String,
    /// Whether this is the main worktree.
    #[pyo3(get)]
    is_main: bool,
}

#[pymethods]
impl PyWorktreeEntry {
    /// Return a string representation of the worktree entry.
    fn __repr__(&self) -> String {
        if self.is_main {
            format!("WorktreeEntry(main: {})", self.path)
        } else {
            format!("WorktreeEntry({}: {})", self.name, self.path)
        }
    }
}

impl From<WorktreeEntry> for PyWorktreeEntry {
    fn from(e: WorktreeEntry) -> Self {
        Self {
            name: e.name,
            path: e.path,
            branch: e.branch,
            is_main: e.is_main,
        }
    }
}

/// A single blame entry for one line of a file.
#[pyclass]
#[derive(Clone)]
struct PyBlameEntry {
    /// 1-based line number.
    #[pyo3(get)]
    line_number: usize,
    /// Line content.
    #[pyo3(get)]
    content: String,
    /// Patch ID that last modified this line (hex).
    #[pyo3(get)]
    patch_id: String,
    /// Author of the commit that modified this line.
    #[pyo3(get)]
    author: String,
    /// Unix timestamp of the commit.
    #[pyo3(get)]
    timestamp: i64,
}

#[pymethods]
impl PyBlameEntry {
    /// Return a string representation of the blame entry.
    fn __repr__(&self) -> String {
        format!(
            "BlameEntry(line {}: {} <{}>)",
            self.line_number,
            self.author,
            &self.patch_id[..12.min(self.patch_id.len())]
        )
    }
}

impl From<BlameEntry> for PyBlameEntry {
    fn from(e: BlameEntry) -> Self {
        Self {
            line_number: e.line_number,
            content: e.line,
            patch_id: e.patch_id.to_hex(),
            author: e.author,
            timestamp: 0,
        }
    }
}

/// Compute the content-addressable hash of arbitrary bytes.
///
/// Args:
///     data: Raw bytes to hash.
///
/// Returns:
///     The hex hash string.
#[pyfunction]
fn hash_bytes(data: &[u8]) -> String {
    Hash::from_data(data).to_hex()
}

/// Check if a path is a Suture repository.
///
/// Args:
///     path: Directory path to check.
///
/// Returns:
///     True if the path contains a `.suture/` directory, False otherwise.
#[pyfunction]
fn is_repo(path: &str) -> bool {
    PathBuf::from(path).join(".suture").exists()
}

/// Python module initialization.
///
/// Exports all public classes and functions for the `suture` Python package.
#[pymodule]
fn suture(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SutureRepo>()?;
    m.add_class::<RepoStatus>()?;
    m.add_class::<PyLogEntry>()?;
    m.add_class::<PyDiffEntry>()?;
    m.add_class::<PyMergeResult>()?;
    m.add_class::<PyConflictInfo>()?;
    m.add_class::<PyRebaseResult>()?;
    m.add_class::<PyGcResult>()?;
    m.add_class::<PyFsckResult>()?;
    m.add_class::<PyStashEntry>()?;
    m.add_class::<PyWorktreeEntry>()?;
    m.add_class::<PyBlameEntry>()?;
    m.add_function(wrap_pyfunction!(hash_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(is_repo, m)?)?;
    m.add(
        "__all__",
        pyo3::types::PyList::new(
            m.py(),
            vec![
                "SutureRepo",
                "RepoStatus",
                "PyLogEntry",
                "PyDiffEntry",
                "PyMergeResult",
                "PyConflictInfo",
                "PyRebaseResult",
                "PyGcResult",
                "PyFsckResult",
                "PyStashEntry",
                "PyWorktreeEntry",
                "PyBlameEntry",
                "hash_bytes",
                "is_repo",
            ],
        )?,
    )?;
    Ok(())
}
