//! Node.js bindings for Suture via napi-rs.
//!
//! This crate exposes the Suture core library as a native Node.js addon,
//! enabling JavaScript/TypeScript applications to use Suture's
//! patch-based version control with semantic merge.

use std::path::Path;

use napi_derive::napi;

#[napi(object)]
pub struct LogEntry {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: f64,
}

#[napi(object)]
pub struct StatusResult {
    pub branch: String,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
}

/// Initialize a new Suture repository at the given path.
#[napi]
pub fn init_repo(repo_path: String, author: String) -> napi::Result<()> {
    suture_core::repository::Repository::init(Path::new(&repo_path), &author)
        .map_err(|e| napi::Error::from_reason(format!("Failed to init repo: {e}")))?;
    Ok(())
}

/// Get the current branch name.
#[napi]
pub fn get_current_branch(repo_path: String) -> napi::Result<String> {
    let repo = suture_core::repository::Repository::open(Path::new(&repo_path))
        .map_err(|e| napi::Error::from_reason(format!("Failed to open repo: {e}")))?;
    let (branch, _) = repo
        .head()
        .map_err(|e| napi::Error::from_reason(format!("Failed to get HEAD: {e}")))?;
    Ok(branch)
}

/// List all branches in the repository.
#[napi]
pub fn list_branches(repo_path: String) -> napi::Result<Vec<String>> {
    let repo = suture_core::repository::Repository::open(Path::new(&repo_path))
        .map_err(|e| napi::Error::from_reason(format!("Failed to open repo: {e}")))?;
    Ok(repo
        .list_branches()
        .into_iter()
        .map(|(name, _)| name)
        .collect())
}

/// Create a new branch.
#[napi]
pub fn create_branch(repo_path: String, name: String) -> napi::Result<()> {
    let mut repo = suture_core::repository::Repository::open(Path::new(&repo_path))
        .map_err(|e| napi::Error::from_reason(format!("Failed to open repo: {e}")))?;
    repo.create_branch(&name, None)
        .map_err(|e| napi::Error::from_reason(format!("Failed to create branch: {e}")))?;
    Ok(())
}

/// Add a file to the staging area.
#[napi]
pub fn add_file(repo_path: String, file_path: String) -> napi::Result<()> {
    let repo = suture_core::repository::Repository::open(Path::new(&repo_path))
        .map_err(|e| napi::Error::from_reason(format!("Failed to open repo: {e}")))?;
    repo.add(&file_path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to add file: {e}")))?;
    Ok(())
}

/// Commit staged changes.
#[napi]
pub fn commit(repo_path: String, message: String) -> napi::Result<String> {
    let mut repo = suture_core::repository::Repository::open(Path::new(&repo_path))
        .map_err(|e| napi::Error::from_reason(format!("Failed to open repo: {e}")))?;
    let hash = repo
        .commit(&message)
        .map_err(|e| napi::Error::from_reason(format!("Failed to commit: {e}")))?;
    Ok(hash.to_hex())
}

/// Get the commit log.
#[napi]
pub fn get_log(repo_path: String, limit: Option<u32>) -> napi::Result<Vec<LogEntry>> {
    let repo = suture_core::repository::Repository::open(Path::new(&repo_path))
        .map_err(|e| napi::Error::from_reason(format!("Failed to open repo: {e}")))?;
    let patches = repo
        .log(None)
        .map_err(|e| napi::Error::from_reason(format!("Failed to get log: {e}")))?;
    let limit = limit.unwrap_or(20) as usize;
    Ok(patches
        .into_iter()
        .take(limit)
        .map(|p| LogEntry {
            hash: p.id.to_hex(),
            message: p.message,
            author: p.author,
            timestamp: p.timestamp as f64,
        })
        .collect())
}

/// Get repository status.
#[napi]
pub fn get_status(repo_path: String) -> napi::Result<StatusResult> {
    let repo = suture_core::repository::Repository::open(Path::new(&repo_path))
        .map_err(|e| napi::Error::from_reason(format!("Failed to open repo: {e}")))?;
    let status = repo
        .status()
        .map_err(|e| napi::Error::from_reason(format!("Failed to get status: {e}")))?;
    Ok(StatusResult {
        branch: status.head_branch.unwrap_or_default(),
        staged: status.staged_files.iter().map(|(p, _)| p.clone()).collect(),
        unstaged: vec![],
        untracked: vec![],
    })
}

/// Get the Suture library version.
#[napi]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
