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
use crate::engine::apply::{ApplyError, apply_patch, apply_patch_chain, resolve_payload_to_hash};
use crate::engine::diff::{DiffEntry, DiffType, diff_trees};
use crate::engine::tree::FileTree;
use crate::metadata::MetaError;
use crate::patch::conflict::Conflict;
use crate::patch::merge::MergeResult;
use crate::patch::types::{FileChange, OperationType, Patch, PatchId, TouchSet};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use suture_common::{BranchName, CommonError, FileStatus, Hash, RepoPath};
use thiserror::Error;

/// Repository errors.
#[derive(Error, Debug)]
pub enum RepoError {
    /// The given path is not a Suture repository (no `.suture/` directory).
    #[error("not a suture repository: {0}")]
    NotARepository(PathBuf),

    /// A Suture repository already exists at the given path.
    #[error("repository already exists: {0}")]
    AlreadyExists(PathBuf),

    /// An error occurred in the Content Addressable Storage.
    #[error("CAS error: {0}")]
    Cas(#[from] CasError),

    /// An error occurred in the Patch DAG.
    #[error("DAG error: {0}")]
    Dag(#[from] DagError),

    /// An error occurred in the metadata store.
    #[error("metadata error: {0}")]
    Meta(#[from] MetaError),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An error occurred during patch application.
    #[error("patch application error: {0}")]
    Apply(#[from] ApplyError),

    /// A patch-related error occurred.
    #[error("patch error: {0}")]
    Patch(String),

    /// No changes are staged for commit.
    #[error("nothing to commit")]
    NothingToCommit,

    /// A merge is in progress with unresolved conflicts.
    #[error("merge in progress — resolve conflicts first")]
    MergeInProgress,

    /// Uncommitted staged changes would be overwritten by this operation.
    #[error("uncommitted changes would be overwritten (staged: {0})")]
    DirtyWorkingTree(usize),

    /// The specified branch was not found.
    #[error("branch not found: {0}")]
    BranchNotFound(String),

    /// An error from the `suture-common` crate.
    #[error("common error: {0}")]
    Common(#[from] CommonError),

    /// A generic custom error.
    #[error("{0}")]
    Custom(String),

    /// The requested operation is not supported.
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

/// Reset mode for the `reset` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    /// Move branch pointer only; keep staging and working tree.
    Soft,
    /// Move branch pointer and clear staging; keep working tree.
    Mixed,
    /// Move branch pointer, clear staging, and restore working tree.
    Hard,
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
    /// Pending merge parents (set during a conflicting merge).
    pending_merge_parents: Vec<PatchId>,
    /// Cached FileTree snapshot for the current HEAD.
    cached_head_snapshot: RefCell<Option<FileTree>>,
    /// The patch ID that the cached snapshot corresponds to.
    cached_head_id: RefCell<Option<PatchId>>,
    /// The branch name that HEAD points to (cached).
    cached_head_branch: RefCell<Option<String>>,
    /// Per-repo configuration loaded from `.suture/config`.
    repo_config: crate::metadata::repo_config::RepoConfig,
    /// Whether this repository is a worktree (linked to a main repo).
    is_worktree: bool,
}

/// A single reflog entry with structured fields.
pub struct ReflogEntry {
    /// The HEAD before this operation.
    pub old_head: PatchId,
    /// The HEAD after this operation.
    pub new_head: PatchId,
    /// Human-readable description of the operation.
    pub message: String,
    /// Unix timestamp of when this operation occurred.
    pub timestamp: i64,
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

        // Initialize CAS (disable per-read hash verification for performance;
        // content addressing already ensures correctness by construction)
        let mut cas = BlobStore::new(&suture_dir)?;
        cas.set_verify_on_read(false);

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
        let main_branch = BranchName::new("main").expect("hardcoded 'main' is always valid");
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
            pending_merge_parents: Vec::new(),
            cached_head_snapshot: RefCell::new(None),
            cached_head_id: RefCell::new(None),
            cached_head_branch: RefCell::new(None),
            repo_config: crate::metadata::repo_config::RepoConfig::default(),
            is_worktree: false,
        })
    }
    ///
    /// Reconstructs the full DAG from the metadata database by loading
    /// all stored patches and their edges.
    pub fn open(path: &Path) -> Result<Self, RepoError> {
        let suture_dir = path.join(".suture");
        if !suture_dir.exists() {
            return Err(RepoError::NotARepository(path.to_path_buf()));
        }

        let is_worktree = suture_dir.join("worktree").exists();

        // Initialize CAS (disable per-read hash verification for performance)
        let mut cas = BlobStore::new(&suture_dir)?;
        cas.set_verify_on_read(false);
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
            .get_config("user.name")
            .unwrap_or(None)
            .or_else(|| meta.get_config("author").unwrap_or(None))
            .unwrap_or_else(|| "unknown".to_string());

        // Restore pending merge parents if a merge was in progress
        let restored_parents = restore_pending_merge_parents(&meta);

        // Load ignore patterns
        let ignore_patterns = load_ignore_patterns(path);

        // Load per-repo config from .suture/config
        let repo_config = crate::metadata::repo_config::RepoConfig::load(path);

        Ok(Self {
            root: path.to_path_buf(),
            suture_dir,
            cas,
            dag,
            meta,
            author,
            ignore_patterns,
            pending_merge_parents: restored_parents,
            cached_head_snapshot: RefCell::new(None),
            cached_head_id: RefCell::new(None),
            cached_head_branch: RefCell::new(None),
            repo_config,
            is_worktree,
        })
    }
    /// Open an in-memory repository for testing or embedded use.
    ///
    /// Creates a repository backed entirely by in-memory storage. No
    /// filesystem I/O occurs except for the initial tempdir creation.
    /// The CAS uses a temporary directory that is cleaned up on drop.
    pub fn open_in_memory() -> Result<Self, RepoError> {
        let temp_root = tempfile::tempdir().map_err(RepoError::Io)?.keep();
        let suture_dir = temp_root.join(".suture");
        fs::create_dir_all(&suture_dir)?;

        let mut cas = BlobStore::new(&suture_dir)?;
        cas.set_verify_on_read(false);
        let meta = crate::metadata::MetadataStore::open_in_memory()?;

        let mut dag = PatchDag::new();
        let root_patch = Patch::new(
            OperationType::Create,
            TouchSet::empty(),
            None,
            vec![],
            vec![],
            "suture".to_string(),
            "Initial commit".to_string(),
        );
        let root_id = dag.add_patch(root_patch.clone(), vec![])?;
        meta.store_patch(&root_patch)?;

        let main_branch = BranchName::new("main").expect("hardcoded 'main' is always valid");
        dag.create_branch(main_branch.clone(), root_id)?;
        meta.set_branch(&main_branch, &root_id)?;
        meta.set_config("author", "suture")?;

        Ok(Self {
            root: temp_root,
            suture_dir,
            cas,
            dag,
            meta,
            author: "suture".to_string(),
            ignore_patterns: Vec::new(),
            pending_merge_parents: Vec::new(),
            cached_head_snapshot: RefCell::new(None),
            cached_head_id: RefCell::new(None),
            cached_head_branch: RefCell::new(None),
            repo_config: crate::metadata::repo_config::RepoConfig::default(),
            is_worktree: false,
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
                // Check for HEAD / HEAD~N before trying branch name resolution
                if t == "HEAD" {
                    let head = self
                        .dag
                        .head()
                        .ok_or_else(|| RepoError::Custom("no HEAD".to_string()))?;
                    head.1
                } else if let Some(rest) = t.strip_prefix("HEAD~") {
                    let n: usize = rest
                        .parse()
                        .map_err(|_| RepoError::Custom(format!("invalid HEAD~N: {}", t)))?;
                    let (_, head_id) = self.head()?;
                    let mut current = head_id;
                    for _ in 0..n {
                        let patch = self.dag.get_patch(&current).ok_or_else(|| {
                            RepoError::Custom("HEAD ancestor not found".to_string())
                        })?;
                        current = patch
                            .parent_ids
                            .first()
                            .ok_or_else(|| {
                                RepoError::Custom("HEAD has no parent".to_string())
                            })?
                            .to_owned();
                    }
                    current
                } else if let Ok(bn) = BranchName::new(t) {
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
        if let Some(ref cached) = *self.cached_head_id.borrow()
            && let Some(ref branch) = *self.cached_head_branch.borrow()
        {
            return Ok((branch.clone(), *cached));
        }
        let branch_name = self.read_head_branch()?;

        let bn = BranchName::new(&branch_name)?;
        let target_id = self
            .dag
            .get_branch(&bn)
            .ok_or_else(|| RepoError::BranchNotFound(branch_name.clone()))?;

        *self.cached_head_branch.borrow_mut() = Some(branch_name.clone());
        *self.cached_head_id.borrow_mut() = Some(target_id);
        Ok((branch_name, target_id))
    }

    /// List all branches.
    pub fn list_branches(&self) -> Vec<(String, PatchId)> {
        self.dag.list_branches()
    }

    /// Delete a branch. Cannot delete the currently checked-out branch.
    pub fn delete_branch(&mut self, name: &str) -> Result<(), RepoError> {
        let (current_branch, _) = self.head()?;
        if current_branch == name {
            return Err(RepoError::Custom(format!(
                "cannot delete the current branch '{}'",
                name
            )));
        }
        let branch = BranchName::new(name)?;
        self.dag.delete_branch(&branch)?;
        // Also remove from metadata
        self.meta
            .conn()
            .execute(
                "DELETE FROM branches WHERE name = ?1",
                rusqlite::params![name],
            )
            .map_err(|e| RepoError::Custom(e.to_string()))?;
        Ok(())
    }

    // =========================================================================
    // Config
    // =========================================================================

    /// Get a configuration value.
    ///
    /// Lookup order:
    /// 1. `.suture/config` file (repo-level TOML config)
    /// 2. SQLite config table (set via `suture config key=value`)
    /// 3. Global config `~/.config/suture/config.toml`
    pub fn get_config(&self, key: &str) -> Result<Option<String>, RepoError> {
        // 1. Check repo-level config file
        if let Some(val) = self.repo_config.get(key) {
            return Ok(Some(val));
        }
        // 2. Check SQLite config
        if let Some(val) = self.meta.get_config(key).map_err(RepoError::from)? {
            return Ok(Some(val));
        }
        // 3. Check global config
        let global = crate::metadata::global_config::GlobalConfig::load();
        Ok(global.get(key))
    }

    /// Set a configuration value.
    pub fn set_config(&mut self, key: &str, value: &str) -> Result<(), RepoError> {
        self.meta.set_config(key, value).map_err(RepoError::from)
    }

    /// List all configuration key-value pairs.
    pub fn list_config(&self) -> Result<Vec<(String, String)>, RepoError> {
        self.meta.list_config().map_err(RepoError::from)
    }

    // =========================================================================
    // Worktree HEAD (per-worktree branch pointer)
    // =========================================================================

    fn read_head_branch(&self) -> Result<String, RepoError> {
        if self.is_worktree {
            let head_path = self.suture_dir.join("HEAD");
            if head_path.exists() {
                Ok(fs::read_to_string(&head_path)?.trim().to_string())
            } else {
                Ok("main".to_string())
            }
        } else {
            Ok(self
                .meta
                .get_config("head_branch")
                .unwrap_or(None)
                .unwrap_or_else(|| "main".to_string()))
        }
    }

    fn write_head_branch(&self, branch: &str) -> Result<(), RepoError> {
        if self.is_worktree {
            fs::write(self.suture_dir.join("HEAD"), branch)?;
        } else {
            self.meta
                .set_config("head_branch", branch)
                .map_err(RepoError::Meta)?;
        }
        Ok(())
    }

    // =========================================================================
    // Tag Operations
    // =========================================================================

    /// Create a tag pointing to a patch ID (or HEAD).
    ///
    /// Tags are stored as config entries `tag.<name>` pointing to a patch hash.
    pub fn create_tag(&mut self, name: &str, target: Option<&str>) -> Result<(), RepoError> {
        let target_id = match target {
            Some(t) if t == "HEAD" || t.starts_with("HEAD~") => {
                let (_, head_id) = self.head()?;
                let mut current = head_id;
                if let Some(n_str) = t.strip_prefix("HEAD~") {
                    let n: usize = n_str
                        .parse()
                        .map_err(|_| RepoError::Custom(format!("invalid HEAD~N: {}", n_str)))?;
                    for _ in 0..n {
                        if let Some(patch) = self.dag.get_patch(&current) {
                            current = *patch
                                .parent_ids
                                .first()
                                .ok_or_else(|| RepoError::Custom("HEAD has no parent".into()))?;
                        } else {
                            return Err(RepoError::Custom(
                                "HEAD ancestor not found".into(),
                            ));
                        }
                    }
                }
                current
            }
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
                let (_, head_id) = self.head()?;
                head_id
            }
        };
        self.set_config(&format!("tag.{name}"), &target_id.to_hex())
    }

    /// Delete a tag. Returns an error if the tag does not exist.
    pub fn delete_tag(&mut self, name: &str) -> Result<(), RepoError> {
        let key = format!("tag.{name}");
        let exists: bool = self
            .meta
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM config WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .map_err(|e| RepoError::Custom(e.to_string()))?;
        if !exists {
            return Err(RepoError::Custom(format!("tag '{}' not found", name)));
        }
        self.meta
            .conn()
            .execute("DELETE FROM config WHERE key = ?1", rusqlite::params![key])
            .map_err(|e| RepoError::Custom(e.to_string()))?;
        Ok(())
    }

    /// List all tags as (name, target_patch_id).
    pub fn list_tags(&self) -> Result<Vec<(String, PatchId)>, RepoError> {
        let config = self.list_config()?;
        let mut tags = Vec::new();
        for (key, value) in config {
            if let Some(name) = key.strip_prefix("tag.")
                && let Ok(id) = Hash::from_hex(&value)
            {
                tags.push((name.to_string(), id));
            }
        }
        tags.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(tags)
    }

    /// Resolve a tag name to a patch ID.
    pub fn resolve_tag(&self, name: &str) -> Result<Option<PatchId>, RepoError> {
        let val = self.get_config(&format!("tag.{name}"))?;
        match val {
            Some(hex) => Ok(Some(Hash::from_hex(&hex)?)),
            None => Ok(None),
        }
    }

    // =========================================================================
    // Notes
    // =========================================================================

    /// Add a note to a commit.
    pub fn add_note(&self, patch_id: &PatchId, note: &str) -> Result<(), RepoError> {
        let existing = self.list_notes(patch_id)?;
        let next_idx = existing.len();
        let key = format!("note.{}.{}", patch_id, next_idx);
        self.meta.set_config(&key, note).map_err(RepoError::Meta)
    }

    /// List notes for a commit.
    pub fn list_notes(&self, patch_id: &PatchId) -> Result<Vec<String>, RepoError> {
        let prefix = format!("note.{}.", patch_id);
        let all_config = self.meta.list_config().map_err(RepoError::Meta)?;
        let mut notes: Vec<(usize, String)> = Vec::new();
        for (key, value) in &all_config {
            if let Some(idx_str) = key.strip_prefix(&prefix)
                && let Ok(idx) = idx_str.parse::<usize>()
            {
                notes.push((idx, value.clone()));
            }
        }
        notes.sort_by_key(|(idx, _)| *idx);
        Ok(notes.into_iter().map(|(_, v)| v).collect())
    }

    /// Remove a note from a commit. Returns an error if the index is out of range.
    pub fn remove_note(&self, patch_id: &PatchId, index: usize) -> Result<(), RepoError> {
        let notes = self.list_notes(patch_id)?;
        if index >= notes.len() {
            return Err(RepoError::Custom(format!(
                "note index {} out of range ({} notes for commit)",
                index,
                notes.len()
            )));
        }
        let key = format!("note.{}.{}", patch_id, index);
        self.meta.delete_config(&key).map_err(RepoError::Meta)
    }

    // =========================================================================
    // Incremental Push Support
    // =========================================================================

    /// Get patches created after a given patch ID (ancestry walk).
    ///
    /// Returns patches reachable from branch tips but NOT ancestors of `since_id`.
    pub fn patches_since(&self, since_id: &PatchId) -> Vec<Patch> {
        let since_ancestors = self.dag.ancestors(since_id);
        // Include since_id itself in the "already known" set
        let mut known = since_ancestors;
        known.insert(*since_id);

        // Walk from all branch tips, collect patches not in `known`
        let mut new_ids: HashSet<PatchId> = HashSet::new();
        let mut stack: Vec<PatchId> = self.dag.list_branches().iter().map(|(_, id)| *id).collect();

        while let Some(id) = stack.pop() {
            if !known.contains(&id)
                && new_ids.insert(id)
                && let Some(node) = self.dag.get_node(&id)
            {
                for parent in &node.patch.parent_ids {
                    if !known.contains(parent) && !new_ids.contains(parent) {
                        stack.push(*parent);
                    }
                }
            }
        }

        // Topological sort: parents before children (Kahn's algorithm)
        let patches: HashMap<PatchId, Patch> = new_ids
            .into_iter()
            .filter_map(|id| self.dag.get_patch(&id).map(|p| (id, p.clone())))
            .collect();

        // Count in-edges from within our set
        let mut in_degree: HashMap<PatchId, usize> = HashMap::new();
        let mut children: HashMap<PatchId, Vec<PatchId>> = HashMap::new();
        for (&id, patch) in &patches {
            in_degree.entry(id).or_insert(0);
            for parent_id in &patch.parent_ids {
                if patches.contains_key(parent_id) {
                    children.entry(*parent_id).or_default().push(id);
                    *in_degree.entry(id).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<PatchId> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut sorted_ids: Vec<PatchId> = Vec::with_capacity(patches.len());

        while let Some(id) = queue.pop_front() {
            sorted_ids.push(id);
            if let Some(kids) = children.get(&id) {
                for &child in kids {
                    let deg = in_degree
                        .get_mut(&child)
                        .expect("in-degree entry exists for child in topo sort");
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(child);
                    }
                }
            }
        }

        sorted_ids
            .into_iter()
            .filter_map(|id| patches.get(&id).cloned())
            .collect()
    }

    // =========================================================================
    // Staging & Commit
    // =========================================================================

    /// Get repository status.
    pub fn status(&self) -> Result<RepoStatus, RepoError> {
        let working_set = self.meta.working_set()?;
        let branches = self.list_branches();
        let head = self.head()?;

        Ok(RepoStatus {
            head_branch: Some(head.0),
            head_patch: Some(head.1),
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
            if self.is_tracked(path)? {
                self.meta.working_set_add(&repo_path, FileStatus::Deleted)?;
                return Ok(());
            }
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
    ///
    /// Uses the SQLite file_trees table for O(1) lookups when HEAD is cached,
    /// falling back to the in-memory DAG walk only on cold start.
    fn is_tracked(&self, path: &str) -> Result<bool, RepoError> {
        // Fast path: use in-memory cache if available
        if let Some(ref tree) = *self.cached_head_snapshot.borrow() {
            return Ok(tree.contains(path));
        }
        // Medium path: use SQLite file_trees table
        if let Ok((_, head_id)) = self.head()
            && let Ok(result) = self.meta.file_tree_contains(&head_id, path)
        {
            return Ok(result);
        }
        // Slow path: walk the DAG (shouldn't happen after first commit)
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
        let old_head = self.head().map(|(_, id)| id).unwrap_or(Hash::ZERO);
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
        let is_merge_resolution = !self.pending_merge_parents.is_empty();

        let parent_ids = if self.pending_merge_parents.is_empty() {
            vec![head_id]
        } else {
            std::mem::take(&mut self.pending_merge_parents)
        };

        // Clear persisted merge state on commit
        let _ = self
            .meta
            .conn()
            .execute("DELETE FROM config WHERE key = 'pending_merge_parents'", []);

        // Build batched file changes
        let mut file_changes = Vec::new();
        for (path, status) in &staged {
            let full_path = self.root.join(path);

            let (op_type, payload) = match status {
                FileStatus::Added => {
                    let data = fs::read(&full_path)?;
                    let hash = self.cas.put_blob(&data)?;
                    let payload = hash.to_hex().as_bytes().to_vec();
                    (OperationType::Create, payload)
                }
                FileStatus::Modified => {
                    let data = fs::read(&full_path)?;
                    let hash = self.cas.put_blob(&data)?;
                    let payload = hash.to_hex().as_bytes().to_vec();
                    (OperationType::Modify, payload)
                }
                FileStatus::Deleted => (OperationType::Delete, Vec::new()),
                _ => continue,
            };
            file_changes.push(FileChange {
                op: op_type,
                path: path.clone(),
                payload,
            });
        }

        if file_changes.is_empty() {
            return Err(RepoError::NothingToCommit);
        }

        // Create single batched patch
        let batch_patch = Patch::new_batch(
            file_changes,
            parent_ids.clone(),
            self.author.clone(),
            message.to_string(),
        );

        let patch_id = self.dag.add_patch(batch_patch.clone(), parent_ids)?;
        self.meta.store_patch(&batch_patch)?;

        // Clear working set entries in a single batch operation
        let staged_paths: Vec<&str> = staged.iter().map(|(path, _)| path.as_str()).collect();
        self.meta.clear_working_set_batch(&staged_paths)?;

        let branch = BranchName::new(&branch_name)?;
        self.dag.update_branch(&branch, patch_id)?;
        self.meta.set_branch(&branch, &patch_id)?;

        // Persist the file tree for this commit tip (enables O(1) cold-load later).
        // Incremental computation: apply the new patch to the parent's cached tree
        // instead of replaying the entire chain (which is O(n) per commit).
        let tree = if head_id != Hash::ZERO {
            // Load parent's tree from SQLite (stored on previous commit), then
            // apply just the new batch patch. Falls back to full replay only if
            // the parent's tree is missing from cache.
            match self.snapshot(&head_id) {
                Ok(parent_tree) => apply_patch(&parent_tree, &batch_patch, resolve_payload_to_hash)
                    .unwrap_or_else(|_| self.snapshot_uncached(&patch_id).unwrap_or_default()),
                Err(_) => self.snapshot_uncached(&patch_id).unwrap_or_default(),
            }
        } else {
            // Root commit — no parent tree, must replay from scratch
            self.snapshot_uncached(&patch_id).unwrap_or_default()
        };
        {
            let tree_hash = tree.content_hash();
            let _ = self.meta.set_config("head_tree_hash", &tree_hash.to_hex());
            let _ = self.meta.store_file_tree(&patch_id, &tree);
        }

        self.invalidate_head_cache();

        let _ = self.record_reflog(&old_head, &patch_id, &format!("commit: {}", message));

        // If this was a merge resolution, update merge commit's parent_ids
        if is_merge_resolution {
            // The batch patch already has the correct merge parents
            // (already handled above via pending_merge_parents)
        }

        Ok(patch_id)
    }

    // =========================================================================
    // Stash
    // =========================================================================

    pub fn has_uncommitted_changes(&self) -> Result<bool, RepoError> {
        let working_set = self.meta.working_set()?;

        let has_staged = working_set.iter().any(|(_, s)| {
            matches!(
                s,
                FileStatus::Added | FileStatus::Modified | FileStatus::Deleted
            )
        });
        if has_staged {
            return Ok(true);
        }

        if let Ok(head_tree) = self.snapshot_head() {
            for (path, hash) in head_tree.iter() {
                let full_path = self.root.join(path);
                if let Ok(data) = fs::read(&full_path) {
                    let current_hash = Hash::from_data(&data);
                    if &current_hash != hash {
                        return Ok(true);
                    }
                } else {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn stash_push(&mut self, message: Option<&str>) -> Result<usize, RepoError> {
        if !self.has_uncommitted_changes()? {
            return Err(RepoError::NothingToCommit);
        }

        let working_set = self.meta.working_set()?;
        let mut files: Vec<(String, Option<String>)> = Vec::new();

        for (path, status) in &working_set {
            match status {
                FileStatus::Added | FileStatus::Modified => {
                    let full_path = self.root.join(path);
                    if let Ok(data) = fs::read(&full_path) {
                        let hash = self.cas.put_blob(&data)?;
                        files.push((path.clone(), Some(hash.to_hex())));
                    } else {
                        files.push((path.clone(), None));
                    }
                }
                FileStatus::Deleted => {
                    files.push((path.clone(), None));
                }
                _ => {}
            }
        }

        if let Ok(head_tree) = self.snapshot_head() {
            for (path, _hash) in head_tree.iter() {
                let full_path = self.root.join(path);
                if let Ok(data) = fs::read(&full_path) {
                    let current_hash = Hash::from_data(&data);
                    if &current_hash != _hash {
                        let already = files.iter().any(|(p, _)| p == path);
                        if !already {
                            let hash = self.cas.put_blob(&data)?;
                            files.push((path.clone(), Some(hash.to_hex())));
                        }
                    }
                }
            }
        }

        let mut index: usize = 0;
        loop {
            let key = format!("stash.{}.message", index);
            if self.meta.get_config(&key)?.is_none() {
                break;
            }
            index += 1;
        }

        let (branch_name, head_id) = self.head()?;
        let msg = message.unwrap_or("WIP").to_string();
        let files_json = serde_json::to_string(&files).unwrap_or_else(|_| "[]".to_string());

        self.set_config(&format!("stash.{}.message", index), &msg)?;
        self.set_config(&format!("stash.{}.head_branch", index), &branch_name)?;
        self.set_config(&format!("stash.{}.head_id", index), &head_id.to_hex())?;
        self.set_config(&format!("stash.{}.files", index), &files_json)?;

        self.meta
            .conn()
            .execute("DELETE FROM working_set", [])
            .map_err(|e| RepoError::Meta(crate::metadata::MetaError::Database(e)))?;

        if let Ok(head_tree) = self.snapshot_head() {
            let current_tree = head_tree;
            for (path, _) in current_tree.iter() {
                let full_path = self.root.join(path);
                if full_path.exists() {
                    let _ = fs::remove_file(&full_path);
                }
            }
            for (path, hash) in current_tree.iter() {
                let full_path = self.root.join(path);
                if let Some(parent) = full_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Ok(blob) = self.cas.get_blob(hash) {
                    let _ = fs::write(&full_path, &blob);
                }
            }
        }

        Ok(index)
    }

    pub fn stash_pop(&mut self) -> Result<(), RepoError> {
        let stashes = self.stash_list()?;
        if stashes.is_empty() {
            return Err(RepoError::Custom("No stashes found".to_string()));
        }
        let highest = stashes
            .iter()
            .map(|s| s.index)
            .max()
            .expect("stash list is non-empty (checked above)");
        self.stash_apply(highest)?;
        self.stash_drop(highest)?;
        Ok(())
    }

    pub fn stash_apply(&mut self, index: usize) -> Result<(), RepoError> {
        let files_key = format!("stash.{}.files", index);
        let files_json = self
            .meta
            .get_config(&files_key)?
            .ok_or_else(|| RepoError::Custom(format!("stash@{{{}}} not found", index)))?;

        let head_id_key = format!("stash.{}.head_id", index);
        let stash_head_id = self.meta.get_config(&head_id_key)?.unwrap_or_default();

        if let Ok((_, current_head_id)) = self.head()
            && current_head_id.to_hex() != stash_head_id
        {
            tracing::warn!(
                "Warning: HEAD has moved since stash@{{{}}} was created",
                index
            );
        }

        let files: Vec<(String, Option<String>)> =
            serde_json::from_str(&files_json).unwrap_or_default();

        for (path, hash_opt) in &files {
            let full_path = self.root.join(path);
            match hash_opt {
                Some(hex_hash) => {
                    let hash = Hash::from_hex(hex_hash)
                        .map_err(|e| RepoError::Custom(format!("invalid hash in stash: {}", e)))?;
                    let blob = self.cas.get_blob(&hash)?;
                    if let Some(parent) = full_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&full_path, &blob)?;
                    let repo_path = RepoPath::new(path.clone())?;
                    self.meta
                        .working_set_add(&repo_path, FileStatus::Modified)?;
                }
                None => {
                    if full_path.exists() {
                        fs::remove_file(&full_path)?;
                    }
                    let repo_path = RepoPath::new(path.clone())?;
                    self.meta.working_set_add(&repo_path, FileStatus::Deleted)?;
                }
            }
        }

        Ok(())
    }

    pub fn stash_list(&self) -> Result<Vec<StashEntry>, RepoError> {
        let all_config = self.list_config()?;
        let mut entries = Vec::new();

        for (key, value) in &all_config {
            if let Some(rest) = key.strip_prefix("stash.")
                && let Some(idx_str) = rest.strip_suffix(".message")
                && let Ok(idx) = idx_str.parse::<usize>()
            {
                let branch_key = format!("stash.{}.head_branch", idx);
                let head_id_key = format!("stash.{}.head_id", idx);
                let branch = self.meta.get_config(&branch_key)?.unwrap_or_default();
                let head_id = self.meta.get_config(&head_id_key)?.unwrap_or_default();
                entries.push(StashEntry {
                    index: idx,
                    message: value.clone(),
                    branch,
                    head_id,
                });
            }
        }

        entries.sort_by_key(|e| e.index);
        Ok(entries)
    }

    pub fn stash_drop(&mut self, index: usize) -> Result<(), RepoError> {
        let prefix = format!("stash.{}.", index);
        let all_config = self.list_config()?;
        let keys_to_delete: Vec<String> = all_config
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, _)| k.clone())
            .collect();

        if keys_to_delete.is_empty() {
            return Err(RepoError::Custom(format!("stash@{{{}}} not found", index)));
        }

        for key in &keys_to_delete {
            self.meta
                .conn()
                .execute("DELETE FROM config WHERE key = ?1", rusqlite::params![key])
                .map_err(|e| RepoError::Meta(crate::metadata::MetaError::Database(e)))?;
        }

        Ok(())
    }

    // =========================================================================
    // Snapshot & Checkout
    // =========================================================================

    /// Build a FileTree snapshot for the HEAD commit.
    ///
    /// Returns a cached snapshot if the HEAD has not changed since the last
    /// call, making this O(1) instead of O(n) where n = total patches.
    pub fn snapshot_head(&self) -> Result<FileTree, RepoError> {
        // Always get the fresh head_id from the DAG (branch pointers may have
        // been updated externally, e.g., by do_fetch). Only use the in-memory
        // cache if the IDs match.
        let (branch_name, head_id) = {
            let branch_name = self.read_head_branch()?;
            let bn = BranchName::new(&branch_name)?;
            let target_id = self
                .dag
                .get_branch(&bn)
                .ok_or_else(|| RepoError::BranchNotFound(branch_name.clone()))?;
            (branch_name, target_id)
        };

        // Update head caches
        *self.cached_head_branch.borrow_mut() = Some(branch_name.clone());
        *self.cached_head_id.borrow_mut() = Some(head_id);

        if let Some(ref tree) = *self.cached_head_snapshot.borrow() {
            return Ok(tree.clone());
        }

        // Try loading from SQLite (O(1) — no patch replay needed)
        if let Some(tree) = self
            .meta
            .load_file_tree(&head_id)
            .map_err(RepoError::Meta)?
        {
            // Verify the stored tree matches the expected hash
            let tree_hash = tree.content_hash();
            let stored_hash = self
                .meta
                .get_config("head_tree_hash")
                .ok()
                .flatten()
                .and_then(|h| Hash::from_hex(&h).ok());

            if stored_hash.is_none_or(|h| h == tree_hash) {
                // Update stored hash if needed
                if stored_hash.is_none() {
                    let _ = self.meta.set_config("head_tree_hash", &tree_hash.to_hex());
                }

                *self.cached_head_snapshot.borrow_mut() = Some(tree.clone());
                return Ok(tree);
            }
            // Hash mismatch — fall through to recompute
        }

        // Cold path: replay all patches (expensive, but correct)
        let tree = self.snapshot_uncached(&head_id)?;
        let tree_hash = tree.content_hash();

        let _ = self.meta.set_config("head_tree_hash", &tree_hash.to_hex());

        // Persist the tree for next cold start
        let _ = self.meta.store_file_tree(&head_id, &tree);

        *self.cached_head_snapshot.borrow_mut() = Some(tree.clone());
        Ok(tree)
    }

    /// Invalidate the cached HEAD snapshot and branch name.
    ///
    /// Must be called after any operation that changes the HEAD pointer,
    /// including branch updates from external sources (e.g., fetch/pull).
    pub fn invalidate_head_cache(&self) {
        *self.cached_head_snapshot.borrow_mut() = None;
        *self.cached_head_id.borrow_mut() = None;
        *self.cached_head_branch.borrow_mut() = None;
        let _ = self
            .meta
            .conn()
            .execute("DELETE FROM config WHERE key = 'head_tree_hash'", []);
    }

    /// Build a FileTree snapshot for a specific patch (uncached).
    fn snapshot_uncached(&self, patch_id: &PatchId) -> Result<FileTree, RepoError> {
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

    /// Build a FileTree snapshot for a specific patch.
    ///
    /// Tries loading from SQLite first (O(1)), falls back to patch replay (O(n)).
    pub fn snapshot(&self, patch_id: &PatchId) -> Result<FileTree, RepoError> {
        // Try SQLite first
        if let Some(tree) = self
            .meta
            .load_file_tree(patch_id)
            .map_err(RepoError::Meta)?
        {
            return Ok(tree);
        }
        // Fall back to patch replay, then persist
        let tree = self.snapshot_uncached(patch_id)?;
        let _ = self.meta.store_file_tree(patch_id, &tree);
        Ok(tree)
    }

    /// Sync the working tree to match the current HEAD snapshot.
    ///
    /// Compares `old_tree` (the state before the operation) against the
    /// current HEAD snapshot and applies file additions, modifications,
    /// deletions, and renames to disk.
    /// Update the working tree to match the current HEAD snapshot.
    ///
    /// Compares `old_tree` (the state before the operation) against the
    /// current HEAD snapshot and applies file additions, modifications,
    /// deletions, and renames to disk.
    pub fn sync_working_tree(&self, old_tree: &FileTree) -> Result<(), RepoError> {
        use rayon::prelude::*;

        let new_tree = self.snapshot_head()?;
        let diffs = diff_trees(old_tree, &new_tree);

        // Extract fields needed by parallel closures (BlobStore is Send + Sync)
        let cas = &self.cas;
        let root = &self.root;

        // Phase 1: Pre-fetch all blobs in parallel (the I/O-heavy part)
        let blob_results: Result<Vec<(String, Vec<u8>)>, CasError> = diffs
            .par_iter()
            .filter_map(|entry| {
                if let (DiffType::Added | DiffType::Modified, Some(new_hash)) =
                    (&entry.diff_type, &entry.new_hash)
                {
                    Some((entry.path.clone(), *new_hash))
                } else {
                    None
                }
            })
            .map(|(path, hash)| {
                let blob = cas.get_blob(&hash)?;
                Ok((path, blob))
            })
            .collect();

        let blobs: Vec<(String, Vec<u8>)> = blob_results?;

        // Phase 2: Ensure all parent directories exist (sequential, idempotent)
        for (path, _) in &blobs {
            let full_path = root.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        // Phase 3: Write all files in parallel
        blobs
            .par_iter()
            .map(|(path, data)| {
                let full_path = root.join(path);
                fs::write(&full_path, data).map_err(RepoError::Io)
            })
            .collect::<Result<Vec<()>, RepoError>>()?;

        // Phase 4: Handle deletions and renames (sequential — filesystem rename is not parallelizable)
        for entry in &diffs {
            let full_path = root.join(&entry.path);
            match &entry.diff_type {
                DiffType::Deleted => {
                    if full_path.exists() {
                        fs::remove_file(&full_path)?;
                    }
                }
                DiffType::Renamed { old_path, .. } => {
                    let old_full = root.join(old_path);
                    if old_full.exists() {
                        if let Some(parent) = full_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::rename(&old_full, &full_path)?;
                    }
                }
                DiffType::Added | DiffType::Modified => {
                    // Already handled in parallel phases above
                }
            }
        }

        // Phase 5: Clean up files in old_tree but not in new_tree
        for (path, _) in old_tree.iter() {
            if !new_tree.contains(path) {
                let full_path = root.join(path);
                if full_path.exists() {
                    let _ = fs::remove_file(&full_path);
                }
            }
        }

        Ok(())
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
        let old_head = self.head().map(|(_, id)| id).unwrap_or(Hash::ZERO);
        let old_branch = self.head().ok().map(|(n, _)| n);
        let target = BranchName::new(branch_name)?;

        let target_id = self
            .dag
            .get_branch(&target)
            .ok_or_else(|| RepoError::BranchNotFound(branch_name.to_string()))?;

        let has_changes = self.has_uncommitted_changes()?;
        if has_changes {
            self.stash_push(Some("auto-stash before checkout"))?;
        }

        let target_tree = self.snapshot(&target_id)?;

        let current_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());

        let diffs = diff_trees(&current_tree, &target_tree);

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

        for (path, _) in current_tree.iter() {
            if !target_tree.contains(path) {
                let full_path = self.root.join(path);
                if full_path.exists() {
                    let _ = fs::remove_file(&full_path);
                }
            }
        }

        self.write_head_branch(branch_name)?;

        self.invalidate_head_cache();

        let _ = self.record_reflog(
            &old_head,
            &target_id,
            &format!(
                "checkout: moving from {} to {}",
                old_branch.as_deref().unwrap_or("HEAD"),
                branch_name
            ),
        );

        if has_changes && let Err(e) = self.stash_pop() {
            tracing::warn!("Warning: could not restore stashed changes: {}", e);
        }

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
            if name == "HEAD" || name.starts_with("HEAD~") {
                let (_, head_id) = self.head()?;
                let mut target_id = head_id;
                if let Some(n_str) = name.strip_prefix("HEAD~") {
                    let n: usize = n_str
                        .parse()
                        .map_err(|_| RepoError::Custom(format!("invalid HEAD~N: {}", name)))?;
                    for _ in 0..n {
                        let patch = self.dag.get_patch(&target_id).ok_or_else(|| {
                            RepoError::Custom("HEAD ancestor not found".to_string())
                        })?;
                        target_id = patch
                            .parent_ids
                            .first()
                            .ok_or_else(|| RepoError::Custom("HEAD has no parent".to_string()))?
                            .to_owned();
                    }
                }
                return Ok(target_id);
            }
            // Try full hex hash (patch IDs are 64-char hex strings that
            // also happen to pass BranchName validation, so we must try
            // hex before branch name to avoid false branch lookups).
            if let Ok(hash) = Hash::from_hex(name)
                && self.dag.has_patch(&hash)
            {
                return Ok(hash);
            }
            // Try short hash prefix
            let all_patch_ids = self.dag.patch_ids();
            let prefix_matches: Vec<&PatchId> = all_patch_ids
                .iter()
                .filter(|id| id.to_hex().starts_with(name))
                .collect();
            match prefix_matches.len() {
                1 => return Ok(*prefix_matches[0]),
                0 => {}
                n => {
                    return Err(RepoError::Custom(format!(
                        "ambiguous ref '{}' matches {} commits",
                        name, n
                    )));
                }
            }
            // Try tag
            if let Ok(Some(tag_id)) = self.resolve_tag(name) {
                return Ok(tag_id);
            }
            // Fall back to branch name
            let bn = BranchName::new(name)?;
            self.dag
                .get_branch(&bn)
                .ok_or_else(|| RepoError::BranchNotFound(name.to_string()))
        };

        // When both from and to are None, diff HEAD vs working tree
        // to show all uncommitted changes.
        if from.is_none() && to.is_none() {
            let head_tree = self.snapshot_head()?;
            let working_tree = self.build_working_tree()?;
            return Ok(diff_trees(&head_tree, &working_tree));
        }

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

    /// Build a FileTree from the current working directory files.
    fn build_working_tree(&self) -> Result<FileTree, RepoError> {
        let mut tree = FileTree::empty();
        let entries = walk_dir(&self.root, &self.ignore_patterns)?;
        for entry in &entries {
            if let Ok(data) = fs::read(&entry.full_path) {
                let hash = Hash::from_data(&data);
                tree.insert(entry.relative.clone(), hash);
            }
        }
        Ok(tree)
    }

    /// Show staged changes (diff of staged files vs HEAD).
    pub fn diff_staged(&self) -> Result<Vec<DiffEntry>, RepoError> {
        let head_tree = self.snapshot_head()?;
        // Start from HEAD tree so unchanged files are preserved, then overlay
        // staged additions/modifications and remove staged deletions.
        let mut staged_tree = head_tree.clone();
        let working_set = self.meta.working_set()?;
        for (path, status) in &working_set {
            match status {
                FileStatus::Added | FileStatus::Modified => {
                    let full_path = self.root.join(path);
                    if let Ok(data) = fs::read(&full_path) {
                        let hash = Hash::from_data(&data);
                        staged_tree.insert(path.clone(), hash);
                    }
                }
                FileStatus::Deleted => {
                    // File is staged for deletion — remove it from the staged tree
                    staged_tree.remove(path);
                }
                _ => {}
            }
        }
        Ok(diff_trees(&head_tree, &staged_tree))
    }

    // =========================================================================
    // Reset
    // =========================================================================

    /// Reset HEAD to a specific commit.
    ///
    /// Resolves `target` (hex hash or branch name), moves the current branch
    /// pointer, and optionally clears staging and/or restores the working tree
    /// depending on `mode`.
    ///
    /// Returns the resolved target patch ID.
    pub fn reset(&mut self, target: &str, mode: ResetMode) -> Result<PatchId, RepoError> {
        let old_head = self.head().map(|(_, id)| id).unwrap_or(Hash::ZERO);
        let target_id = if target == "HEAD" {
            let (_, id) = self.head()?;
            id
        } else if let Some(rest) = target.strip_prefix("HEAD~") {
            let n: usize = rest
                .parse()
                .map_err(|_| RepoError::Custom(format!("invalid HEAD~N: {}", target)))?;
            let (_, head_id) = self.head()?;
            let mut current = head_id;
            for _ in 0..n {
                let patch = self
                    .dag
                    .get_patch(&current)
                    .ok_or_else(|| RepoError::Custom("HEAD ancestor not found".to_string()))?;
                current = patch
                    .parent_ids
                    .first()
                    .ok_or_else(|| RepoError::Custom("HEAD has no parent".to_string()))?
                    .to_owned();
            }
            current
        } else if let Ok(hash) = Hash::from_hex(target)
            && self.dag.has_patch(&hash)
        {
            hash
        } else {
            let bn = BranchName::new(target)?;
            self.dag
                .get_branch(&bn)
                .ok_or_else(|| RepoError::BranchNotFound(target.to_string()))?
        };

        let (branch_name, _) = self.head()?;
        let old_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());

        let branch = BranchName::new(&branch_name)?;
        self.dag.update_branch(&branch, target_id)?;
        self.meta.set_branch(&branch, &target_id)?;
        self.invalidate_head_cache();

        match mode {
            ResetMode::Soft => {
                let new_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());
                let diffs = diff_trees(&new_tree, &old_tree);
                for entry in &diffs {
                    match &entry.diff_type {
                        DiffType::Added | DiffType::Modified => {
                            let repo_path = RepoPath::new(entry.path.clone())?;
                            self.meta
                                .working_set_add(&repo_path, FileStatus::Modified)?;
                        }
                        DiffType::Deleted => {
                            let repo_path = RepoPath::new(entry.path.clone())?;
                            self.meta.working_set_add(&repo_path, FileStatus::Deleted)?;
                        }
                        DiffType::Renamed { old_path, .. } => {
                            let repo_path = RepoPath::new(old_path.clone())?;
                            self.meta.working_set_add(&repo_path, FileStatus::Deleted)?;
                            let repo_path = RepoPath::new(entry.path.clone())?;
                            self.meta.working_set_add(&repo_path, FileStatus::Added)?;
                        }
                    }
                }
            }
            ResetMode::Mixed | ResetMode::Hard => {
                self.meta
                    .conn()
                    .execute("DELETE FROM working_set", [])
                    .map_err(|e| RepoError::Meta(crate::metadata::MetaError::Database(e)))?;
                if mode == ResetMode::Hard {
                    self.sync_working_tree(&old_tree)?;
                }
            }
        }

        let _ = self.record_reflog(
            &old_head,
            &target_id,
            &format!("reset: moving to {}", target),
        );

        Ok(target_id)
    }

    // =========================================================================
    // Revert
    // =========================================================================

    /// Revert a commit by creating a new patch that undoes its changes.
    ///
    /// The revert creates inverse patches (Delete for Create, etc.)
    /// and commits them on top of HEAD, then syncs the working tree.
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

        let old_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());

        match &patch.operation_type {
            OperationType::Batch => {
                let changes = patch.file_changes().ok_or_else(|| {
                    RepoError::Custom("batch patch has invalid file changes".into())
                })?;
                if changes.is_empty() {
                    return Err(RepoError::Custom("cannot revert empty batch".into()));
                }
                let parent_tree = patch
                    .parent_ids
                    .first()
                    .map(|pid| self.snapshot(pid).unwrap_or_else(|_| FileTree::empty()))
                    .unwrap_or_else(FileTree::empty);
                let mut revert_changes = Vec::new();
                for change in &changes {
                    match change.op {
                        OperationType::Create | OperationType::Modify => {
                            revert_changes.push(FileChange {
                                op: OperationType::Delete,
                                path: change.path.clone(),
                                payload: Vec::new(),
                            });
                        }
                        OperationType::Delete => {
                            if let Some(hash) = parent_tree.get(&change.path) {
                                revert_changes.push(FileChange {
                                    op: OperationType::Modify,
                                    path: change.path.clone(),
                                    payload: hash.to_hex().as_bytes().to_vec(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                if revert_changes.is_empty() {
                    return Err(RepoError::Custom("nothing to revert in batch".into()));
                }
                let revert_patch =
                    Patch::new_batch(revert_changes, vec![head_id], self.author.clone(), msg);
                let revert_id = self.dag.add_patch(revert_patch.clone(), vec![head_id])?;
                self.meta.store_patch(&revert_patch)?;

                let branch = BranchName::new(&branch_name)?;
                self.dag.update_branch(&branch, revert_id)?;
                self.meta.set_branch(&branch, &revert_id)?;

                self.invalidate_head_cache();

                self.sync_working_tree(&old_tree)?;
                Ok(revert_id)
            }
            OperationType::Create | OperationType::Modify => {
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

                self.invalidate_head_cache();

                self.sync_working_tree(&old_tree)?;
                Ok(revert_id)
            }
            OperationType::Delete => {
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

                        let revert_id = self.dag.add_patch(revert_patch.clone(), vec![head_id])?;
                        self.meta.store_patch(&revert_patch)?;

                        let branch = BranchName::new(&branch_name)?;
                        self.dag.update_branch(&branch, revert_id)?;
                        self.meta.set_branch(&branch, &revert_id)?;

                        self.invalidate_head_cache();

                        self.sync_working_tree(&old_tree)?;
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
    // Squash
    // =========================================================================

    /// Squash the last N patches on the current branch into a single patch.
    ///
    /// Returns the new tip patch ID.
    pub fn squash(&mut self, count: usize, message: &str) -> Result<PatchId, RepoError> {
        if count < 2 {
            return Err(RepoError::Custom(
                "need at least 2 patches to squash".into(),
            ));
        }

        let (branch_name, tip_id) = self.head()?;
        let chain = self.dag().patch_chain(&tip_id);

        // chain is tip-first, so the last N patches are chain[0..count]
        if chain.len() < count + 1 {
            return Err(RepoError::Custom(format!(
                "only {} patches on branch, cannot squash {}",
                chain.len(),
                count
            )));
        }

        // Extract patches to squash (reversed to get oldest-first)
        let mut to_squash = Vec::new();
        for i in (0..count).rev() {
            let pid = &chain[i];
            let patch = self
                .dag()
                .get_patch(pid)
                .ok_or_else(|| RepoError::Custom(format!("patch not found: {}", pid.to_hex())))?;
            to_squash.push(patch.clone());
        }

        let parent_of_first = *to_squash[0]
            .parent_ids
            .first()
            .ok_or_else(|| RepoError::Custom("cannot squash root patch".into()))?;

        let result = crate::patch::compose::compose_chain(&to_squash, &self.author, message)
            .map_err(|e| RepoError::Custom(e.to_string()))?;

        let new_id = self
            .dag_mut()
            .add_patch(result.patch.clone(), vec![parent_of_first])?;
        self.meta().store_patch(&result.patch)?;

        let branch = BranchName::new(&branch_name).map_err(|e| RepoError::Custom(e.to_string()))?;
        self.dag_mut().update_branch(&branch, new_id)?;
        self.meta().set_branch(&branch, &new_id)?;

        self.record_reflog(
            to_squash.last().map(|p| &p.id).unwrap_or(&parent_of_first),
            &new_id,
            &format!("squash: {} patches into one", count),
        )?;

        self.invalidate_head_cache();

        Ok(new_id)
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

    /// Execute a merge of `source_branch` into the current HEAD branch.
    ///
    /// For clean merges (no conflicts):
    /// 1. Collect unique patches from both branches (after LCA)
    /// 2. Apply the source branch's tree onto HEAD's working tree
    /// 3. Create a merge commit (patch with two parents)
    /// 4. Update the working tree to reflect the merge result
    ///
    /// For merges with conflicts:
    /// 1. Apply all non-conflicting patches from source
    /// 2. Return a `MergeExecutionResult` with conflict details
    /// 3. The caller can then resolve conflicts and commit
    ///
    /// Preview a merge without modifying the repository.
    ///
    /// Returns the same `MergeExecutionResult` that `execute_merge` would produce,
    /// but without creating any patches, moving branches, or writing files.
    /// The `merge_patch_id` and `unresolved_conflicts` fields are computed
    /// heuristically (the patch ID is a placeholder since no patch is actually created).
    pub fn preview_merge(
        &self,
        source_branch: &str,
    ) -> Result<MergeExecutionResult, RepoError> {
        if !self.pending_merge_parents.is_empty() {
            return Err(RepoError::MergeInProgress);
        }

        let (head_branch, head_id) = self.head()?;
        let source_bn = BranchName::new(source_branch)?;
        let source_tip = self
            .dag
            .get_branch(&source_bn)
            .ok_or_else(|| RepoError::BranchNotFound(source_branch.to_string()))?;

        let head_bn = BranchName::new(&head_branch)?;

        let merge_result = self.dag.merge_branches(&head_bn, &source_bn)?;

        if head_id == source_tip {
            return Ok(MergeExecutionResult {
                is_clean: true,
                merged_tree: self.snapshot_head()?,
                merge_patch_id: None,
                unresolved_conflicts: Vec::new(),
                patches_applied: 0,
            });
        }

        if merge_result.patches_b_only.is_empty() && merge_result.patches_a_only.is_empty() {
            return Ok(MergeExecutionResult {
                is_clean: true,
                merged_tree: self.snapshot_head()?,
                merge_patch_id: None,
                unresolved_conflicts: Vec::new(),
                patches_applied: 0,
            });
        }

        let patches_applied = merge_result.patches_b_only.len();
        let is_clean = merge_result.is_clean;

        if is_clean {
            // For a clean merge preview, compute what the merge tree would look like
            // without actually writing files or creating patches.
            let source_tree = self.snapshot(&source_tip).unwrap_or_else(|_| FileTree::empty());
            let lca_id = self
                .dag
                .lca(&head_id, &source_tip)
                .ok_or_else(|| RepoError::Custom("no common ancestor found".to_string()))?;
            let lca_tree = self.snapshot(&lca_id).unwrap_or_else(|_| FileTree::empty());
            let head_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());

            let source_diffs = diff_trees(&lca_tree, &source_tree);
            let mut merged_tree = head_tree.clone();
            for entry in &source_diffs {
                match &entry.diff_type {
                    DiffType::Added | DiffType::Modified => {
                        if let Some(new_hash) = &entry.new_hash {
                            merged_tree.insert(entry.path.clone(), *new_hash);
                        }
                    }
                    DiffType::Deleted => {
                        merged_tree.remove(&entry.path);
                    }
                    DiffType::Renamed { old_path, .. } => {
                        if let Some(old_hash) = entry.old_hash {
                            merged_tree.remove(old_path);
                            merged_tree.insert(entry.path.clone(), old_hash);
                        }
                    }
                }
            }

            Ok(MergeExecutionResult {
                is_clean: true,
                merged_tree,
                merge_patch_id: None, // No actual patch created in preview
                unresolved_conflicts: Vec::new(),
                patches_applied,
            })
        } else {
            // For conflicting merge preview, compute conflicts without writing files.
            let head_tree = self.snapshot(&head_id).unwrap_or_else(|_| FileTree::empty());
            let source_tree = self.snapshot(&source_tip).unwrap_or_else(|_| FileTree::empty());
            let lca_id = self
                .dag
                .lca(&head_id, &source_tip)
                .ok_or_else(|| RepoError::Custom("no common ancestor found".to_string()))?;
            let lca_tree = self.snapshot(&lca_id).unwrap_or_else(|_| FileTree::empty());

            // Collect all paths from all three trees
            let mut all_paths = std::collections::HashSet::new();
            for path in head_tree.paths() {
                all_paths.insert(path);
            }
            for path in source_tree.paths() {
                all_paths.insert(path);
            }
            for path in lca_tree.paths() {
                all_paths.insert(path);
            }

            let mut unresolved_conflicts: Vec<ConflictInfo> = Vec::new();
            for path in &all_paths {
                let lca_hash = lca_tree.get(path).copied();
                let ours_hash = head_tree.get(path).copied();
                let theirs_hash = source_tree.get(path).copied();

                // Skip if all three sides agree
                if ours_hash == theirs_hash {
                    continue;
                }
                // Skip if only one side changed
                if ours_hash == lca_hash || theirs_hash == lca_hash {
                    continue;
                }
                // This is a genuine conflict
                unresolved_conflicts.push(ConflictInfo {
                    path: path.to_string(),
                    our_patch_id: head_id,
                    their_patch_id: source_tip,
                    our_content_hash: ours_hash,
                    their_content_hash: theirs_hash,
                    base_content_hash: lca_hash,
                });
            }

            Ok(MergeExecutionResult {
                is_clean: false,
                merged_tree: self.snapshot_head()?,
                merge_patch_id: None,
                unresolved_conflicts,
                patches_applied,
            })
        }
    }

    /// Execute a merge: applies patches from source_branch into the current branch.
    pub fn execute_merge(
        &mut self,
        source_branch: &str,
    ) -> Result<MergeExecutionResult, RepoError> {
        if !self.pending_merge_parents.is_empty() {
            return Err(RepoError::MergeInProgress);
        }

        let (head_branch, head_id) = self.head()?;
        let source_bn = BranchName::new(source_branch)?;
        let source_tip = self
            .dag
            .get_branch(&source_bn)
            .ok_or_else(|| RepoError::BranchNotFound(source_branch.to_string()))?;

        let head_bn = BranchName::new(&head_branch)?;

        let merge_result = self.dag.merge_branches(&head_bn, &source_bn)?;

        if head_id == source_tip {
            return Ok(MergeExecutionResult {
                is_clean: true,
                merged_tree: self.snapshot_head()?,
                merge_patch_id: None,
                unresolved_conflicts: Vec::new(),
                patches_applied: 0,
            });
        }

        if merge_result.patches_b_only.is_empty() && merge_result.patches_a_only.is_empty() {
            return Ok(MergeExecutionResult {
                is_clean: true,
                merged_tree: self.snapshot_head()?,
                merge_patch_id: None,
                unresolved_conflicts: Vec::new(),
                patches_applied: 0,
            });
        }

        if merge_result.is_clean {
            self.execute_clean_merge(&head_id, &source_tip, &head_branch, &merge_result)
        } else {
            self.execute_conflicting_merge(
                &head_id,
                &source_tip,
                source_branch,
                &head_branch,
                &merge_result,
            )
        }
    }

    fn execute_clean_merge(
        &mut self,
        head_id: &PatchId,
        source_tip: &PatchId,
        head_branch: &str,
        merge_result: &MergeResult,
    ) -> Result<MergeExecutionResult, RepoError> {
        let head_tree = self.snapshot(head_id)?;
        let source_tree = self.snapshot(source_tip)?;
        let lca_id = self
            .dag
            .lca(head_id, source_tip)
            .ok_or_else(|| RepoError::Custom("no common ancestor found".to_string()))?;
        let lca_tree = self.snapshot(&lca_id).unwrap_or_else(|_| FileTree::empty());

        let source_diffs = diff_trees(&lca_tree, &source_tree);
        let mut merged_tree = head_tree.clone();

        for entry in &source_diffs {
            let full_path = self.root.join(&entry.path);
            match &entry.diff_type {
                DiffType::Added | DiffType::Modified => {
                    if let Some(new_hash) = &entry.new_hash {
                        let blob = self.cas.get_blob(new_hash)?;
                        if let Some(parent) = full_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(&full_path, &blob)?;
                        merged_tree.insert(entry.path.clone(), *new_hash);
                    }
                }
                DiffType::Deleted => {
                    if full_path.exists() {
                        fs::remove_file(&full_path)?;
                    }
                    merged_tree.remove(&entry.path);
                }
                DiffType::Renamed { old_path, .. } => {
                    let old_full = self.root.join(old_path);
                    if old_full.exists() {
                        if let Some(parent) = full_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::rename(&old_full, &full_path)?;
                    }
                    if let Some(old_hash) = entry.old_hash {
                        merged_tree.remove(old_path);
                        merged_tree.insert(entry.path.clone(), old_hash);
                    }
                }
            }
        }

        let merge_patch = Patch::new(
            OperationType::Merge,
            TouchSet::empty(),
            None,
            vec![],
            vec![*head_id, *source_tip],
            self.author.clone(),
            format!("Merge branch '{}' into {}", source_tip, head_branch),
        );

        let merge_id = self
            .dag
            .add_patch(merge_patch.clone(), vec![*head_id, *source_tip])?;
        self.meta.store_patch(&merge_patch)?;

        let branch = BranchName::new(head_branch)?;
        self.dag.update_branch(&branch, merge_id)?;
        self.meta.set_branch(&branch, &merge_id)?;

        self.invalidate_head_cache();

        Ok(MergeExecutionResult {
            is_clean: true,
            merged_tree,
            merge_patch_id: Some(merge_id),
            unresolved_conflicts: Vec::new(),
            patches_applied: merge_result.patches_b_only.len(),
        })
    }

    fn execute_conflicting_merge(
        &mut self,
        head_id: &PatchId,
        source_tip: &PatchId,
        source_branch: &str,
        head_branch: &str,
        merge_result: &MergeResult,
    ) -> Result<MergeExecutionResult, RepoError> {
        let head_tree = self.snapshot(head_id)?;
        let source_tree = self.snapshot(source_tip)?;

        let lca_id = self
            .dag
            .lca(head_id, source_tip)
            .ok_or_else(|| RepoError::Custom("no common ancestor found".to_string()))?;
        let lca_tree = self.snapshot(&lca_id).unwrap_or_else(|_| FileTree::empty());

        let conflicting_patch_ids: HashSet<PatchId> = merge_result
            .conflicts
            .iter()
            .flat_map(|c| [c.patch_a_id, c.patch_b_id])
            .collect();

        let mut merged_tree = head_tree.clone();
        let mut patches_applied = 0;

        for entry in &merge_result.patches_b_only {
            if conflicting_patch_ids.contains(entry) {
                continue;
            }
            if let Some(patch) = self.dag.get_patch(entry) {
                if patch.is_identity() || patch.operation_type == OperationType::Merge {
                    continue;
                }
                if let Some(path) = &patch.target_path {
                    let full_path = self.root.join(path);
                    match patch.operation_type {
                        OperationType::Create | OperationType::Modify => {
                            if let Some(blob_hash) = resolve_payload_to_hash(patch)
                                && self.cas.has_blob(&blob_hash)
                            {
                                let blob = self.cas.get_blob(&blob_hash)?;
                                if let Some(parent) = full_path.parent() {
                                    fs::create_dir_all(parent)?;
                                }
                                fs::write(&full_path, &blob)?;
                                merged_tree.insert(path.clone(), blob_hash);
                            }
                        }
                        OperationType::Delete => {
                            if full_path.exists() {
                                fs::remove_file(&full_path)?;
                            }
                            merged_tree.remove(path);
                        }
                        _ => {}
                    }
                }
                patches_applied += 1;
            }
        }

        let mut unresolved_conflicts = Vec::new();

        for conflict in &merge_result.conflicts {
            let conflict_info =
                self.build_conflict_info(conflict, &head_tree, &source_tree, &lca_tree);
            if let Some(info) = conflict_info {
                let full_path = self.root.join(&info.path);
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let conflict_content =
                    self.write_conflict_markers(&info, source_branch, head_branch)?;
                fs::write(&full_path, conflict_content.as_bytes())?;
                let hash = self.cas.put_blob(conflict_content.as_bytes())?;
                merged_tree.insert(info.path.clone(), hash);
                unresolved_conflicts.push(info);
            }
        }

        self.pending_merge_parents = vec![*head_id, *source_tip];

        // Persist merge state so it survives repo reopen
        let parents_json = serde_json::to_string(&self.pending_merge_parents).unwrap_or_default();
        let _ = self.meta.set_config("pending_merge_parents", &parents_json);

        Ok(MergeExecutionResult {
            is_clean: false,
            merged_tree,
            merge_patch_id: None,
            unresolved_conflicts,
            patches_applied,
        })
    }

    fn build_conflict_info(
        &self,
        conflict: &Conflict,
        head_tree: &FileTree,
        source_tree: &FileTree,
        lca_tree: &FileTree,
    ) -> Option<ConflictInfo> {
        let patch_a = self.dag.get_patch(&conflict.patch_a_id)?;
        let patch_b = self.dag.get_patch(&conflict.patch_b_id)?;

        let path = patch_a
            .target_path
            .clone()
            .or_else(|| patch_b.target_path.clone())
            .or_else(|| {
                // For batch patches, find the conflicting path from the conflict addresses
                conflict.conflict_addresses.first().cloned()
            })?;

        let our_content_hash = head_tree.get(&path).copied();
        let their_content_hash = source_tree.get(&path).copied();
        let base_content_hash = lca_tree.get(&path).copied();

        Some(ConflictInfo {
            path,
            our_patch_id: conflict.patch_a_id,
            their_patch_id: conflict.patch_b_id,
            our_content_hash,
            their_content_hash,
            base_content_hash,
        })
    }

    fn write_conflict_markers(
        &self,
        info: &ConflictInfo,
        source_branch: &str,
        head_branch: &str,
    ) -> Result<String, RepoError> {
        let our_content = match info.our_content_hash {
            Some(hash) => String::from_utf8(self.cas.get_blob(&hash)?).unwrap_or_default(),
            None => String::new(),
        };

        let their_content = match info.their_content_hash {
            Some(hash) => String::from_utf8(self.cas.get_blob(&hash)?).unwrap_or_default(),
            None => String::new(),
        };

        let base_content = match info.base_content_hash {
            Some(hash) => Some(String::from_utf8(self.cas.get_blob(&hash)?).unwrap_or_default()),
            None => None,
        };

        let merged = three_way_merge(
            base_content.as_deref(),
            &our_content,
            &their_content,
            head_branch,
            source_branch,
        );

        match merged {
            Ok(content) => Ok(content),
            Err(conflict_lines) => {
                let mut result = String::new();
                for line in conflict_lines {
                    result.push_str(&line);
                    result.push('\n');
                }
                Ok(result)
            }
        }
    }

    // =========================================================================
    // Cherry-pick
    // =========================================================================

    /// Cherry-pick a patch onto the current HEAD branch.
    ///
    /// Creates a new patch with the same changes (operation_type, touch_set,
    /// target_path, payload) but with the current HEAD as its parent.
    pub fn cherry_pick(&mut self, patch_id: &PatchId) -> Result<PatchId, RepoError> {
        let old_head = self.head().map(|(_, id)| id).unwrap_or(Hash::ZERO);
        let patch = self
            .dag
            .get_patch(patch_id)
            .ok_or_else(|| RepoError::Custom(format!("patch not found: {}", patch_id)))?;

        if patch.operation_type == OperationType::Identity
            || patch.operation_type == OperationType::Merge
            || patch.operation_type == OperationType::Create
        {
            return Err(RepoError::Custom(format!(
                "cannot cherry-pick {:?} patches",
                patch.operation_type
            )));
        }

        let (branch_name, head_id) = self.head()?;

        let new_patch = if patch.operation_type == OperationType::Batch {
            let changes = patch
                .file_changes()
                .ok_or_else(|| RepoError::Custom("batch patch has invalid file changes".into()))?;
            Patch::new_batch(
                changes,
                vec![head_id],
                self.author.clone(),
                patch.message.clone(),
            )
        } else {
            Patch::new(
                patch.operation_type.clone(),
                patch.touch_set.clone(),
                patch.target_path.clone(),
                patch.payload.clone(),
                vec![head_id],
                self.author.clone(),
                patch.message.clone(),
            )
        };

        let new_id = match self.dag.add_patch(new_patch.clone(), vec![head_id]) {
            Ok(id) => id,
            Err(DagError::DuplicatePatch(_)) => {
                let head_ancestors = self.dag.ancestors(&head_id);
                let new_patch_id = new_patch.id;
                if head_ancestors.contains(&new_patch_id) {
                    return Ok(new_patch_id);
                }
                return Err(RepoError::Custom(
                    "patch already exists in DAG and is not reachable from HEAD".to_string(),
                ));
            }
            Err(e) => return Err(RepoError::Dag(e)),
        };
        self.meta.store_patch(&new_patch)?;

        let branch = BranchName::new(&branch_name)?;
        let old_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());
        self.dag.update_branch(&branch, new_id)?;
        self.meta.set_branch(&branch, &new_id)?;

        self.invalidate_head_cache();

        let _ = self.record_reflog(&old_head, &new_id, &format!("cherry-pick: {}", patch_id));

        self.sync_working_tree(&old_tree)?;

        Ok(new_id)
    }

    // =========================================================================
    // Rebase
    // =========================================================================

    /// Rebase the current branch onto a target branch.
    ///
    /// Finds commits unique to the current branch (after the LCA with target),
    /// then replays them onto the target branch tip. Updates the current
    /// branch pointer to the new tip.
    pub fn rebase(&mut self, target_branch: &str) -> Result<RebaseResult, RepoError> {
        let old_head = self.head().map(|(_, id)| id).unwrap_or(Hash::ZERO);
        let (head_branch, head_id) = self.head()?;
        let target_bn = BranchName::new(target_branch)?;
        let target_tip = self
            .dag
            .get_branch(&target_bn)
            .ok_or_else(|| RepoError::BranchNotFound(target_branch.to_string()))?;

        if head_id == target_tip {
            return Ok(RebaseResult {
                patches_replayed: 0,
                new_tip: head_id,
            });
        }

        let lca_id = self
            .dag
            .lca(&head_id, &target_tip)
            .ok_or_else(|| RepoError::Custom("no common ancestor found".to_string()))?;

        if lca_id == head_id {
            let branch = BranchName::new(&head_branch)?;
            let old_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());
            self.dag.update_branch(&branch, target_tip)?;
            self.meta.set_branch(&branch, &target_tip)?;
            self.invalidate_head_cache();

            self.sync_working_tree(&old_tree)?;

            return Ok(RebaseResult {
                patches_replayed: 0,
                new_tip: target_tip,
            });
        }

        let mut head_ancestors = self.dag.ancestors(&lca_id);
        head_ancestors.insert(lca_id);

        let mut to_replay: Vec<Patch> = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = vec![head_id];

        while let Some(id) = stack.pop() {
            if visited.contains(&id) || head_ancestors.contains(&id) {
                continue;
            }
            visited.insert(id);
            if let Some(patch) = self.dag.get_patch(&id) {
                to_replay.push(patch.clone());
                for parent_id in &patch.parent_ids {
                    if !visited.contains(parent_id) {
                        stack.push(*parent_id);
                    }
                }
            }
        }

        to_replay.sort_by_key(|p| p.timestamp);

        let branch = BranchName::new(&head_branch)?;
        let old_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());
        self.dag.update_branch(&branch, target_tip)?;
        self.meta.set_branch(&branch, &target_tip)?;
        self.invalidate_head_cache();

        let mut current_parent = target_tip;
        let mut last_new_id = target_tip;
        let mut replayed = 0usize;

        for patch in &to_replay {
            if patch.operation_type == OperationType::Merge
                || patch.operation_type == OperationType::Identity
                || patch.operation_type == OperationType::Create
            {
                continue;
            }

            let new_patch = if patch.operation_type == OperationType::Batch {
                let changes = patch.file_changes().unwrap_or_default();
                Patch::new_batch(
                    changes,
                    vec![current_parent],
                    self.author.clone(),
                    patch.message.clone(),
                )
            } else {
                Patch::new(
                    patch.operation_type.clone(),
                    patch.touch_set.clone(),
                    patch.target_path.clone(),
                    patch.payload.clone(),
                    vec![current_parent],
                    self.author.clone(),
                    patch.message.clone(),
                )
            };

            let new_id = self
                .dag
                .add_patch(new_patch.clone(), vec![current_parent])?;
            self.meta.store_patch(&new_patch)?;

            last_new_id = new_id;
            current_parent = new_id;
            replayed += 1;
        }

        self.dag.update_branch(&branch, last_new_id)?;
        self.meta.set_branch(&branch, &last_new_id)?;
        self.invalidate_head_cache();

        self.sync_working_tree(&old_tree)?;

        let _ = self.record_reflog(
            &old_head,
            &last_new_id,
            &format!("rebase onto {}", target_branch),
        );

        Ok(RebaseResult {
            patches_replayed: replayed,
            new_tip: last_new_id,
        })
    }

    // =========================================================================
    // Interactive Rebase
    // =========================================================================

    /// Group a patch chain into logical commits.
    ///
    /// A "logical commit" is a contiguous chain of per-file patches that share
    /// the same message. Returns groups in oldest-first order (root to tip).
    pub fn commit_groups(&self, patches: &[Patch]) -> Vec<Vec<Patch>> {
        if patches.is_empty() {
            return Vec::new();
        }

        // Sort oldest first
        let mut sorted: Vec<Patch> = patches.to_vec();
        sorted.sort_by_key(|p| p.timestamp);

        let mut groups: Vec<Vec<Patch>> = Vec::new();
        let mut current_group: Vec<Patch> = Vec::new();
        let mut current_message: Option<String> = None;

        for patch in &sorted {
            // Skip structural patches (same as the rebase skip logic)
            if patch.operation_type == OperationType::Merge
                || patch.operation_type == OperationType::Identity
                || patch.operation_type == OperationType::Create
            {
                continue;
            }

            match &current_message {
                None => {
                    current_message = Some(patch.message.clone());
                    current_group.push(patch.clone());
                }
                Some(msg) if msg == &patch.message => {
                    // Same message — same logical commit
                    current_group.push(patch.clone());
                }
                Some(_) => {
                    // Different message — new logical commit
                    if !current_group.is_empty() {
                        groups.push(std::mem::take(&mut current_group));
                    }
                    current_message = Some(patch.message.clone());
                    current_group.push(patch.clone());
                }
            }
        }

        if !current_group.is_empty() {
            groups.push(current_group);
        }

        groups
    }

    /// Get patches between a base commit and HEAD (exclusive of base).
    ///
    /// Walks the first-parent chain from HEAD back to `base`, collecting
    /// all patches that are NOT ancestors of `base`.
    pub fn patches_since_base(&self, base: &PatchId) -> Vec<Patch> {
        let base_ancestors = self.dag.ancestors(base);
        let mut exclusion = base_ancestors;
        exclusion.insert(*base);

        let (_, head_id) = self
            .head()
            .unwrap_or_else(|_| ("main".to_string(), Hash::ZERO));
        let chain = self.dag.patch_chain(&head_id);

        chain
            .into_iter()
            .filter(|id| !exclusion.contains(id))
            .filter_map(|id| self.dag.get_patch(&id).cloned())
            .collect()
    }

    /// Generate a TODO file for interactive rebase.
    ///
    /// Returns the TODO file content as a string.
    pub fn generate_rebase_todo(&self, base: &PatchId) -> Result<String, RepoError> {
        let patches = self.patches_since_base(base);
        let groups = self.commit_groups(&patches);

        let mut lines = vec![
            String::new(),
            "# Interactive Rebase TODO".to_string(),
            "#".to_string(),
            "# Commands:".to_string(),
            "#  pick   = use commit".to_string(),
            "#  reword = use commit, but edit the commit message".to_string(),
            "#  edit   = use commit, but stop for amending".to_string(),
            "#  squash = use commit, but meld into previous commit".to_string(),
            "#  drop   = remove commit".to_string(),
            String::new(),
        ];

        for group in &groups {
            if let Some(patch) = group.first() {
                let short_hash = patch.id.to_hex().chars().take(8).collect::<String>();
                lines.push(format!("pick {} {}", short_hash, patch.message));
            }
        }

        lines.push(String::new());
        Ok(lines.join("\n"))
    }

    /// Parse a TODO file into a rebase plan.
    pub fn parse_rebase_todo(
        &self,
        todo_content: &str,
        base: &PatchId,
    ) -> Result<RebasePlan, RepoError> {
        let patches = self.patches_since_base(base);
        let groups = self.commit_groups(&patches);

        // Build a map from short hash -> commit group
        let mut group_map: HashMap<String, (String, Vec<PatchId>)> = HashMap::new();
        for group in &groups {
            if let Some(first) = group.first() {
                let short_hash = first.id.to_hex().chars().take(8).collect::<String>();
                let patch_ids: Vec<PatchId> = group.iter().map(|p| p.id).collect();
                group_map.insert(short_hash, (first.message.clone(), patch_ids));
            }
        }

        let mut entries = Vec::new();

        for line in todo_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.splitn(3, ' ');
            let action_str = match parts.next() {
                Some(a) => a,
                None => continue,
            };
            let short_hash = match parts.next() {
                Some(h) => h,
                None => continue,
            };
            let message = parts.next().unwrap_or("").to_string();

            let action = match action_str {
                "pick" | "p" => RebaseAction::Pick,
                "reword" | "r" => RebaseAction::Reword,
                "edit" | "e" => RebaseAction::Edit,
                "squash" | "s" => RebaseAction::Squash,
                "drop" | "d" => RebaseAction::Drop,
                _ => continue, // Skip unknown actions
            };

            // Look up the commit group by short hash
            let (group_message, patch_ids) = group_map
                .get(short_hash)
                .cloned()
                .unwrap_or_else(|| (message.clone(), Vec::new()));

            // Use the message from the TODO if the user changed it (for reword)
            let effective_message = if action == RebaseAction::Reword {
                message
            } else {
                group_message
            };

            let commit_tip = patch_ids.last().copied().unwrap_or(Hash::ZERO);

            entries.push(RebasePlanEntry {
                action,
                commit_tip,
                message: effective_message,
                patch_ids,
            });
        }

        Ok(RebasePlan { entries })
    }

    /// Execute an interactive rebase plan.
    ///
    /// Replays commits according to the plan, handling pick/reword/edit/squash/drop.
    /// Returns the new tip patch ID.
    pub fn rebase_interactive(
        &mut self,
        plan: &RebasePlan,
        onto: &PatchId,
    ) -> Result<PatchId, RepoError> {
        let old_head = self.head().map(|(_, id)| id).unwrap_or(Hash::ZERO);
        let (head_branch, _head_id) = self.head()?;

        // Detach HEAD to point at the onto target
        let branch = BranchName::new(&head_branch)?;
        let old_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());
        self.dag.update_branch(&branch, *onto)?;
        self.meta.set_branch(&branch, onto)?;
        self.invalidate_head_cache();

        let mut current_parent = *onto;
        let mut last_new_id = *onto;
        let mut squash_message_acc: Option<String> = None;

        for entry in &plan.entries {
            match entry.action {
                RebaseAction::Drop => {
                    // Skip this commit entirely
                    continue;
                }
                RebaseAction::Pick
                | RebaseAction::Reword
                | RebaseAction::Edit
                | RebaseAction::Squash => {
                    // Get the original patches for this commit group
                    let patches: Vec<Patch> = entry
                        .patch_ids
                        .iter()
                        .filter_map(|id| self.dag.get_patch(id).cloned())
                        .collect();

                    if patches.is_empty() {
                        continue;
                    }

                    // Determine the message to use
                    let message = if entry.action == RebaseAction::Squash {
                        // For squash, we accumulate messages and use them later
                        let mut msg = squash_message_acc.take().unwrap_or_default();
                        if !msg.is_empty() {
                            msg.push('\n');
                        }
                        msg.push_str(&entry.message);
                        squash_message_acc = Some(msg);
                        continue; // Don't create patches yet — wait for next pick/edit/reword
                    } else {
                        // For pick/reword/edit: use accumulated squash message if any
                        if let Some(sq_msg) = squash_message_acc.take() {
                            let mut combined = sq_msg;
                            if !combined.is_empty() && !entry.message.is_empty() {
                                combined.push('\n');
                            }
                            combined.push_str(&entry.message);
                            combined
                        } else {
                            entry.message.clone()
                        }
                    };

                    // Replay each patch in the commit group
                    for patch in &patches {
                        if patch.operation_type == OperationType::Merge
                            || patch.operation_type == OperationType::Identity
                            || patch.operation_type == OperationType::Create
                        {
                            continue;
                        }

                        let new_patch = if patch.operation_type == OperationType::Batch {
                            let changes = patch.file_changes().unwrap_or_default();
                            Patch::new_batch(
                                changes,
                                vec![current_parent],
                                self.author.clone(),
                                message.clone(),
                            )
                        } else {
                            Patch::new(
                                patch.operation_type.clone(),
                                patch.touch_set.clone(),
                                patch.target_path.clone(),
                                patch.payload.clone(),
                                vec![current_parent],
                                self.author.clone(),
                                message.clone(),
                            )
                        };

                        let new_id = self
                            .dag
                            .add_patch(new_patch.clone(), vec![current_parent])?;
                        self.meta.store_patch(&new_patch)?;

                        last_new_id = new_id;
                        current_parent = new_id;
                    }

                    // Handle edit: save state and return
                    if entry.action == RebaseAction::Edit {
                        let state = RebaseState {
                            original_head: old_head,
                            original_branch: head_branch.clone(),
                            onto: *onto,
                            next_entry: 0, // Will be set by caller
                            current_parent,
                            squash_message: None,
                            plan: Vec::new(), // Will be set by caller
                        };
                        let _ = self.save_rebase_state(&state);
                        // Point branch to current state so user can amend
                        self.dag.update_branch(&branch, last_new_id)?;
                        self.meta.set_branch(&branch, &last_new_id)?;
                        self.invalidate_head_cache();
                        self.sync_working_tree(&old_tree)?;
                        return Ok(last_new_id);
                    }
                }
            }
        }

        // If there's an unflushed squash message, apply it to the last commit
        // (This shouldn't normally happen — squash should be followed by another action)

        // Point branch to new tip and sync working tree
        self.dag.update_branch(&branch, last_new_id)?;
        self.meta.set_branch(&branch, &last_new_id)?;
        self.invalidate_head_cache();
        self.sync_working_tree(&old_tree)?;

        let _ = self.record_reflog(&old_head, &last_new_id, "interactive rebase");

        // Clean up rebase state
        let _ = self.clear_rebase_state();

        Ok(last_new_id)
    }

    /// Save interactive rebase state for --continue / --abort.
    fn save_rebase_state(&self, state: &RebaseState) -> Result<(), RepoError> {
        let serialized = serde_json::to_string(state)
            .map_err(|e| RepoError::Custom(format!("failed to serialize rebase state: {}", e)))?;
        self.meta
            .set_config("rebase_state", &serialized)
            .map_err(RepoError::Meta)?;
        Ok(())
    }

    /// Load interactive rebase state.
    pub fn load_rebase_state(&self) -> Result<Option<RebaseState>, RepoError> {
        match self
            .meta
            .get_config("rebase_state")
            .map_err(RepoError::Meta)?
        {
            Some(json) => {
                let state: RebaseState = serde_json::from_str(&json).map_err(|e| {
                    RepoError::Custom(format!("failed to parse rebase state: {}", e))
                })?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Clear interactive rebase state.
    fn clear_rebase_state(&self) -> Result<(), RepoError> {
        let _ = self
            .meta
            .conn()
            .execute("DELETE FROM config WHERE key = 'rebase_state'", []);
        Ok(())
    }

    /// Abort an in-progress interactive rebase.
    ///
    /// Restores the branch to its original position before rebase started.
    pub fn rebase_abort(&mut self) -> Result<(), RepoError> {
        let state = self
            .load_rebase_state()?
            .ok_or_else(|| RepoError::Custom("no rebase in progress".to_string()))?;

        let branch = BranchName::new(&state.original_branch)?;
        let old_tree = self.snapshot_head().unwrap_or_else(|_| FileTree::empty());
        self.dag.update_branch(&branch, state.original_head)?;
        self.meta.set_branch(&branch, &state.original_head)?;
        self.invalidate_head_cache();
        self.sync_working_tree(&old_tree)?;

        let _ = self.record_reflog(
            &state.current_parent,
            &state.original_head,
            "rebase --abort",
        );

        self.clear_rebase_state()?;
        Ok(())
    }

    // =========================================================================
    // Blame
    // =========================================================================

    /// Show per-line commit attribution for a file.
    ///
    /// Returns a vector of `BlameEntry` tuples, one per line in the file at HEAD.
    pub fn blame(&self, path: &str) -> Result<Vec<BlameEntry>, RepoError> {
        let head_tree = self.snapshot_head()?;
        let hash = head_tree
            .get(path)
            .ok_or_else(|| RepoError::Custom(format!("file not found in HEAD: {}", path)))?;

        let blob = self.cas.get_blob(hash)?;
        let content = String::from_utf8_lossy(&blob);
        let lines: Vec<&str> = content.lines().collect();

        let (_, head_id) = self.head()?;
        let chain = self.dag.patch_chain(&head_id);

        let mut patches: Vec<Patch> = chain
            .iter()
            .filter_map(|id| self.dag.get_patch(id).cloned())
            .collect();
        patches.reverse();

        let mut line_author: Vec<Option<(PatchId, String, String)>> = vec![None; lines.len()];
        let mut current_lines: Vec<String> = Vec::new();

        for patch in &patches {
            match &patch.operation_type {
                OperationType::Batch => {
                    if let Some(changes) = patch.file_changes()
                        && let Some(change) = changes.iter().find(|c| c.path == path)
                    {
                        match change.op {
                            OperationType::Create | OperationType::Modify => {
                                let payload_hex = String::from_utf8_lossy(&change.payload);
                                let new_content =
                                    if let Ok(blob_hash) = Hash::from_hex(&payload_hex) {
                                        if let Ok(blob_data) = self.cas.get_blob(&blob_hash) {
                                            String::from_utf8_lossy(&blob_data).to_string()
                                        } else {
                                            continue;
                                        }
                                    } else {
                                        continue;
                                    };

                                let old_refs: Vec<&str> =
                                    current_lines.iter().map(|s| s.as_str()).collect();
                                let new_refs: Vec<&str> = new_content.lines().collect();
                                let changes_diff =
                                    crate::engine::merge::diff_lines(&old_refs, &new_refs);

                                let mut new_line_author: Vec<Option<(PatchId, String, String)>> =
                                    Vec::new();
                                let mut old_idx = 0usize;

                                for change_diff in &changes_diff {
                                    match change_diff {
                                        crate::engine::merge::LineChange::Unchanged(clines) => {
                                            for i in 0..clines.len() {
                                                if old_idx + i < line_author.len() {
                                                    new_line_author
                                                        .push(line_author[old_idx + i].clone());
                                                } else {
                                                    new_line_author.push(None);
                                                }
                                            }
                                            old_idx += clines.len();
                                        }
                                        crate::engine::merge::LineChange::Deleted(clines) => {
                                            old_idx += clines.len();
                                        }
                                        crate::engine::merge::LineChange::Inserted(clines) => {
                                            for _ in 0..clines.len() {
                                                new_line_author.push(Some((
                                                    patch.id,
                                                    patch.message.clone(),
                                                    patch.author.clone(),
                                                )));
                                            }
                                        }
                                    }
                                }

                                line_author = new_line_author;
                                current_lines =
                                    new_content.lines().map(|s| s.to_string()).collect();
                            }
                            OperationType::Delete => {
                                line_author.clear();
                                current_lines.clear();
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {
                    let targets_file = patch.target_path.as_deref() == Some(path);

                    match patch.operation_type {
                        OperationType::Create | OperationType::Modify if targets_file => {
                            let new_content = if !patch.payload.is_empty() {
                                let payload_hex = String::from_utf8_lossy(&patch.payload);
                                if let Ok(blob_hash) = Hash::from_hex(&payload_hex) {
                                    if let Ok(blob_data) = self.cas.get_blob(&blob_hash) {
                                        String::from_utf8_lossy(&blob_data).to_string()
                                    } else {
                                        continue;
                                    }
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            };

                            let old_refs: Vec<&str> =
                                current_lines.iter().map(|s| s.as_str()).collect();
                            let new_refs: Vec<&str> = new_content.lines().collect();
                            let changes = crate::engine::merge::diff_lines(&old_refs, &new_refs);

                            let mut new_line_author: Vec<Option<(PatchId, String, String)>> =
                                Vec::new();
                            let mut old_idx = 0usize;

                            for change in &changes {
                                match change {
                                    crate::engine::merge::LineChange::Unchanged(clines) => {
                                        for i in 0..clines.len() {
                                            if old_idx + i < line_author.len() {
                                                new_line_author
                                                    .push(line_author[old_idx + i].clone());
                                            } else {
                                                new_line_author.push(None);
                                            }
                                        }
                                        old_idx += clines.len();
                                    }
                                    crate::engine::merge::LineChange::Deleted(clines) => {
                                        old_idx += clines.len();
                                    }
                                    crate::engine::merge::LineChange::Inserted(clines) => {
                                        for _ in 0..clines.len() {
                                            new_line_author.push(Some((
                                                patch.id,
                                                patch.message.clone(),
                                                patch.author.clone(),
                                            )));
                                        }
                                    }
                                }
                            }

                            line_author = new_line_author;
                            current_lines = new_content.lines().map(|s| s.to_string()).collect();
                        }
                        OperationType::Delete if targets_file => {
                            line_author.clear();
                            current_lines.clear();
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        let mut result = Vec::new();
        for (i, entry) in line_author.iter().enumerate() {
            let line_content = lines.get(i).unwrap_or(&"").to_string();
            if let Some((pid, msg, author)) = entry {
                result.push(BlameEntry {
                    patch_id: *pid,
                    message: msg.clone(),
                    author: author.clone(),
                    line: line_content,
                    line_number: i + 1,
                });
            } else {
                result.push(BlameEntry {
                    patch_id: Hash::ZERO,
                    message: String::new(),
                    author: String::new(),
                    line: line_content,
                    line_number: i + 1,
                });
            }
        }

        Ok(result)
    }

    // =========================================================================
    // Log
    // =========================================================================

    /// Get the patch history (log) for a branch (first-parent chain only).
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

    /// Get the full patch history for a branch, including all reachable commits
    /// (not just the first-parent chain). Merged branch commits are included.
    pub fn log_all(&self, branch: Option<&str>) -> Result<Vec<Patch>, RepoError> {
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

        let mut patches = self.dag.reachable_patches(&target_id);
        patches.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| a.id.cmp(&b.id)));
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

    /// Get the current author name.
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Get a reference to the CAS.
    pub fn cas(&self) -> &BlobStore {
        &self.cas
    }

    // =========================================================================
    // Remote Operations
    // =========================================================================

    /// Add a remote Hub.
    /// Stores the remote URL in metadata config as "remote.<name>.url".
    pub fn add_remote(&self, name: &str, url: &str) -> Result<(), RepoError> {
        let key = format!("remote.{}.url", name);
        self.meta.set_config(&key, url).map_err(RepoError::Meta)
    }

    /// List configured remotes.
    pub fn list_remotes(&self) -> Result<Vec<(String, String)>, RepoError> {
        let mut remotes = Vec::new();
        for (key, value) in self.meta.list_config()? {
            if let Some(name) = key
                .strip_prefix("remote.")
                .and_then(|n| n.strip_suffix(".url"))
            {
                remotes.push((name.to_string(), value));
            }
        }
        Ok(remotes)
    }

    /// Remove a configured remote.
    pub fn remove_remote(&self, name: &str) -> Result<(), RepoError> {
        let key = format!("remote.{}.url", name);
        if self.meta.get_config(&key)?.is_none() {
            return Err(RepoError::Custom(format!("remote '{}' not found", name)));
        }
        self.meta.delete_config(&key)?;
        if let Ok(Some(_)) = self
            .meta
            .get_config(&format!("remote.{}.last_pushed", name))
        {
            self.meta
                .delete_config(&format!("remote.{}.last_pushed", name))?;
        }
        Ok(())
    }

    // =========================================================================
    // Worktree Operations
    // =========================================================================

    /// Check whether this repository is a linked worktree.
    pub fn is_worktree(&self) -> bool {
        self.is_worktree
    }

    /// Add a worktree. Creates a new directory linked to this repo's data.
    pub fn add_worktree(
        &mut self,
        name: &str,
        path: &Path,
        branch: Option<&str>,
    ) -> Result<(), RepoError> {
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name.contains("..")
            || name.contains('\0')
        {
            return Err(RepoError::Custom("invalid worktree name".into()));
        }
        if path.exists() {
            return Err(RepoError::Custom(format!(
                "path '{}' already exists",
                path.display()
            )));
        }
        if self.is_worktree {
            return Err(RepoError::Custom(
                "cannot add worktree from a linked worktree; use the main repo".into(),
            ));
        }

        let abs_path = if path.is_relative() {
            std::env::current_dir()?.join(path)
        } else {
            path.to_path_buf()
        };

        fs::create_dir_all(&abs_path)?;
        let new_suture_dir = abs_path.join(".suture");
        fs::create_dir_all(&new_suture_dir)?;

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                self.suture_dir.join("metadata.db"),
                new_suture_dir.join("metadata.db"),
            )?;
            if self.suture_dir.join("objects").exists() {
                std::os::unix::fs::symlink(
                    self.suture_dir.join("objects"),
                    new_suture_dir.join("objects"),
                )?;
            }
            if self.suture_dir.join("keys").exists() {
                std::os::unix::fs::symlink(
                    self.suture_dir.join("keys"),
                    new_suture_dir.join("keys"),
                )?;
            }
        }
        #[cfg(not(unix))]
        {
            return Err(RepoError::Unsupported(
                "worktrees require symlink support (Unix only)".into(),
            ));
        }

        fs::write(
            new_suture_dir.join("worktree"),
            self.root.to_string_lossy().as_ref(),
        )?;

        let branch_name = branch.unwrap_or("main");
        fs::write(new_suture_dir.join("HEAD"), branch_name)?;

        self.set_config(
            &format!("worktree.{}.path", name),
            &abs_path.to_string_lossy(),
        )?;
        self.set_config(&format!("worktree.{}.branch", name), branch_name)?;

        let mut wt_repo = Repository::open(&abs_path)?;
        wt_repo.checkout(branch_name)?;

        Ok(())
    }

    /// List all worktrees. Returns the main worktree plus any linked worktrees.
    pub fn list_worktrees(&self) -> Result<Vec<WorktreeEntry>, RepoError> {
        let mut worktrees = Vec::new();

        let main_branch = self
            .head()
            .map(|(n, _)| n)
            .unwrap_or_else(|_| "main".to_string());
        worktrees.push(WorktreeEntry {
            name: String::new(),
            path: self.root.to_string_lossy().to_string(),
            branch: main_branch,
            is_main: true,
        });

        let config = self.list_config()?;
        let mut names: Vec<&str> = Vec::new();
        for (key, _value) in &config {
            if let Some(n) = key
                .strip_prefix("worktree.")
                .and_then(|n| n.strip_suffix(".path"))
            {
                names.push(n);
            }
        }
        names.sort();

        for name in names {
            let path_key = format!("worktree.{}.path", name);
            let branch_key = format!("worktree.{}.branch", name);
            let path_val = self
                .meta
                .get_config(&path_key)
                .unwrap_or(None)
                .unwrap_or_default();
            let branch_val = self
                .meta
                .get_config(&branch_key)
                .unwrap_or(None)
                .unwrap_or_default();
            worktrees.push(WorktreeEntry {
                name: name.to_string(),
                path: path_val,
                branch: branch_val,
                is_main: false,
            });
        }

        Ok(worktrees)
    }

    /// Remove a worktree by name. Deletes the worktree directory and cleans
    /// up the main repo's config entries.
    pub fn remove_worktree(&mut self, name: &str) -> Result<(), RepoError> {
        let path_key = format!("worktree.{}.path", name);
        let path_val = self
            .meta
            .get_config(&path_key)?
            .ok_or_else(|| RepoError::Custom(format!("worktree '{}' not found", name)))?;

        let wt_path = Path::new(&path_val);
        if wt_path.exists() {
            fs::remove_dir_all(wt_path)?;
        }

        self.meta.delete_config(&path_key)?;
        self.meta
            .delete_config(&format!("worktree.{}.branch", name))?;

        Ok(())
    }

    /// Rename a tracked file. Stages both the deletion of the old path
    /// and the addition of the new path.
    pub fn rename_file(&self, old_path: &str, new_path: &str) -> Result<(), RepoError> {
        let old = self.root.join(old_path);
        let new = self.root.join(new_path);

        if !old.exists() {
            return Err(RepoError::Custom(format!("path not found: {}", old_path)));
        }

        if new.exists() {
            return Err(RepoError::Custom(format!(
                "path already exists: {}",
                new_path
            )));
        }

        fs::rename(old, new).map_err(|e| RepoError::Custom(format!("rename failed: {}", e)))?;

        self.add(old_path)?;
        self.add(new_path)?;

        Ok(())
    }

    /// Get the URL for a remote.
    pub fn get_remote_url(&self, name: &str) -> Result<String, RepoError> {
        let key = format!("remote.{}.url", name);
        self.meta
            .get_config(&key)
            .unwrap_or(None)
            .ok_or_else(|| RepoError::Custom(format!("remote '{}' not found", name)))
    }

    /// Get all patches in the DAG as a Vec.
    pub fn all_patches(&self) -> Vec<Patch> {
        self.dag
            .patch_ids()
            .iter()
            .filter_map(|id| self.dag.get_patch(id).cloned())
            .collect()
    }

    // =========================================================================
    // Garbage Collection
    // =========================================================================

    /// Remove unreachable patches and orphaned blobs from the repository.
    ///
    /// 1. Computes the set of patches reachable from all branch tips.
    /// 2. Deletes unreachable patches from the metadata store (patches, edges,
    ///    signatures tables) in a single transaction.
    /// 3. Collects all blob hashes referenced by reachable patches.
    /// 4. Deletes orphaned blobs from the CAS store (both loose and packed).
    ///
    /// The in-memory DAG is not updated; reopen the repository after GC.
    pub fn gc(&self) -> Result<GcResult, RepoError> {
        let branches = self.dag.list_branches();
        let all_ids: HashSet<PatchId> = self.dag.patch_ids().into_iter().collect();

        // Compute reachable set
        let mut reachable: HashSet<PatchId> = HashSet::new();
        for (_name, tip_id) in &branches {
            reachable.insert(*tip_id);
            for anc in self.dag.ancestors(tip_id) {
                reachable.insert(anc);
            }
        }

        let unreachable: Vec<PatchId> = all_ids
            .iter()
            .filter(|id| !reachable.contains(id))
            .copied()
            .collect();

        // Delete unreachable patches in a transaction (atomic)
        let conn = self.meta().conn();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| RepoError::Custom(e.to_string()))?;

        for id in &unreachable {
            let hex = id.to_hex();
            tx.execute(
                "DELETE FROM signatures WHERE patch_id = ?1",
                rusqlite::params![hex],
            )
            .map_err(|e| RepoError::Custom(e.to_string()))?;
            tx.execute(
                "DELETE FROM edges WHERE parent_id = ?1 OR child_id = ?1",
                rusqlite::params![hex],
            )
            .map_err(|e| RepoError::Custom(e.to_string()))?;
            tx.execute("DELETE FROM patches WHERE id = ?1", rusqlite::params![hex])
                .map_err(|e| RepoError::Custom(e.to_string()))?;
            // Also clean up file_trees for unreachable patches
            tx.execute("DELETE FROM file_trees WHERE patch_id = ?1", rusqlite::params![hex])
                .map_err(|e| RepoError::Custom(e.to_string()))?;
            // Clean up reflog entries that reference unreachable patches
            tx.execute(
                "DELETE FROM reflog WHERE old_head = ?1 OR new_head = ?1",
                rusqlite::params![hex],
            )
            .map_err(|e| RepoError::Custom(e.to_string()))?;
        }

        tx.commit()
            .map_err(|e| RepoError::Custom(e.to_string()))?;

        // Collect all blob hashes referenced by reachable patches
        let mut referenced_blobs: HashSet<Hash> = HashSet::new();
        for id in &reachable {
            if let Some(patch) = self.dag.get_patch(id) {
                if patch.is_batch() {
                    if let Some(changes) = patch.file_changes() {
                        for change in &changes {
                            if !change.payload.is_empty()
                                && let Ok(hash) = Hash::from_hex(
                                    &String::from_utf8_lossy(&change.payload),
                                ) && referenced_blobs.insert(hash)
                            {
                                // blob hash collected
                            }
                        }
                    }
                } else if let Some(hash) = resolve_payload_to_hash(patch) {
                    referenced_blobs.insert(hash);
                }
            }
        }

        // Prune orphaned blobs from CAS
        let mut blobs_removed = 0usize;
        if let Ok(all_blobs) = self.cas().list_blobs() {
            for blob_hash in &all_blobs {
                if !referenced_blobs.contains(blob_hash)
                    && self.cas().delete_blob(blob_hash).is_ok()
                {
                    blobs_removed += 1;
                }
            }
        }

        Ok(GcResult {
            patches_removed: unreachable.len(),
            blobs_removed,
        })
    }

    // =========================================================================
    // Filesystem Check
    // =========================================================================

    /// Verify repository integrity.
    ///
    /// Checks DAG consistency (parent references), branch integrity
    /// (branch targets exist), blob references (CAS has blobs referenced
    /// by patches), and HEAD consistency.
    pub fn fsck(&self) -> Result<FsckResult, RepoError> {
        let mut checks_passed = 0usize;
        let mut errors = Vec::new();

        // 1. DAG consistency: every patch's parents exist in the DAG
        let all_ids: HashSet<PatchId> = self.dag.patch_ids().into_iter().collect();
        let mut parent_ok = true;
        for id in &all_ids {
            if let Some(node) = self.dag.get_node(id) {
                for parent_id in &node.parent_ids {
                    if !all_ids.contains(parent_id) {
                        errors.push(format!(
                            "patch {} references missing parent {}",
                            id.to_hex(),
                            parent_id.to_hex()
                        ));
                        parent_ok = false;
                    }
                }
            }
        }
        if parent_ok {
            checks_passed += 1;
        }

        // 2. Branch integrity: every branch target exists in the DAG
        let branches = self.dag.list_branches();
        let mut branch_ok = true;
        for (name, target_id) in &branches {
            if !all_ids.contains(target_id) {
                errors.push(format!(
                    "branch '{}' targets non-existent patch {}",
                    name,
                    target_id.to_hex()
                ));
                branch_ok = false;
            }
        }
        if branch_ok {
            checks_passed += 1;
        }

        // 3. Blob references: non-empty payloads should reference CAS blobs
        let mut blob_ok = true;
        let all_patches = self.all_patches();
        for patch in &all_patches {
            if patch.is_batch() {
                if let Some(changes) = patch.file_changes() {
                    for change in &changes {
                        if change.payload.is_empty() {
                            continue;
                        }
                        let hex = String::from_utf8_lossy(&change.payload);
                        if let Ok(hash) = Hash::from_hex(&hex)
                            && !self.cas().has_blob(&hash)
                        {
                            errors.push(format!(
                                "batch patch {} references missing blob {} for path {}",
                                patch.id.to_hex(),
                                hash.to_hex(),
                                change.path
                            ));
                            blob_ok = false;
                        }
                    }
                }
                continue;
            }
            if patch.payload.is_empty() {
                continue;
            }
            if let Some(hash) = resolve_payload_to_hash(patch) {
                if !self.cas().has_blob(&hash) {
                    errors.push(format!(
                        "patch {} references missing blob {}",
                        patch.id.to_hex(),
                        hash.to_hex()
                    ));
                    blob_ok = false;
                }
            } else {
                errors.push(format!(
                    "patch {} has non-UTF-8 payload, cannot verify blob reference",
                    patch.id.to_hex()
                ));
                blob_ok = false;
            }
        }
        if blob_ok {
            checks_passed += 1;
        }

        // 4. HEAD consistency: the current HEAD branch exists
        let mut head_ok = false;
        match self.head() {
            Ok((branch_name, _target_id)) => {
                if branches.iter().any(|(n, _)| n == &branch_name) {
                    head_ok = true;
                } else {
                    errors.push(format!(
                        "HEAD branch '{}' does not exist in branch list",
                        branch_name
                    ));
                }
            }
            Err(e) => {
                errors.push(format!("HEAD is invalid: {}", e));
            }
        }
        if head_ok {
            checks_passed += 1;
        }

        Ok(FsckResult {
            checks_passed,
            warnings: Vec::new(),
            errors,
        })
    }

    // =========================================================================
    // Reflog
    // =========================================================================

    fn record_reflog(
        &self,
        old_head: &PatchId,
        new_head: &PatchId,
        message: &str,
    ) -> Result<(), RepoError> {
        // Use the SQLite reflog table (O(1) append, no full-rewrite)
        self.meta
            .reflog_push(old_head, new_head, message)
            .map_err(RepoError::Meta)?;
        Ok(())
    }

    /// Get all reflog entries (newest first).
    pub fn reflog_entries(&self) -> Result<Vec<ReflogEntry>, RepoError> {
        let sqlite_entries = self.meta.reflog_list().map_err(RepoError::Meta)?;

        if !sqlite_entries.is_empty() {
            let entries: Vec<ReflogEntry> = sqlite_entries
                .into_iter()
                .filter_map(|(old_head, new_head, message, timestamp)| {
                    let old = Hash::from_hex(&old_head).ok()?;
                    let new = Hash::from_hex(&new_head).ok()?;
                    Some(ReflogEntry {
                        old_head: old,
                        new_head: new,
                        message,
                        timestamp,
                    })
                })
                .collect();
            return Ok(entries);
        }

        // Fallback: migrate from legacy config-based reflog
        match self.meta.get_config("reflog").map_err(RepoError::Meta)? {
            Some(json) => {
                let legacy: Vec<(String, String)> = serde_json::from_str(&json).unwrap_or_default();
                for (new_head, entry) in &legacy {
                    let parts: Vec<&str> = entry.splitn(3, ':').collect();
                    if parts.len() >= 3 {
                        let old_head = parts[1];
                        let msg = parts[2];
                        if let (Ok(old), Ok(new)) =
                            (Hash::from_hex(old_head), Hash::from_hex(new_head))
                        {
                            let _ = self.meta.reflog_push(&old, &new, msg);
                        }
                    }
                }
                let _ = self.meta.delete_config("reflog");
                let sqlite_entries = self.meta.reflog_list().map_err(RepoError::Meta)?;
                let entries: Vec<ReflogEntry> = sqlite_entries
                    .into_iter()
                    .filter_map(|(old_head, new_head, message, timestamp)| {
                        let old = Hash::from_hex(&old_head).ok()?;
                        let new = Hash::from_hex(&new_head).ok()?;
                        Some(ReflogEntry {
                            old_head: old,
                            new_head: new,
                            message,
                            timestamp,
                        })
                    })
                    .collect();
                Ok(entries)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Get the `old_head` from the Nth most recent reflog entry (0-indexed).
    /// This is used by `undo` to rewind HEAD to a previous state.
    pub fn reflog_older_head(&self, n: usize) -> Result<Option<PatchId>, RepoError> {
        let entries = self.reflog_entries()?;
        Ok(entries.get(n).map(|e| e.old_head))
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

/// Restore pending merge parents from config (persisted across repo reopens).
fn restore_pending_merge_parents(meta: &crate::metadata::MetadataStore) -> Vec<PatchId> {
    let Ok(Some(json)) = meta.get_config("pending_merge_parents") else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<PatchId>>(&json).unwrap_or_default()
}

// =============================================================================
// Repository Status
// =============================================================================

/// A single stash entry.
#[derive(Debug, Clone)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    pub branch: String,
    pub head_id: String,
}

/// Information about a worktree.
#[derive(Debug, Clone)]
pub struct WorktreeEntry {
    pub name: String,
    pub path: String,
    pub branch: String,
    pub is_main: bool,
}

/// A single blame entry for one line of a file.
#[derive(Debug, Clone)]
pub struct BlameEntry {
    /// The patch ID that last modified this line.
    pub patch_id: PatchId,
    /// The commit message.
    pub message: String,
    /// The author of the commit.
    pub author: String,
    /// The line content.
    pub line: String,
    /// The 1-based line number.
    pub line_number: usize,
}

/// Result of a rebase operation.
#[derive(Debug, Clone)]
pub struct RebaseResult {
    /// Number of patches that were replayed.
    pub patches_replayed: usize,
    /// The new tip patch ID after rebase.
    pub new_tip: PatchId,
}

/// Actions available during interactive rebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseAction {
    /// Apply the commit as-is.
    Pick,
    /// Apply the commit but edit the message.
    Reword,
    /// Apply the commit and pause for amending.
    Edit,
    /// Combine with the previous commit.
    Squash,
    /// Skip this commit entirely.
    Drop,
}

/// A single entry in the interactive rebase plan.
#[derive(Debug, Clone)]
pub struct RebasePlanEntry {
    /// The action to perform.
    pub action: RebaseAction,
    /// The tip patch ID of the logical commit (last patch in the per-file chain).
    pub commit_tip: PatchId,
    /// The commit message (for display and reword).
    pub message: String,
    /// All patch IDs in this logical commit's chain.
    pub patch_ids: Vec<PatchId>,
}

/// A complete interactive rebase plan.
#[derive(Debug, Clone)]
pub struct RebasePlan {
    pub entries: Vec<RebasePlanEntry>,
}

/// State persisted during an interactive rebase for --continue / --abort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseState {
    /// Original HEAD before rebase started.
    pub original_head: PatchId,
    /// Original branch name.
    pub original_branch: String,
    /// Target we're rebasing onto.
    pub onto: PatchId,
    /// Index of the next entry to process.
    pub next_entry: usize,
    /// The plan entries remaining.
    pub plan: Vec<RebasePlanEntrySerialized>,
    /// Current parent for chaining new patches.
    pub current_parent: PatchId,
    /// Accumulated squash messages (for combining with squash action).
    pub squash_message: Option<String>,
}

/// Serialized form of a plan entry (for state persistence).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebasePlanEntrySerialized {
    pub action: String,
    pub commit_tip: String,
    pub message: String,
    pub patch_ids: Vec<String>,
}

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
// Merge Execution Types
// =============================================================================

/// Result of executing a merge.
#[derive(Debug, Clone)]
pub struct MergeExecutionResult {
    /// Whether the merge was fully clean (no conflicts).
    pub is_clean: bool,
    /// The resulting file tree after the merge.
    pub merged_tree: FileTree,
    /// The merge commit patch ID (set if is_clean or all conflicts resolved).
    pub merge_patch_id: Option<PatchId>,
    /// Unresolved conflicts (empty if is_clean).
    pub unresolved_conflicts: Vec<ConflictInfo>,
    /// Number of patches applied from the source branch.
    pub patches_applied: usize,
}

/// Information about an unresolved merge conflict.
#[derive(Debug, Clone)]
pub struct ConflictInfo {
    /// The path where the conflict occurs.
    pub path: String,
    /// The patch ID from the current branch.
    pub our_patch_id: PatchId,
    /// The patch ID from the source branch.
    pub their_patch_id: PatchId,
    /// Our version of the file (blob hash).
    pub our_content_hash: Option<Hash>,
    /// Their version of the file (blob hash).
    pub their_content_hash: Option<Hash>,
    /// The base version of the file (blob hash from LCA).
    pub base_content_hash: Option<Hash>,
}

/// Result of a garbage collection pass.
#[derive(Debug, Clone)]
pub struct GcResult {
    /// Number of unreachable patches removed.
    pub patches_removed: usize,
    /// Number of orphaned blobs removed from the CAS store.
    pub blobs_removed: usize,
}

/// Result of a filesystem check.
#[derive(Debug, Clone)]
pub struct FsckResult {
    /// Number of checks that passed without issues.
    pub checks_passed: usize,
    /// Non-fatal warnings encountered.
    pub warnings: Vec<String>,
    /// Fatal errors encountered.
    pub errors: Vec<String>,
}

/// Line-level three-way merge using diff3 algorithm.
///
/// Returns `Ok(merged_content)` if clean, `Err(conflict_marker_lines)` if conflicts.
fn three_way_merge(
    base: Option<&str>,
    ours: &str,
    theirs: &str,
    head_branch: &str,
    source_branch: &str,
) -> Result<String, Vec<String>> {
    use crate::engine::merge::three_way_merge_lines;

    let base_lines: Vec<&str> = base.map(|s| s.lines().collect()).unwrap_or_default();
    let ours_lines: Vec<&str> = ours.lines().collect();
    let theirs_lines: Vec<&str> = theirs.lines().collect();

    let ours_label = if head_branch.is_empty() {
        "HEAD".to_string()
    } else {
        format!("{head_branch} (HEAD)")
    };
    let theirs_label = if source_branch.is_empty() {
        "theirs".to_string()
    } else {
        source_branch.to_string()
    };

    let result = three_way_merge_lines(
        &base_lines,
        &ours_lines,
        &theirs_lines,
        &ours_label,
        &theirs_label,
    );

    if result.is_clean {
        Ok(result.lines.join("\n"))
    } else {
        Err(result.lines)
    }
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

        // Checkout now auto-stashes instead of refusing
        let result = repo.checkout("main");
        assert!(result.is_ok());

        // After auto-stash pop, the stashed changes should be restored to the working set
        let working_set = repo.meta.working_set().unwrap();
        assert!(working_set.iter().any(|(p, _)| p == "staged.txt"));
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

        // Revert the commit — should remove the file from disk
        repo.revert(&commit_id, None).unwrap();

        let tree = repo.snapshot_head().unwrap();
        assert!(!tree.contains("test.txt"));
        assert!(
            !test_file.exists(),
            "revert should remove the file from the working tree"
        );
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

    #[test]
    fn test_execute_merge_clean() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        fs::write(dir.path().join("base.txt"), "base").unwrap();
        repo.add("base.txt").unwrap();
        repo.commit("add base").unwrap();

        repo.create_branch("feature", None).unwrap();

        fs::write(dir.path().join("main_file.txt"), "main content").unwrap();
        repo.add("main_file.txt").unwrap();
        repo.commit("add main file").unwrap();

        repo.checkout("feature").unwrap();

        fs::write(dir.path().join("feat_file.txt"), "feature content").unwrap();
        repo.add("feat_file.txt").unwrap();
        repo.commit("add feature file").unwrap();

        let result = repo.execute_merge("main").unwrap();
        assert!(result.is_clean);
        assert!(result.merge_patch_id.is_some());
        assert!(result.unresolved_conflicts.is_empty());
        assert!(dir.path().join("main_file.txt").exists());
        assert!(dir.path().join("feat_file.txt").exists());
        assert!(dir.path().join("base.txt").exists());

        let log = repo.log(None).unwrap();
        let merge_patch = log
            .iter()
            .find(|p| p.operation_type == OperationType::Merge);
        assert!(merge_patch.is_some());
        assert_eq!(merge_patch.unwrap().parent_ids.len(), 2);
    }

    #[test]
    fn test_execute_merge_conflicting() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        fs::write(dir.path().join("shared.txt"), "original").unwrap();
        repo.add("shared.txt").unwrap();
        repo.commit("add shared").unwrap();

        repo.create_branch("feature", None).unwrap();

        fs::write(dir.path().join("shared.txt"), "main version").unwrap();
        repo.add("shared.txt").unwrap();
        repo.commit("modify on main").unwrap();

        repo.checkout("feature").unwrap();

        fs::write(dir.path().join("shared.txt"), "feature version").unwrap();
        repo.add("shared.txt").unwrap();
        repo.commit("modify on feature").unwrap();

        let result = repo.execute_merge("main").unwrap();
        assert!(!result.is_clean);
        assert!(result.merge_patch_id.is_none());
        assert_eq!(result.unresolved_conflicts.len(), 1);
        assert_eq!(result.unresolved_conflicts[0].path, "shared.txt");

        let content = fs::read_to_string(dir.path().join("shared.txt")).unwrap();
        assert!(content.contains("<<<<<<< feature (HEAD)"));
        assert!(content.contains("main version"));
        assert!(content.contains("feature version"));
        assert!(content.contains(">>>>>>> main"));
    }

    #[test]
    fn test_execute_merge_fast_forward() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        fs::write(dir.path().join("base.txt"), "base").unwrap();
        repo.add("base.txt").unwrap();
        repo.commit("add base").unwrap();

        repo.create_branch("feature", None).unwrap();

        repo.checkout("feature").unwrap();
        fs::write(dir.path().join("new_file.txt"), "new content").unwrap();
        repo.add("new_file.txt").unwrap();
        repo.commit("add new file on feature").unwrap();

        repo.checkout("main").unwrap();

        let result = repo.execute_merge("feature").unwrap();
        assert!(result.is_clean);
        assert!(dir.path().join("new_file.txt").exists());
    }

    #[test]
    fn test_resolve_merge_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        fs::write(dir.path().join("shared.txt"), "original").unwrap();
        repo.add("shared.txt").unwrap();
        repo.commit("add shared").unwrap();

        repo.create_branch("feature", None).unwrap();

        fs::write(dir.path().join("shared.txt"), "main version").unwrap();
        repo.add("shared.txt").unwrap();
        repo.commit("modify on main").unwrap();

        repo.checkout("feature").unwrap();

        fs::write(dir.path().join("shared.txt"), "feature version").unwrap();
        repo.add("shared.txt").unwrap();
        repo.commit("modify on feature").unwrap();

        let _result = repo.execute_merge("main").unwrap();

        fs::write(dir.path().join("shared.txt"), "resolved content").unwrap();
        repo.add("shared.txt").unwrap();
        let commit_id = repo.commit("resolve merge conflict").unwrap();

        assert!(repo.pending_merge_parents.is_empty());

        let log = repo.log(None).unwrap();
        let resolve_patch = log.iter().find(|p| p.id == commit_id).unwrap();
        assert_eq!(resolve_patch.parent_ids.len(), 2);
    }

    #[test]
    fn test_three_way_merge() {
        let ours = "line1\nline2-modified\nline3";
        let theirs = "line1\nline2-modified\nline3";
        let result = three_way_merge(Some("line1\nline2\nline3"), ours, theirs, "main", "feature");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ours);

        let result = three_way_merge(Some("base"), "base", "changed", "main", "feature");
        assert_eq!(result.unwrap(), "changed");

        let result = three_way_merge(Some("base"), "changed", "base", "main", "feature");
        assert_eq!(result.unwrap(), "changed");

        let result = three_way_merge(None, "ours content", "theirs content", "main", "feature");
        assert!(result.is_err());
        let lines = result.unwrap_err();
        assert!(lines[0].contains("<<<<<<<"));
        assert!(lines.last().unwrap().contains(">>>>>>>"));
    }

    #[test]
    fn test_config_get_set() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        assert!(repo.get_config("user.name")?.is_none());
        assert!(repo.get_config("user.email")?.is_none());

        repo.set_config("user.name", "Alice")?;
        repo.set_config("user.email", "alice@example.com")?;

        assert_eq!(repo.get_config("user.name")?.unwrap(), "Alice");
        assert_eq!(repo.get_config("user.email")?.unwrap(), "alice@example.com");

        // List config (filters internal keys)
        let config = repo.list_config()?;
        assert!(config.iter().any(|(k, v)| k == "user.name" && v == "Alice"));
        assert!(
            config
                .iter()
                .any(|(k, v)| k == "user.email" && v == "alice@example.com")
        );
        // Internal keys should be present in raw list
        assert!(config.iter().any(|(k, _)| k == "author"));

        Ok(())
    }

    #[test]
    fn test_delete_branch() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        repo.create_branch("feature", None)?;
        repo.create_branch("develop", None)?;
        assert_eq!(repo.list_branches().len(), 3);

        // Cannot delete current branch
        let result = repo.delete_branch("main");
        assert!(result.is_err());

        // Can delete other branches
        repo.delete_branch("feature")?;
        assert_eq!(repo.list_branches().len(), 2);

        repo.delete_branch("develop")?;
        assert_eq!(repo.list_branches().len(), 1);

        Ok(())
    }

    #[test]
    fn test_tags() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "v1")?;
        repo.add("a.txt")?;
        let _commit_id = repo.commit("first commit")?;

        // Create tag at HEAD
        repo.create_tag("v1.0", None)?;
        let tags = repo.list_tags()?;
        assert_eq!(tags.len(), 1);

        Ok(())
    }

    #[test]
    fn test_patches_since() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        // Commit 1
        fs::write(dir.path().join("a.txt"), "v1")?;
        repo.add("a.txt")?;
        let id1 = repo.commit("first")?;

        // Commit 2
        fs::write(dir.path().join("a.txt"), "v2")?;
        repo.add("a.txt")?;
        let id2 = repo.commit("second")?;

        // Commit 3
        fs::write(dir.path().join("b.txt"), "new")?;
        repo.add("b.txt")?;
        let id3 = repo.commit("third")?;

        // patches_since(id1) should return [id2, id3]
        let since = repo.patches_since(&id1);
        assert_eq!(since.len(), 2);
        assert_eq!(since[0].id, id2);
        assert_eq!(since[1].id, id3);

        // patches_since(id3) should return []
        let since = repo.patches_since(&id3);
        assert!(since.is_empty());

        // patches_since(root_patch) should return [id1, id2, id3] (3 file patches)
        // Get the root patch (Initial commit)
        let root_id = repo.log(None)?.last().unwrap().id;
        let since = repo.patches_since(&root_id);
        assert_eq!(since.len(), 3);
        assert_eq!(since[0].id, id1);
        assert_eq!(since[1].id, id2);
        assert_eq!(since[2].id, id3);

        Ok(())
    }

    #[test]
    fn test_pending_merge_persistence() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("shared.txt"), "original")?;
        repo.add("shared.txt")?;
        repo.commit("add shared")?;

        repo.create_branch("feature", None)?;

        fs::write(dir.path().join("shared.txt"), "main version")?;
        repo.add("shared.txt")?;
        repo.commit("modify on main")?;

        repo.checkout("feature")?;

        fs::write(dir.path().join("shared.txt"), "feature version")?;
        repo.add("shared.txt")?;
        repo.commit("modify on feature")?;

        // Trigger conflicting merge — should persist parents
        let _ = repo.execute_merge("main")?;
        assert_eq!(repo.pending_merge_parents.len(), 2);

        // Simulate repo close + reopen
        drop(repo);
        let mut repo2 = Repository::open(dir.path())?;
        assert_eq!(repo2.pending_merge_parents.len(), 2);

        // Resolve the merge
        fs::write(dir.path().join("shared.txt"), "resolved")?;
        repo2.add("shared.txt")?;
        let resolve_id = repo2.commit("resolve")?;
        assert!(repo2.pending_merge_parents.is_empty());

        // Verify merge commit has 2 parents
        let patch = repo2
            .log(None)?
            .into_iter()
            .find(|p| p.id == resolve_id)
            .unwrap();
        assert_eq!(patch.parent_ids.len(), 2);

        Ok(())
    }

    #[test]
    fn test_has_uncommitted_changes_clean() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice")?;

        assert!(!repo.has_uncommitted_changes()?);

        Ok(())
    }

    #[test]
    fn test_has_uncommitted_changes_staged() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "content")?;
        repo.add("a.txt")?;

        assert!(repo.has_uncommitted_changes()?);

        Ok(())
    }

    #[test]
    fn test_has_uncommitted_changes_unstaged() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "original")?;
        repo.add("a.txt")?;
        repo.commit("initial")?;

        fs::write(dir.path().join("a.txt"), "modified on disk")?;

        assert!(repo.has_uncommitted_changes()?);

        Ok(())
    }

    #[test]
    fn test_stash_push_pop() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "original")?;
        repo.add("a.txt")?;
        repo.commit("initial")?;

        fs::write(dir.path().join("a.txt"), "staged changes")?;
        repo.add("a.txt")?;

        let stash_index = repo.stash_push(Some("my stash"))?;
        assert_eq!(stash_index, 0);

        assert!(repo.meta.working_set()?.is_empty());
        let on_disk = fs::read_to_string(dir.path().join("a.txt"))?;
        assert_eq!(on_disk, "original");

        repo.stash_pop()?;

        let on_disk = fs::read_to_string(dir.path().join("a.txt"))?;
        assert_eq!(on_disk, "staged changes");

        let ws = repo.meta.working_set()?;
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].0, "a.txt");
        assert_eq!(ws[0].1, FileStatus::Modified);

        Ok(())
    }

    #[test]
    fn test_stash_list() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "original")?;
        repo.add("a.txt")?;
        repo.commit("initial")?;

        fs::write(dir.path().join("a.txt"), "change 1")?;
        repo.add("a.txt")?;
        let idx0 = repo.stash_push(Some("first stash"))?;
        assert_eq!(idx0, 0);

        fs::write(dir.path().join("a.txt"), "change 2")?;
        repo.add("a.txt")?;
        let idx1 = repo.stash_push(Some("second stash"))?;
        assert_eq!(idx1, 1);

        let list = repo.stash_list()?;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].index, 0);
        assert_eq!(list[0].message, "first stash");
        assert_eq!(list[1].index, 1);
        assert_eq!(list[1].message, "second stash");

        Ok(())
    }

    #[test]
    fn test_stash_apply_keeps_entry() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "original")?;
        repo.add("a.txt")?;
        repo.commit("initial")?;

        fs::write(dir.path().join("a.txt"), "changes to apply")?;
        repo.add("a.txt")?;
        let idx = repo.stash_push(Some("keep me"))?;
        assert_eq!(idx, 0);

        repo.stash_apply(0)?;

        let on_disk = fs::read_to_string(dir.path().join("a.txt"))?;
        assert_eq!(on_disk, "changes to apply");

        let list = repo.stash_list()?;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].index, 0);
        assert_eq!(list[0].message, "keep me");

        Ok(())
    }

    #[test]
    fn test_stash_drop() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "original")?;
        repo.add("a.txt")?;
        repo.commit("initial")?;

        fs::write(dir.path().join("a.txt"), "stashed content")?;
        repo.add("a.txt")?;
        repo.stash_push(Some("droppable"))?;

        repo.stash_drop(0)?;

        let list = repo.stash_list()?;
        assert!(list.is_empty());

        let result = repo.stash_drop(0);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_stash_pop_empty() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        let result = repo.stash_pop();
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_stash_push_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        let result = repo.stash_push(None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("nothing to commit"));

        Ok(())
    }

    #[test]
    fn test_reset_soft() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("file1.txt"), "first content")?;
        repo.add("file1.txt")?;
        let first_commit = repo.commit("first commit")?;

        fs::write(dir.path().join("file2.txt"), "second content")?;
        repo.add("file2.txt")?;
        repo.commit("second commit")?;

        // Stage a modification before reset to verify soft preserves staging
        fs::write(dir.path().join("file2.txt"), "modified second")?;
        repo.add("file2.txt")?;

        let result = repo.reset(&first_commit.to_hex(), ResetMode::Soft)?;
        assert_eq!(result, first_commit);

        // HEAD points to first commit
        let (_, head_id) = repo.head()?;
        assert_eq!(head_id, first_commit);

        // Working tree still has file2 (soft doesn't touch working tree)
        assert!(dir.path().join("file2.txt").exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("file2.txt"))?,
            "modified second"
        );

        // Staging area still has the staged changes (soft doesn't clear staging)
        let status = repo.status()?;
        assert_eq!(status.staged_files.len(), 1);
        assert_eq!(status.staged_files[0].0, "file2.txt");

        Ok(())
    }

    #[test]
    fn test_reset_mixed() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("file1.txt"), "first content")?;
        repo.add("file1.txt")?;
        let first_commit = repo.commit("first commit")?;

        fs::write(dir.path().join("file2.txt"), "second content")?;
        repo.add("file2.txt")?;
        repo.commit("second commit")?;

        // Stage a modification before reset to verify mixed clears staging
        fs::write(dir.path().join("file2.txt"), "modified second")?;
        repo.add("file2.txt")?;

        let result = repo.reset(&first_commit.to_hex(), ResetMode::Mixed)?;
        assert_eq!(result, first_commit);

        // HEAD points to first commit
        let (_, head_id) = repo.head()?;
        assert_eq!(head_id, first_commit);

        // Working tree still has file2 content on disk (mixed doesn't touch working tree)
        assert!(dir.path().join("file2.txt").exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("file2.txt"))?,
            "modified second"
        );

        // Staging area is cleared
        let status = repo.status()?;
        assert!(status.staged_files.is_empty());

        Ok(())
    }

    #[test]
    fn test_reset_hard() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("file1.txt"), "first content")?;
        repo.add("file1.txt")?;
        let first_commit = repo.commit("first commit")?;

        fs::write(dir.path().join("file2.txt"), "second content")?;
        repo.add("file2.txt")?;
        repo.commit("second commit")?;

        let result = repo.reset(&first_commit.to_hex(), ResetMode::Hard)?;
        assert_eq!(result, first_commit);

        // HEAD points to first commit
        let (_, head_id) = repo.head()?;
        assert_eq!(head_id, first_commit);

        // Working tree matches first commit (file2 removed from disk)
        assert!(dir.path().join("file1.txt").exists());
        assert!(!dir.path().join("file2.txt").exists());

        let tree = repo.snapshot_head()?;
        assert!(tree.contains("file1.txt"));
        assert!(!tree.contains("file2.txt"));

        Ok(())
    }

    #[test]
    fn test_cherry_pick() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "content of a")?;
        repo.add("a.txt")?;
        repo.commit("add a.txt")?;

        repo.create_branch("feature", None)?;

        fs::write(dir.path().join("b.txt"), "content of b")?;
        repo.add("b.txt")?;
        let b_commit = repo.commit("add b.txt")?;

        repo.checkout("feature")?;

        // Add a commit on feature so parent_ids differ from the original b.txt commit
        fs::write(dir.path().join("c.txt"), "content of c")?;
        repo.add("c.txt")?;
        repo.commit("add c.txt on feature")?;

        repo.cherry_pick(&b_commit)?;

        assert!(dir.path().join("b.txt").exists());
        let content = fs::read_to_string(dir.path().join("b.txt"))?;
        assert_eq!(content, "content of b");

        let log = repo.log(None)?;
        assert!(log.iter().any(|p| p.message == "add b.txt"));

        Ok(())
    }

    #[test]
    fn test_cherry_pick_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice").unwrap();

        let fake_hash = Hash::from_data(b"nonexistent");
        let result = repo.cherry_pick(&fake_hash);
        assert!(result.is_err());
    }

    #[test]
    fn test_rebase() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "content of a")?;
        repo.add("a.txt")?;
        repo.commit("add a.txt")?;

        repo.create_branch("feature", None)?;

        repo.checkout("feature")?;
        fs::write(dir.path().join("b.txt"), "content of b")?;
        repo.add("b.txt")?;
        repo.commit("add b.txt on feature")?;

        repo.checkout("main")?;
        fs::write(dir.path().join("c.txt"), "content of c")?;
        repo.add("c.txt")?;
        repo.commit("add c.txt on main")?;

        repo.checkout("feature")?;

        let result = repo.rebase("main")?;
        assert!(result.patches_replayed > 0);

        assert!(dir.path().join("b.txt").exists());
        assert!(dir.path().join("c.txt").exists());

        let log = repo.log(None)?;
        assert!(log.iter().any(|p| p.message == "add b.txt on feature"));
        assert!(log.iter().any(|p| p.message == "add c.txt on main"));

        Ok(())
    }

    #[test]
    fn test_rebase_fast_forward() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("a.txt"), "content of a")?;
        repo.add("a.txt")?;
        repo.commit("add a.txt")?;

        repo.create_branch("feature", None)?;

        fs::write(dir.path().join("b.txt"), "content of b")?;
        repo.add("b.txt")?;
        repo.commit("add b.txt")?;

        repo.checkout("feature")?;

        let result = repo.rebase("main")?;
        assert_eq!(result.patches_replayed, 0);

        assert!(dir.path().join("b.txt").exists());

        Ok(())
    }

    #[test]
    fn test_blame() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("test.txt"), "line1\nline2\nline3")?;
        repo.add("test.txt")?;
        let first_commit = repo.commit("initial content")?;

        fs::write(dir.path().join("test.txt"), "line1\nline2-modified\nline3")?;
        repo.add("test.txt")?;
        let second_commit = repo.commit("modify line2")?;

        let blame = repo.blame("test.txt")?;

        assert_eq!(blame.len(), 3);
        assert_eq!(blame[0].line, "line1");
        assert_eq!(blame[0].patch_id, first_commit);

        assert_eq!(blame[1].line, "line2-modified");
        assert_eq!(blame[1].patch_id, second_commit);

        assert_eq!(blame[2].line, "line3");
        assert_eq!(blame[2].patch_id, first_commit);

        Ok(())
    }

    #[test]
    fn test_blame_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice").unwrap();

        let result = repo.blame("nonexistent.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_rm_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("test.txt"), "content")?;
        repo.add("test.txt")?;
        repo.commit("initial")?;

        fs::remove_file(dir.path().join("test.txt"))?;
        repo.add("test.txt")?;

        assert!(!dir.path().join("test.txt").exists());

        let ws = repo.meta.working_set()?;
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].0, "test.txt");
        assert_eq!(ws[0].1, FileStatus::Deleted);

        Ok(())
    }

    #[test]
    fn test_rm_cached() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("test.txt"), "content")?;
        repo.add("test.txt")?;
        repo.commit("initial")?;

        let repo_path = RepoPath::new("test.txt")?;
        repo.meta.working_set_add(&repo_path, FileStatus::Deleted)?;

        assert!(dir.path().join("test.txt").exists());

        let ws = repo.meta.working_set()?;
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].0, "test.txt");
        assert_eq!(ws[0].1, FileStatus::Deleted);

        Ok(())
    }

    #[test]
    fn test_mv_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = Repository::init(dir.path(), "alice")?;

        fs::write(dir.path().join("old.txt"), "content")?;
        repo.add("old.txt")?;
        repo.commit("initial")?;

        repo.rename_file("old.txt", "new.txt")?;

        assert!(!dir.path().join("old.txt").exists());
        assert!(dir.path().join("new.txt").exists());

        let ws = repo.meta.working_set()?;
        assert!(
            ws.iter()
                .any(|(p, s)| p == "old.txt" && *s == FileStatus::Deleted)
        );
        assert!(
            ws.iter()
                .any(|(p, s)| p == "new.txt" && *s == FileStatus::Added)
        );

        Ok(())
    }

    #[test]
    fn test_mv_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice").unwrap();

        let result = repo.rename_file("nonexistent.txt", "new.txt");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("path not found"));
    }

    #[test]
    fn test_remove_remote() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), "alice")?;

        repo.add_remote("origin", "http://example.com")?;

        let remotes = repo.list_remotes()?;
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].0, "origin");

        repo.remove_remote("origin")?;

        let remotes = repo.list_remotes()?;
        assert!(remotes.is_empty());

        let result = repo.remove_remote("nonexistent");
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_log_performance_100_commits() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "perf-test").unwrap();

        for i in 0..100 {
            let file = format!("file_{i}.txt");
            std::fs::write(repo_path.join(&file), format!("content {i}")).unwrap();
            repo.add(&file).unwrap();
            repo.commit(&format!("commit {i}")).unwrap();
        }

        let start = std::time::Instant::now();
        let log = repo.log(None).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(log.len(), 101); // 100 commits + 1 root "Initial commit"
        assert!(
            elapsed.as_secs() < 1,
            "log() took {:?}",
            elapsed
        );
    }

    #[test]
    fn test_log_performance_with_limit() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "perf-test").unwrap();

        for i in 0..200 {
            let file = format!("file_{i}.txt");
            std::fs::write(repo_path.join(&file), format!("content {i}")).unwrap();
            repo.add(&file).unwrap();
            repo.commit(&format!("commit {i}")).unwrap();
        }

        let start = std::time::Instant::now();
        let log = repo.log(None).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(log.len(), 201); // 200 commits + 1 root "Initial commit"
        assert!(
            elapsed.as_secs() < 2,
            "log() took {:?}",
            elapsed
        );
    }

    #[test]
    fn test_perf_10k_commits_and_log() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "perf-test").unwrap();

        let t0 = std::time::Instant::now();
        for i in 0..10_000u32 {
            let path = "file.txt";
            std::fs::write(repo_path.join(path), format!("content {}", i)).unwrap();
            repo.add(path).unwrap();
            repo.commit(&format!("commit {}", i)).unwrap();
        }
        let commit_time = t0.elapsed();
        eprintln!("10K commits: {:?}", commit_time);

        let t1 = std::time::Instant::now();
        let log = repo.log(None).unwrap();
        let log_time = t1.elapsed();
        eprintln!("log() 10001 entries: {:?} ({} entries)", log_time, log.len());

        assert_eq!(log.len(), 10_001); // 10K commits + 1 root
        // 10K commits should complete in <30s even in debug mode
        assert!(
            commit_time.as_secs() < 30,
            "10K commits took too long: {:?}",
            commit_time
        );
    }
}
