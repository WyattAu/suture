//! The Suture Repository — high-level API for version control operations.
//!
//! A Repository combines:
//! - `BlobStore` (CAS) for content-addressed blob storage
//! - `PatchDag` (in-memory) for patch history
//! - `MetadataStore` (SQLite) for persistent metadata
//! - `Patch Application Engine` for reconstructing file trees
//!
//! # Repository Layout
//!
//! ```text
//! my-project/
//!   .suture/
//!     objects/        # CAS blob storage
//!     metadata.db     # SQLite metadata
//!     HEAD            # Current branch reference
//! ```
//!
//! .sutureignore (in repo root):
//!   build/
//!   *.o
//!   target/

use crate::cas::store::{BlobStore, CasError};
use crate::dag::graph::{DagError, PatchDag};
use crate::engine::apply::{apply_patch_chain, resolve_payload_to_hash, ApplyError};
use crate::engine::diff::{diff_trees, DiffEntry, DiffType};
use crate::engine::tree::FileTree;
use crate::metadata::MetaError;
use crate::patch::merge::MergeResult;
use crate::patch::types::{OperationType, Patch, PatchId, TouchSet};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use suture_common::{BranchName, CommonError, FileStatus, Hash, RepoPath};
use thiserror::Error;

/// Repository errors.
#[derive(Error, Debug)]
pub enum RepoError {
    #[error("not a suture repository: {0}")]
    NotARepository(PathBuf),

    #[error("repository already exists: {0}")]
    AlreadyExists(PathBuf),

    #[error("CAS error: {0}")]
    Cas(#[from] CasError),

    #[error("DAG error: {0}")]
    Dag(#[from] DagError),

    #[error("metadata error: {0}")]
    Meta(#[from] MetaError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("patch application error: {0}")]
    Apply(#[from] ApplyError),

    #[error("patch error: {0}")]
    Patch(String),

    #[error("nothing to commit")]
    NothingToCommit,

    #[error("uncommitted changes would be overwritten (staged: {0})")]
    DirtyWorkingTree(usize),

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("common error: {0}")]
    Common(#[from] CommonError),

    #[error("{0}")]
    Custom(String),
}

/// The Suture Repository.
pub struct Repository {
    /// Path to the repository root (the directory containing `.suture/`).
    root: PathBuf,
    /// Path to the `.suture/` directory.
    #[allow(dead_code)]
    suture_dir: PathBuf,
    /// Content Addressable Storage.
    cas: BlobStore,
    /// In-memory Patch DAG.
    dag: PatchDag,
    /// Persistent metadata store.
    meta: crate::metadata::MetadataStore,
    /// Current author name.
    author: String,
    /// Parsed ignore patterns.
    ignore_patterns: Vec<String>,
}

impl Repository {
    /// Initialize a new Suture repository at the given path.
    pub fn init(path: &Path, author: &str) -> Result<Self, RepoError> {
        let suture_dir = path.join(".suture");
        if suture_dir.exists() {
            return Err(RepoError::AlreadyExists(path.to_path_buf()));
        }

        // Create directory structure
        fs::create_dir_all(suture_dir.join("objects"))?;

        // Initialize CAS
        let cas = BlobStore::new(&suture_dir)?;

        // Initialize metadata
        let meta = crate::metadata::MetadataStore::open(&suture_dir.join("metadata.db"))?;

        // Create the in-memory DAG
        let mut dag = PatchDag::new();

        // Create root commit
        let root_patch = Patch::new(
            OperationType::Create,
            TouchSet::empty(),
            None,
            vec![],
            vec![],
            author.to_string(),
            "Initial commit".to_string(),
        );
        let root_id = dag.add_patch(root_patch.clone(), vec![])?;

        // Persist root patch
        meta.store_patch(&root_patch)?;

        // Create default branch
        let main_branch = BranchName::new("main").unwrap();
        dag.create_branch(main_branch.clone(), root_id)?;
        meta.set_branch(&main_branch, &root_id)?;

        // Store author config
        meta.set_config("author", author)?;

        // Load ignore patterns
        let ignore_patterns = load_ignore_patterns(path);

        Ok(Self {
            root: path.to_path_buf(),
            suture_dir,
            cas,
            dag,
            meta,
            author: author.to_string(),
            ignore_patterns,
        })
    }

    /// Open an existing Suture repository.
    ///
    /// Reconstructs the full DAG from the metadata database by loading
    /// all stored patches and their edges.
    pub fn open(path: &Path) -> Result<Self, RepoError> {
        let suture_dir = path.join(".suture");
        if !suture_dir.exists() {
            return Err(RepoError::NotARepository(path.to_path_buf()));
        }

        let cas = BlobStore::new(&suture_dir)?;
        let meta = crate::metadata::MetadataStore::open(&suture_dir.join("metadata.db"))?;

        // Reconstruct DAG from metadata — load ALL patches
        let mut dag = PatchDag::new();

        // Collect all patch IDs from the patches table
        let all_patch_ids: Vec<PatchId> = {
            let mut stmt = meta
                .conn()
                .prepare("SELECT id FROM patches ORDER BY id")
                .map_err(|e: rusqlite::Error| RepoError::Custom(e.to_string()))?;
            let rows = stmt
                .query_map([], |row: &rusqlite::Row| row.get::<_, String>(0))
                .map_err(|e: rusqlite::Error| RepoError::Custom(e.to_string()))?;
            rows.filter_map(|r: Result<String, _>| r.ok())
                .filter_map(|hex| Hash::from_hex(&hex).ok())
                .collect()
        };

        // Load each patch and add to DAG, parents first
        let mut loaded: HashSet<PatchId> = HashSet::new();
        let mut attempts = 0;
        while loaded.len() < all_patch_ids.len() && attempts < all_patch_ids.len() + 1 {
            for patch_id in &all_patch_ids {
                if loaded.contains(patch_id) {
                    continue;
                }
                if let Ok(patch) = meta.get_patch(patch_id) {
                    // Check if all parents are loaded
                    let parents_ready = patch
                        .parent_ids
                        .iter()
                        .all(|pid| loaded.contains(pid) || *pid == Hash::ZERO);
                    if parents_ready {
                        // Filter out non-existent parents (root commits)
                        let valid_parents: Vec<PatchId> = patch
                            .parent_ids
                            .iter()
                            .filter(|pid| loaded.contains(pid))
                            .copied()
                            .collect();
                        let _ = dag.add_patch(patch, valid_parents);
                        loaded.insert(*patch_id);
                    }
                }
            }
            attempts += 1;
        }

        // Recreate branches
        let branches = meta.list_branches()?;
        for (name, target_id) in &branches {
            let branch_name = match BranchName::new(name) {
                Ok(b) => b,
                Err(_) => continue,
            };
            if !dag.branch_exists(&branch_name) {
                let _ = dag.create_branch(branch_name, *target_id);
            }
        }

        let author = meta
            .get_config("author")
            .unwrap_or(None)
            .unwrap_or_else(|| "unknown".to_string());

        // Load ignore patterns
        let ignore_patterns = load_ignore_patterns(path);

        Ok(Self {
            root: path.to_path_buf(),
            suture_dir,
            cas,
            dag,
            meta,
            author,
            ignore_patterns,
        })
    }

    // =========================================================================
    // Branch Operations
    // =========================================================================

    /// Create a new branch.
    pub fn create_branch(&mut self, name: &str, target: Option<&str>) -> Result<(), RepoError> {
        let branch = BranchName::new(name)?;
        let target_id = match target {
            Some(t) => {
                if let Ok(bn) = BranchName::new(t) {
                    self.dag
                        .get_branch(&bn)
                        .ok_or_else(|| RepoError::BranchNotFound(t.to_string()))?
                } else {
                    Hash::from_hex(t)
                        .map_err(|_| RepoError::Custom(format!("invalid target: {}", t)))?
                }
            }
            None => {
                let head = self
                    .dag
                    .head()
                    .ok_or_else(|| RepoError::Custom("no HEAD branch".to_string()))?;
                head.1
            }
        };

        self.dag.create_branch(branch.clone(), target_id)?;
        self.meta.set_branch(&branch, &target_id)?;
        Ok(())
    }

    /// Get the current branch and its target.
    ///
    /// Reads the `head_branch` config key to determine which branch is
    /// currently checked out. Falls back to "main" if not set.
    pub fn head(&self) -> Result<(String, PatchId), RepoError> {
        let branch_name = self
            .meta
            .get_config("head_branch")
            .unwrap_or(None)
            .unwrap_or_else(|| "main".to_string());

        let bn = BranchName::new(&branch_name)?;
        let target_id = self
            .dag
            .get_branch(&bn)
            .ok_or_else(|| RepoError::BranchNotFound(branch_name.clone()))?;

        Ok((branch_name, target_id))
    }

    /// List all branches.
    pub fn list_branches(&self) -> Vec<(String, PatchId)> {
        self.dag.list_branches()
    }

    // =========================================================================
    // Staging & Commit
    // =========================================================================

    /// Get repository status.
    pub fn status(&self) -> Result<RepoStatus, RepoError> {
        let working_set = self.meta.working_set()?;
        let branches = self.list_branches();
        let head = self.dag.head();

        Ok(RepoStatus {
            head_branch: head.as_ref().map(|(name, _)| name.clone()),
            head_patch: head.as_ref().map(|(_, id)| *id),
            branch_count: branches.len(),
            staged_files: working_set
                .iter()
                .filter(|(_, s)| {
                    matches!(
                        s,
                        FileStatus::Added | FileStatus::Modified | FileStatus::Deleted
                    )
                })
                .map(|(p, s)| (p.clone(), *s))
                .collect(),
            patch_count: self.dag.patch_count(),
        })
    }

    /// Add a file to the staging area (working set).
    pub fn add(&self, path: &str) -> Result<(), RepoError> {
        let repo_path = RepoPath::new(path)?;
        let full_path = self.root.join(path);

        if !full_path.exists() {
            return Err(RepoError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!("file not found: {}", path),
            )));
        }

        let status = if self.is_tracked(path)? {
            FileStatus::Modified
        } else {
            FileStatus::Added
        };

        self.meta.working_set_add(&repo_path, status)?;
        Ok(())
    }

    /// Add all files (respecting .sutureignore).
    pub fn add_all(&self) -> Result<usize, RepoError> {
        let tree = self.snapshot_head()?;
        let mut count = 0;

        for entry in walk_dir(&self.root, &self.ignore_patterns)? {
            let rel_path = entry.relative;
            let full_path = self.root.join(&rel_path);

            let is_tracked = tree.contains(&rel_path);

            // Check if file has changed
            if is_tracked
                && let Ok(data) = fs::read(&full_path)
                && let Some(old_hash) = tree.get(&rel_path)
                && Hash::from_data(&data) == *old_hash
            {
                continue; // Unchanged
            }

            let status = if is_tracked {
                FileStatus::Modified
            } else {
                FileStatus::Added
            };

            let repo_path = RepoPath::new(&rel_path)?;
            self.meta.working_set_add(&repo_path, status)?;
            count += 1;
        }

        Ok(count)
    }

    /// Check if a path is tracked.
    fn is_tracked(&self, path: &str) -> Result<bool, RepoError> {
        for id in self.dag.patch_ids() {
            if let Some(node) = self.dag.get_node(&id)
                && node.patch.target_path.as_deref() == Some(path)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Create a commit from the working set.
    pub fn commit(&mut self, message: &str) -> Result<PatchId, RepoError> {
        let working_set = self.meta.working_set()?;

        let staged: Vec<_> = working_set
            .iter()
            .filter(|(_, s)| {
                matches!(
                    s,
                    FileStatus::Added | FileStatus::Modified | FileStatus::Deleted
                )
            })
            .collect();

        if staged.is_empty() {
            return Err(RepoError::NothingToCommit);
        }

        let (branch_name, head_id) = self.head()?;

        let mut parent_ids = vec![head_id];
        let mut last_patch_id = head_id;

        for (path, status) in &staged {
            let full_path = self.root.join(path);

            let (op_type, payload, touch_set) = match status {
                FileStatus::Added | FileStatus::Modified => {
                    let data = fs::read(&full_path)?;
                    let hash = self.cas.put_blob(&data)?;
                    let payload = hash.to_hex().as_bytes().to_vec();
                    let touch_set = TouchSet::single(path.clone());
                    (OperationType::Modify, payload, touch_set)
                }
                FileStatus::Deleted => {
                    let touch_set = TouchSet::single(path.clone());
                    (OperationType::Delete, vec![], touch_set)
                }
                _ => continue,
            };

            let patch = Patch::new(
                op_type,
                touch_set,
                Some(path.clone()),
                payload,
                parent_ids.clone(),
                self.author.clone(),
                message.to_string(),
            );

            let patch_id = self.dag.add_patch(patch.clone(), parent_ids.clone())?;
            self.meta.store_patch(&patch)?;

            last_patch_id = patch_id;
            parent_ids = vec![patch_id];

            let repo_path = RepoPath::new(path.clone())?;
            self.meta.working_set_remove(&repo_path)?;
        }

        let branch = BranchName::new(&branch_name)?;
        self.dag.update_branch(&branch, last_patch_id)?;
        self.meta.set_branch(&branch, &last_patch_id)?;

        Ok(last_patch_id)
    }

    // =========================================================================
    // Snapshot & Checkout
    // =========================================================================

    /// Build a FileTree snapshot for the HEAD commit.
    ///
    /// Applies all patches from root to HEAD tip to produce the current
    /// file tree state.
    pub fn snapshot_head(&self) -> Result<FileTree, RepoError> {
        let (_, head_id) = self.head()?;
        self.snapshot(&head_id)
    }

    /// Build a FileTree snapshot for a specific patch.
    ///
    /// Applies all patches from root to the given patch ID.
    pub fn snapshot(&self, patch_id: &PatchId) -> Result<FileTree, RepoError> {
        let mut chain = self.dag.patch_chain(patch_id);
        // patch_chain returns [tip, parent, ..., root] — reverse for oldest-first
        chain.reverse();
        let patches: Vec<Patch> = chain
            .iter()
            .filter_map(|id| self.dag.get_patch(id).cloned())
            .collect();

        let tree = apply_patch_chain(&patches, resolve_payload_to_hash)?;
        Ok(tree)
    }

    /// Checkout a branch, updating the working tree to match its tip state.
    ///
    /// This operation:
    /// 1. Builds the target FileTree from the branch's patch chain
    /// 2. Compares against the current working tree
    /// 3. Updates files (add/modify/delete) to match the target
    /// 4. Updates the HEAD reference
    ///
    /// Refuses to checkout if there are uncommitted staged changes.
    pub fn checkout(&mut self, branch_name: &str) -> Result<FileTree, RepoError> {
        let target = BranchName::new(branch_name)?;

        // Verify branch exists
        let target_id = self
            .dag
            .get_branch(&target)
            .ok_or_else(|| RepoError::BranchNotFound(branch_name.to_string()))?;

        // Check for uncommitted changes
        let working_set = self.meta.working_set()?;
        let staged_count = working_set
            .iter()
            .filter(|(_, s)| {
                matches!(
                    s,
                    FileStatus::Added | FileStatus::Modified | FileStatus::Deleted
                )
            })
            .count();
        if staged_count > 0 {
            return Err(RepoError::DirtyWorkingTree(staged_count));
        }

        // Build target file tree
        let target_tree = self.snapshot(&target_id)?;

        // Build current file tree (if we can)
        let current_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());

        // Compute diff: what needs to change on disk
        let diffs = diff_trees(&current_tree, &target_tree);

        // Apply changes to the working tree
        for entry in &diffs {
            let full_path = self.root.join(&entry.path);
            match &entry.diff_type {
                DiffType::Added | DiffType::Modified => {
                    if let Some(new_hash) = &entry.new_hash {
                        let blob = self.cas.get_blob(new_hash)?;
                        if let Some(parent) = full_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(&full_path, &blob)?;
                    }
                }
                DiffType::Deleted => {
                    if full_path.exists() {
                        fs::remove_file(&full_path)?;
                    }
                }
                DiffType::Renamed { old_path, .. } => {
                    let old_full = self.root.join(old_path);
                    if old_full.exists() {
                        if let Some(parent) = full_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::rename(&old_full, &full_path)?;
                    }
                }
            }
        }

        // Clean up files that exist on disk but not in the target tree
        // (files that were in the old tree but not in the new tree,
        //  and aren't already handled by the diff)
        for (path, _) in current_tree.iter() {
            if !target_tree.contains(path) {
                let full_path = self.root.join(path);
                if full_path.exists() {
                    let _ = fs::remove_file(&full_path);
                }
            }
        }

        // Update HEAD: record which branch is checked out in config.
        // Do NOT move the target branch pointer — checkout only changes
        // the working tree and which branch HEAD refers to.
        self.meta
            .set_config("head_branch", branch_name)
            .map_err(RepoError::Meta)?;

        Ok(target_tree)
    }

    // =========================================================================
    // Diff
    // =========================================================================

    /// Compute the diff between two commits or branches.
    ///
    /// If `from` is None, compares the empty tree to `to`.
    pub fn diff(&self, from: Option<&str>, to: Option<&str>) -> Result<Vec<DiffEntry>, RepoError> {
        let resolve_id = |name: &str| -> Result<PatchId, RepoError> {
            // Try hex hash first (patch IDs are 64-char hex strings that
            // also happen to pass BranchName validation, so we must try
            // hex before branch name to avoid false branch lookups).
            if let Ok(hash) = Hash::from_hex(name)
                && self.dag.has_patch(&hash)
            {
                return Ok(hash);
            }
            // Fall back to branch name
            let bn = BranchName::new(name)?;
            self.dag
                .get_branch(&bn)
                .ok_or_else(|| RepoError::BranchNotFound(name.to_string()))
        };

        let old_tree = match from {
            Some(f) => self.snapshot(&resolve_id(f)?)?,
            None => FileTree::empty(),
        };

        let new_tree = match to {
            Some(t) => self.snapshot(&resolve_id(t)?)?,
            None => self.snapshot_head()?,
        };

        Ok(diff_trees(&old_tree, &new_tree))
    }

    // =========================================================================
    // Revert
    // =========================================================================

    /// Revert a commit by creating a new patch that undoes its changes.
    ///
    /// The revert creates inverse patches (Delete for Create, etc.)
    /// and commits them on top of HEAD.
    pub fn revert(
        &mut self,
        patch_id: &PatchId,
        message: Option<&str>,
    ) -> Result<PatchId, RepoError> {
        let patch = self
            .dag
            .get_patch(patch_id)
            .ok_or_else(|| RepoError::Custom(format!("patch not found: {}", patch_id)))?;

        let (branch_name, head_id) = self.head()?;
        let msg = message
            .map(|m| m.to_string())
            .unwrap_or_else(|| format!("Revert {}", patch_id));

        match patch.operation_type {
            OperationType::Create | OperationType::Modify => {
                // Create a Delete patch to undo the change
                let revert_patch = Patch::new(
                    OperationType::Delete,
                    patch.touch_set.clone(),
                    patch.target_path.clone(),
                    vec![],
                    vec![head_id],
                    self.author.clone(),
                    msg,
                );

                let revert_id = self.dag.add_patch(revert_patch.clone(), vec![head_id])?;
                self.meta.store_patch(&revert_patch)?;

                let branch = BranchName::new(&branch_name)?;
                self.dag.update_branch(&branch, revert_id)?;
                self.meta.set_branch(&branch, &revert_id)?;

                Ok(revert_id)
            }
            OperationType::Delete => {
                // For a Delete revert, we need the blob from the parent's tree.
                // Re-create by finding the file in the parent snapshot.
                if let Some(parent_id) = patch.parent_ids.first() {
                    let parent_tree = self.snapshot(parent_id)?;
                    if let Some(path) = &patch.target_path
                        && let Some(hash) = parent_tree.get(path)
                    {
                        let payload = hash.to_hex().as_bytes().to_vec();
                        let revert_patch = Patch::new(
                            OperationType::Modify,
                            patch.touch_set.clone(),
                            patch.target_path.clone(),
                            payload,
                            vec![head_id],
                            self.author.clone(),
                            msg,
                        );

                        let revert_id =
                            self.dag.add_patch(revert_patch.clone(), vec![head_id])?;
                        self.meta.store_patch(&revert_patch)?;

                        let branch = BranchName::new(&branch_name)?;
                        self.dag.update_branch(&branch, revert_id)?;
                        self.meta.set_branch(&branch, &revert_id)?;

                        return Ok(revert_id);
                    }
                }
                Err(RepoError::Custom(
                    "cannot revert delete: original file content not found".into(),
                ))
            }
            _ => Err(RepoError::Custom(format!(
                "cannot revert {:?} patches",
                patch.operation_type
            ))),
        }
    }

    // =========================================================================
    // Merge
    // =========================================================================

    /// Compute a merge plan between two branches.
    pub fn merge_plan(&self, branch_a: &str, branch_b: &str) -> Result<MergeResult, RepoError> {
        let ba = BranchName::new(branch_a)?;
        let bb = BranchName::new(branch_b)?;
        self.dag.merge_branches(&ba, &bb).map_err(RepoError::Dag)
    }

    // =========================================================================
    // Log
    // =========================================================================

    /// Get the patch history (log) for a branch.
    pub fn log(&self, branch: Option<&str>) -> Result<Vec<Patch>, RepoError> {
        let target_id = match branch {
            Some(name) => {
                let bn = BranchName::new(name)?;
                self.dag
                    .get_branch(&bn)
                    .ok_or_else(|| RepoError::BranchNotFound(name.to_string()))?
            }
            None => {
                let (_, id) = self.head()?;
                id
            }
        };

        let chain = self.dag.patch_chain(&target_id);
        let mut patches = Vec::new();
        for id in chain {
            if let Some(node) = self.dag.get_node(&id) {
                patches.push(node.patch.clone());
            }
        }
        Ok(patches)
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Get the repository root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get a reference to the DAG.
    pub fn dag(&self) -> &PatchDag {
        &self.dag
    }

    /// Get a mutable reference to the DAG.
    pub fn dag_mut(&mut self) -> &mut PatchDag {
        &mut self.dag
    }

    /// Get a reference to the metadata store.
    pub fn meta(&self) -> &crate::metadata::MetadataStore {
        &self.meta
    }

    /// Get a reference to the CAS.
    pub fn cas(&self) -> &BlobStore {
        &self.cas
    }
}

// =============================================================================
// .sutureignore Support
// =============================================================================

/// Load and parse .sutureignore patterns from the repository root.
fn load_ignore_patterns(root: &Path) -> Vec<String> {
    let ignore_file = root.join(".sutureignore");
    if !ignore_file.exists() {
        return Vec::new();
    }

    fs::read_to_string(&ignore_file)
        .unwrap_or_default()
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// Check if a relative path matches any ignore pattern.
fn is_ignored(rel_path: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if let Some(suffix) = pattern.strip_prefix('*') {
            // Suffix match: "*.o" matches "foo.o"
            if rel_path.ends_with(suffix) {
                return true;
            }
        } else if pattern.ends_with('/') {
            // Directory prefix match: "build/" matches "build/output.o"
            if rel_path.starts_with(pattern) {
                return true;
            }
        } else {
            // Exact match or path component match
            if rel_path == pattern || rel_path.starts_with(&format!("{}/", pattern)) {
                return true;
            }
        }
    }
    false
}

/// A file entry found while walking the repository.
struct WalkEntry {
    relative: String,
    #[allow(dead_code)]
    full_path: PathBuf,
}

/// Walk the repository directory, collecting files and respecting .sutureignore.
fn walk_dir(root: &Path, ignore_patterns: &[String]) -> Result<Vec<WalkEntry>, io::Error> {
    let mut entries = Vec::new();
    walk_dir_recursive(root, root, ignore_patterns, &mut entries)?;
    Ok(entries)
}

fn walk_dir_recursive(
    root: &Path,
    current: &Path,
    ignore_patterns: &[String],
    entries: &mut Vec<WalkEntry>,
) -> Result<(), io::Error> {
    if !current.is_dir() {
        return Ok(());
    }

    let mut dir_entries: Vec<_> = fs::read_dir(current)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            // Skip .suture directory
            let name = e.file_name();
            name != ".suture"
        })
        .collect();

    dir_entries.sort_by_key(|e| e.file_name());

    for entry in dir_entries {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        // Skip ignored paths
        if is_ignored(&rel, ignore_patterns) {
            continue;
        }

        if path.is_dir() {
            walk_dir_recursive(root, &path, ignore_patterns, entries)?;
        } else if path.is_file() {
            entries.push(WalkEntry {
                relative: rel,
                full_path: path,
            });
        }
    }

    Ok(())
}

// =============================================================================
// Repository Status
// =============================================================================

/// Repository status information.
#[derive(Debug, Clone)]
pub struct RepoStatus {
    /// Current HEAD branch name.
    pub head_branch: Option<String>,
    /// Current HEAD patch ID.
    pub head_patch: Option<PatchId>,
    /// Number of branches.
    pub branch_count: usize,
    /// Staged files (path, status).
    pub staged_files: Vec<(String, FileStatus)>,
    /// Total number of patches in the DAG.
    pub patch_count: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_and_open() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let _repo = Repository::init(repo_path, "alice").unwrap();
        assert!(repo_path.join(".suture").exists());
        assert!(repo_path.join(".suture/metadata.db").exists());

        // Open the same repo
        let repo2 = Repository::open(repo_path).unwrap();
        assert_eq!(repo2.list_branches().len(), 1);
    }

    #[test]
    fn test_init_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        Repository::init(dir.path(), "alice").unwrap();
        let result = Repository::init(dir.path(), "alice");
        assert!(matches!(result, Err(RepoError::AlreadyExists(_))));
    }

    #[test]
    fn test_create_branch() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        repo.create_branch("feature", None).unwrap();
        assert_eq!(repo.list_branches().len(), 2);

        let result = repo.create_branch("feature", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_and_status() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice").unwrap();

        let test_file = dir.path().join("hello.txt");
        fs::write(&test_file, "hello, suture!").unwrap();

        repo.add("hello.txt").unwrap();
        let status = repo.status().unwrap();
        assert_eq!(status.staged_files.len(), 1);
        assert_eq!(status.staged_files[0].0, "hello.txt");
        assert_eq!(status.staged_files[0].1, FileStatus::Added);
    }

    #[test]
    fn test_add_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice").unwrap();
        let result = repo.add("does_not_exist.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_commit() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        repo.add("test.txt").unwrap();

        let patch_id = repo.commit("initial file").unwrap();

        let status = repo.status().unwrap();
        assert!(status.staged_files.is_empty());
        assert!(repo.dag.has_patch(&patch_id));
        assert_eq!(repo.dag.patch_count(), 2);
    }

    #[test]
    fn test_commit_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();
        let result = repo.commit("empty commit");
        assert!(matches!(result, Err(RepoError::NothingToCommit)));
    }

    #[test]
    fn test_log() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "v1").unwrap();
        repo.add("test.txt").unwrap();
        repo.commit("first commit").unwrap();

        fs::write(&test_file, "v2").unwrap();
        repo.add("test.txt").unwrap();
        repo.commit("second commit").unwrap();

        let log = repo.log(None).unwrap();
        assert_eq!(log.len(), 3); // root + 2 commits
    }

    #[test]
    fn test_snapshot_head() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "hello world").unwrap();
        repo.add("test.txt").unwrap();
        repo.commit("add test.txt").unwrap();

        let tree = repo.snapshot_head().unwrap();
        assert!(tree.contains("test.txt"));
        assert_eq!(tree.get("test.txt"), Some(&Hash::from_data(b"hello world")));
    }

    #[test]
    fn test_snapshot_empty_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice").unwrap();

        let tree = repo.snapshot_head().unwrap();
        assert!(tree.is_empty());
    }

    #[test]
    fn test_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        // Commit a file on main
        let main_file = dir.path().join("main.txt");
        fs::write(&main_file, "main content").unwrap();
        repo.add("main.txt").unwrap();
        repo.commit("add main.txt").unwrap();

        // Create feature branch and add different file
        let (_, head_id) = repo.head().unwrap();
        let feat_patch = Patch::new(
            OperationType::Modify,
            TouchSet::single("feature.txt"),
            Some("feature.txt".to_string()),
            Hash::from_data(b"feature content")
                .to_hex()
                .as_bytes()
                .to_vec(),
            vec![head_id],
            "alice".to_string(),
            "add feature.txt".to_string(),
        );
        let _feat_id = repo
            .dag_mut()
            .add_patch(feat_patch.clone(), vec![head_id])
            .unwrap();
        repo.meta.store_patch(&feat_patch).unwrap();

        // Checkout main (should remove feature.txt if it exists)
        repo.checkout("main").unwrap();
        assert!(!dir.path().join("feature.txt").exists());
        assert!(dir.path().join("main.txt").exists());
    }

    #[test]
    fn test_checkout_refuses_dirty() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        // Stage a file but don't commit
        let staged = dir.path().join("staged.txt");
        fs::write(&staged, "staged").unwrap();
        repo.add("staged.txt").unwrap();

        // Checkout should fail
        let result = repo.checkout("main");
        assert!(matches!(result, Err(RepoError::DirtyWorkingTree(_))));
    }

    #[test]
    fn test_diff() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "v1").unwrap();
        repo.add("test.txt").unwrap();
        let first_commit = repo.commit("first").unwrap();

        fs::write(&test_file, "v2").unwrap();
        repo.add("test.txt").unwrap();
        repo.commit("second").unwrap();

        // Diff between first commit and HEAD
        let diffs = repo.diff(Some(&first_commit.to_hex()), None).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].diff_type, DiffType::Modified);
    }

    #[test]
    fn test_revert() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "original").unwrap();
        repo.add("test.txt").unwrap();
        let commit_id = repo.commit("add file").unwrap();

        // Revert the commit
        repo.revert(&commit_id, None).unwrap();

        // The file should still exist on disk (revert only adds a patch to the DAG)
        // but the snapshot should no longer include it
        let tree = repo.snapshot_head().unwrap();
        assert!(!tree.contains("test.txt"));
    }

    #[test]
    fn test_open_reconstructs_full_dag() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        // Create a chain of commits
        let f = dir.path().join("f.txt");
        fs::write(&f, "v1").unwrap();
        repo.add("f.txt").unwrap();
        repo.commit("first").unwrap();

        fs::write(&f, "v2").unwrap();
        repo.add("f.txt").unwrap();
        repo.commit("second").unwrap();

        fs::write(&f, "v3").unwrap();
        repo.add("f.txt").unwrap();
        repo.commit("third").unwrap();

        let original_count = repo.dag.patch_count();

        // Open and verify full DAG is reconstructed
        let repo2 = Repository::open(dir.path()).unwrap();
        assert_eq!(repo2.dag.patch_count(), original_count);

        let log = repo2.log(None).unwrap();
        assert_eq!(log.len(), 4); // root + 3 commits
    }

    #[test]
    fn test_ignore_patterns() {
        let patterns = vec![
            "target/".to_string(),
            "*.o".to_string(),
            "build".to_string(),
        ];

        assert!(is_ignored("target/debug/main", &patterns));
        assert!(is_ignored("foo.o", &patterns));
        assert!(is_ignored("build/output", &patterns));
        assert!(is_ignored("build", &patterns));
        assert!(!is_ignored("src/main.rs", &patterns));
        assert!(!is_ignored("main.rs", &patterns));
    }

    #[test]
    fn test_full_workflow_with_checkout() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        // Commit file A on main
        fs::write(dir.path().join("a.txt"), "version 1")?;
        repo.add("a.txt")?;
        repo.commit("add a.txt v1")?;

        // Create feature branch
        repo.create_branch("feature", None)?;

        // Modify A and add B on main
        fs::write(dir.path().join("a.txt"), "version 2")?;
        fs::write(dir.path().join("b.txt"), "new file")?;
        repo.add("a.txt")?;
        repo.add("b.txt")?;
        repo.commit("modify a, add b")?;

        // Checkout feature (should have a.txt v1, no b.txt)
        repo.checkout("feature")?;
        let content = fs::read_to_string(dir.path().join("a.txt"))?;
        assert_eq!(content, "version 1");
        assert!(!dir.path().join("b.txt").exists());

        Ok(())
    }

    #[test]
    fn test_add_all() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice").unwrap();

        fs::write(dir.path().join("a.txt"), "a")?;
        fs::write(dir.path().join("b.txt"), "b")?;
        // .suture is auto-ignored
        let count = repo.add_all().unwrap();
        assert_eq!(count, 2);
        Ok(())
    }
}
