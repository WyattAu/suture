//! Python bindings for Suture version control.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::path::PathBuf;
use suture_common::Hash;
use suture_core::repository::Repository;

/// A Suture repository.
#[pyclass]
struct SutureRepo {
    repo: Repository,
}

#[pymethods]
impl SutureRepo {
    /// Initialize a new Suture repository at the given path.
    #[staticmethod]
    fn init(path: &str, author: Option<&str>) -> PyResult<Self> {
        let author = author.unwrap_or("unknown");
        let repo = Repository::init(PathBuf::from(path).as_path(), author)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { repo })
    }

    /// Open an existing Suture repository.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let repo = Repository::open(PathBuf::from(path).as_path())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { repo })
    }

    /// Get the current branch name and head patch ID.
    fn head(&self) -> PyResult<(String, String)> {
        let (branch, patch_id) = self
            .repo
            .head()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok((branch, patch_id.to_hex()))
    }

    /// List all branches as (name, patch_id_hex) pairs.
    fn list_branches(&self) -> Vec<(String, String)> {
        self.repo
            .list_branches()
            .into_iter()
            .map(|(name, id)| (name, id.to_hex()))
            .collect()
    }

    /// Create a new branch. Optionally target a specific commit or branch.
    fn create_branch(&mut self, name: &str, target: Option<&str>) -> PyResult<()> {
        self.repo
            .create_branch(name, target)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Delete a branch.
    fn delete_branch(&mut self, name: &str) -> PyResult<()> {
        self.repo
            .delete_branch(name)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Switch to a different branch.
    fn checkout(&mut self, branch: &str) -> PyResult<()> {
        self.repo
            .checkout(branch)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    /// Stage a single file for commit.
    fn add(&self, path: &str) -> PyResult<()> {
        self.repo
            .add(path)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Stage all changed files.
    fn add_all(&self) -> PyResult<usize> {
        self.repo
            .add_all()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Commit staged changes.
    fn commit(&mut self, message: &str) -> PyResult<String> {
        let patch_id = self
            .repo
            .commit(message)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(patch_id.to_hex())
    }

    /// Get repository status.
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
            patch_count: status.patch_count,
        })
    }

    /// Get commit log for a branch (or HEAD if None).
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
    /// mode: "soft", "mixed", or "hard".
    fn reset(&mut self, target: &str, mode: &str) -> PyResult<String> {
        let reset_mode = match mode {
            "soft" => suture_core::repository::ResetMode::Soft,
            "mixed" => suture_core::repository::ResetMode::Mixed,
            "hard" => suture_core::repository::ResetMode::Hard,
            _ => {
                return Err(PyRuntimeError::new_err(format!(
                    "invalid reset mode: {}",
                    mode
                )))
            }
        };
        let patch_id = self
            .repo
            .reset(target, reset_mode)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(patch_id.to_hex())
    }

    /// Revert a commit by its hex patch ID.
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
    fn fsck(&self) -> PyResult<PyFsckResult> {
        let result = self
            .repo
            .fsck()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyFsckResult {
            checks_passed: result.checks_passed,
            warnings: result.warnings,
            errors: result.errors,
        })
    }

    /// Get a config value.
    fn get_config(&self, key: &str) -> PyResult<Option<String>> {
        self.repo
            .get_config(key)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Set a config value.
    fn set_config(&mut self, key: &str, value: &str) -> PyResult<()> {
        self.repo
            .set_config(key, value)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List all config key-value pairs.
    fn list_config(&self) -> PyResult<Vec<(String, String)>> {
        self.repo
            .list_config()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Create a tag pointing to a patch ID (or HEAD).
    fn create_tag(&mut self, name: &str, target: Option<&str>) -> PyResult<()> {
        self.repo
            .create_tag(name, target)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Delete a tag.
    fn delete_tag(&mut self, name: &str) -> PyResult<()> {
        self.repo
            .delete_tag(name)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List all tags as (name, patch_id_hex) pairs.
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
    fn stash_push(&mut self, message: Option<&str>) -> PyResult<usize> {
        self.repo
            .stash_push(message)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Pop the most recent stash.
    fn stash_pop(&mut self) -> PyResult<()> {
        self.repo
            .stash_pop()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// List all stashes.
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
    fn stash_drop(&mut self, index: usize) -> PyResult<()> {
        self.repo
            .stash_drop(index)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
}

/// Repository status information.
#[pyclass]
#[derive(Clone)]
struct RepoStatus {
    #[pyo3(get)]
    head_branch: Option<String>,
    #[pyo3(get)]
    head_patch: Option<String>,
    #[pyo3(get)]
    branch_count: usize,
    #[pyo3(get)]
    staged_files: Vec<(String, String)>,
    #[pyo3(get)]
    patch_count: usize,
}

#[pymethods]
impl RepoStatus {
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
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    message: String,
    #[pyo3(get)]
    author: String,
    #[pyo3(get)]
    timestamp: i64,
    #[pyo3(get)]
    parents: Vec<String>,
}

#[pymethods]
impl PyLogEntry {
    fn __repr__(&self) -> String {
        let short_id = if self.id.len() >= 12 {
            &self.id[..12]
        } else {
            &self.id
        };
        format!("LogEntry({}...: {})", short_id, self.message)
    }
}

/// A diff entry.
#[pyclass]
#[derive(Clone)]
struct PyDiffEntry {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    diff_type: String,
    #[pyo3(get)]
    old_hash: Option<String>,
    #[pyo3(get)]
    new_hash: Option<String>,
}

#[pymethods]
impl PyDiffEntry {
    fn __repr__(&self) -> String {
        format!("DiffEntry({} {})", self.diff_type, self.path)
    }
}

/// Merge result.
#[pyclass]
struct PyMergeResult {
    #[pyo3(get)]
    is_clean: bool,
    #[pyo3(get)]
    merge_patch_id: Option<String>,
    #[pyo3(get)]
    patches_applied: usize,
    #[pyo3(get)]
    conflicts: Vec<PyConflictInfo>,
}

/// Conflict info.
#[pyclass]
#[derive(Clone)]
struct PyConflictInfo {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    our_patch_id: String,
    #[pyo3(get)]
    their_patch_id: String,
}

/// Rebase result.
#[pyclass]
struct PyRebaseResult {
    #[pyo3(get)]
    patches_replayed: usize,
    #[pyo3(get)]
    new_tip: String,
}

/// GC result.
#[pyclass]
struct PyGcResult {
    #[pyo3(get)]
    patches_removed: usize,
}

/// FSCK result.
#[pyclass]
struct PyFsckResult {
    #[pyo3(get)]
    checks_passed: usize,
    #[pyo3(get)]
    warnings: Vec<String>,
    #[pyo3(get)]
    errors: Vec<String>,
}

/// Stash entry.
#[pyclass]
#[derive(Clone)]
struct PyStashEntry {
    #[pyo3(get)]
    index: usize,
    #[pyo3(get)]
    message: String,
    #[pyo3(get)]
    branch: String,
    #[pyo3(get)]
    head_id: String,
}

/// Module-level functions.
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
    Ok(())
}
