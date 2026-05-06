// SPDX-License-Identifier: MIT OR Apache-2.0
//! Node.js bindings for Suture via napi-rs.
//!
//! This crate exposes the Suture core library as a native Node.js addon,
//! enabling JavaScript/TypeScript applications to use Suture's
//! patch-based version control with semantic merge.

use std::path::Path;

use napi_derive::napi;
use suture_common::FileStatus;
use suture_driver::SutureDriver;

#[napi(object)]
pub struct RepoInfo {
    pub path: String,
    pub head_branch: Option<String>,
    pub patch_count: i64,
    pub branch_count: i64,
}

#[napi(object)]
pub struct CommitResult {
    pub id: String,
    pub short_id: String,
}

#[napi(object)]
pub struct StatusResult {
    pub head_branch: Option<String>,
    pub patch_count: i64,
    pub branch_count: i64,
    pub staged_files: Vec<FileEntry>,
}

#[napi(object)]
pub struct FileEntry {
    pub path: String,
    pub status: String,
}

#[napi(object)]
pub struct LogEntry {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub message: String,
    pub timestamp: f64,
    pub is_merge: bool,
}

#[napi(object)]
pub struct BranchEntry {
    pub name: String,
    pub target: String,
    pub is_current: bool,
}

#[napi]
pub fn init(path: String, author: String) -> napi::Result<RepoInfo> {
    let repo = suture_core::repository::Repository::init(Path::new(&path), &author)
        .map_err(|e| napi::Error::from_reason(format!("init failed: {e}")))?;
    let status = repo
        .status()
        .map_err(|e| napi::Error::from_reason(format!("status failed: {e}")))?;
    Ok(RepoInfo {
        path,
        head_branch: status.head_branch,
        patch_count: status.patch_count as i64,
        branch_count: status.branch_count as i64,
    })
}

#[napi]
pub fn open(path: String) -> napi::Result<RepoInfo> {
    let repo = suture_core::repository::Repository::open(Path::new(&path))
        .map_err(|e| napi::Error::from_reason(format!("open failed: {e}")))?;
    let status = repo
        .status()
        .map_err(|e| napi::Error::from_reason(format!("status failed: {e}")))?;
    Ok(RepoInfo {
        path,
        head_branch: status.head_branch,
        patch_count: status.patch_count as i64,
        branch_count: status.branch_count as i64,
    })
}

#[napi]
pub fn status(path: String) -> napi::Result<StatusResult> {
    let repo = suture_core::repository::Repository::open(Path::new(&path))
        .map_err(|e| napi::Error::from_reason(format!("open failed: {e}")))?;
    let s = repo
        .status()
        .map_err(|e| napi::Error::from_reason(format!("status failed: {e}")))?;
    let staged: Vec<FileEntry> = s
        .staged_files
        .iter()
        .map(|(p, st)| FileEntry {
            path: p.clone(),
            status: match st {
                FileStatus::Added => "added".to_owned(),
                FileStatus::Modified => "modified".to_owned(),
                FileStatus::Deleted => "deleted".to_owned(),
                FileStatus::Clean => "clean".to_owned(),
                FileStatus::Untracked => "untracked".to_owned(),
            },
        })
        .collect();
    Ok(StatusResult {
        head_branch: s.head_branch,
        patch_count: s.patch_count as i64,
        branch_count: s.branch_count as i64,
        staged_files: staged,
    })
}

#[napi]
pub fn add(path: String, file: String) -> napi::Result<()> {
    let repo = suture_core::repository::Repository::open(Path::new(&path))
        .map_err(|e| napi::Error::from_reason(format!("open failed: {e}")))?;
    repo.add(&file)
        .map_err(|e| napi::Error::from_reason(format!("add failed: {e}")))?;
    Ok(())
}

#[napi]
pub fn add_all(path: String) -> napi::Result<i32> {
    let repo = suture_core::repository::Repository::open(Path::new(&path))
        .map_err(|e| napi::Error::from_reason(format!("open failed: {e}")))?;
    let count = repo
        .add_all()
        .map_err(|e| napi::Error::from_reason(format!("add_all failed: {e}")))?;
    Ok(count as i32)
}

#[napi]
pub fn commit(path: String, message: String) -> napi::Result<CommitResult> {
    let mut repo = suture_core::repository::Repository::open(Path::new(&path))
        .map_err(|e| napi::Error::from_reason(format!("open failed: {e}")))?;
    let id = repo
        .commit(&message)
        .map_err(|e| napi::Error::from_reason(format!("commit failed: {e}")))?;
    let hex = id.to_hex();
    Ok(CommitResult {
        short_id: hex[..12].to_string(),
        id: hex,
    })
}

#[napi]
pub fn log(path: String, limit: Option<i32>) -> napi::Result<Vec<LogEntry>> {
    let repo = suture_core::repository::Repository::open(Path::new(&path))
        .map_err(|e| napi::Error::from_reason(format!("open failed: {e}")))?;
    let patches = repo
        .log(None)
        .map_err(|e| napi::Error::from_reason(format!("log failed: {e}")))?;
    let lim = limit.unwrap_or(50) as usize;
    Ok(patches
        .into_iter()
        .take(lim)
        .map(|p| {
            let hex = p.id.to_hex();
            LogEntry {
                id: hex.clone(),
                short_id: hex[..12.min(hex.len())].to_string(),
                author: p.author,
                message: p.message,
                timestamp: p.timestamp as f64,
                is_merge: p.parent_ids.len() > 1,
            }
        })
        .collect())
}

#[napi]
pub fn branches(path: String) -> napi::Result<Vec<BranchEntry>> {
    let repo = suture_core::repository::Repository::open(Path::new(&path))
        .map_err(|e| napi::Error::from_reason(format!("open failed: {e}")))?;
    let head = repo.status().ok().and_then(|s| s.head_branch);
    Ok(repo
        .dag()
        .list_branches()
        .into_iter()
        .map(|(name, id)| {
            let is_current = head.as_deref() == Some(&name);
            BranchEntry {
                name,
                target: id.to_hex(),
                is_current,
            }
        })
        .collect())
}

#[napi]
pub fn create_branch(path: String, name: String) -> napi::Result<()> {
    let mut repo = suture_core::repository::Repository::open(Path::new(&path))
        .map_err(|e| napi::Error::from_reason(format!("open failed: {e}")))?;
    repo.create_branch(&name, None)
        .map_err(|e| napi::Error::from_reason(format!("create_branch failed: {e}")))?;
    Ok(())
}

#[napi]
pub fn merge_json(base: String, ours: String, theirs: String) -> napi::Result<String> {
    let driver = suture_driver_json::JsonDriver::new();
    let result = driver
        .merge(&base, &ours, &theirs)
        .map_err(|e| napi::Error::from_reason(format!("merge failed: {e}")))?;
    result.ok_or_else(|| napi::Error::from_reason("merge conflict: cannot auto-resolve"))
}

#[napi]
pub fn merge_yaml(base: String, ours: String, theirs: String) -> napi::Result<String> {
    let driver = suture_driver_yaml::YamlDriver::new();
    let result = driver
        .merge(&base, &ours, &theirs)
        .map_err(|e| napi::Error::from_reason(format!("merge failed: {e}")))?;
    result.ok_or_else(|| napi::Error::from_reason("merge conflict: cannot auto-resolve"))
}

#[napi]
pub fn merge_toml(base: String, ours: String, theirs: String) -> napi::Result<String> {
    let driver = suture_driver_toml::TomlDriver::new();
    let result = driver
        .merge(&base, &ours, &theirs)
        .map_err(|e| napi::Error::from_reason(format!("merge failed: {e}")))?;
    result.ok_or_else(|| napi::Error::from_reason("merge conflict: cannot auto-resolve"))
}

#[napi]
pub fn merge_csv(base: String, ours: String, theirs: String) -> napi::Result<String> {
    let driver = suture_driver_csv::CsvDriver::new();
    let result = driver
        .merge(&base, &ours, &theirs)
        .map_err(|e| napi::Error::from_reason(format!("merge failed: {e}")))?;
    result.ok_or_else(|| napi::Error::from_reason("merge conflict: cannot auto-resolve"))
}

#[napi]
#[must_use]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
