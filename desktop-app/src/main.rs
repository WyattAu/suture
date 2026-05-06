#![cfg_attr(not(feature = "tauri"), allow(dead_code))]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub path: String,
    pub head_branch: Option<String>,
    pub patch_count: usize,
    pub branch_count: usize,
    pub staged_count: usize,
    pub unstaged_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub head_branch: Option<String>,
    pub patch_count: usize,
    pub branch_count: usize,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub staged_files: Vec<FileEntry>,
    pub unstaged_files: Vec<FileEntry>,
    pub last_commit: Option<LogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchEntry {
    pub name: String,
    pub target: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub message: String,
    pub timestamp: String,
    pub is_merge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagEntry {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeDriverConfig {
    pub is_configured: bool,
    pub name: String,
    pub driver: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CmdResult<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> CmdResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

#[cfg(feature = "tauri")]
pub struct AppState {
    pub repo: std::sync::Mutex<Option<suture_core::repository::Repository>>,
    pub repo_path: std::sync::Mutex<Option<String>>,
}

// --- Library-based Tauri Commands ---
// These use suture-core directly for maximum performance.

fn which_suture() -> Option<String> {
    let output = std::process::Command::new("which")
        .arg("suture")
        .output()
        .ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}

fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    let max_len = va.len().max(vb.len());
    for i in 0..max_len {
        let ca = va.get(i).copied().unwrap_or(0);
        let cb = vb.get(i).copied().unwrap_or(0);
        match ca.cmp(&cb) {
            std::cmp::Ordering::Equal => continue,
            ord => return Some(ord),
        }
    }
    Some(std::cmp::Ordering::Equal)
}

#[cfg(feature = "tauri")]
mod tauri_commands {
    use super::*;
    use suture_core::repository::{Repository, ResetMode};
    use tauri::State;

    pub fn open_repo(path: String, state: State<'_, AppState>) -> CmdResult<RepoInfo> {
        let repo = match Repository::open(std::path::Path::new(&path)) {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("Failed to open repo: {e}");
                return CmdResult::err(msg);
            }
        };

        let info = match repo.status() {
            Ok(s) => RepoInfo {
                path,
                head_branch: s.head_branch,
                patch_count: s.patch_count,
                branch_count: s.branch_count,
                staged_count: s.staged_files.len(),
                unstaged_count: 0,
            },
            Err(e) => {
                let msg = format!("Failed to get status: {e}");
                return CmdResult::err(msg);
            }
        };

        *state.repo.lock().unwrap() = Some(repo);
        *state.repo_path.lock().unwrap() = Some(info.path.clone());
        CmdResult::ok(info)
    }

    pub fn init_repo(path: String, state: State<'_, AppState>) -> CmdResult<RepoInfo> {
        let repo = match Repository::init(std::path::Path::new(&path), "desktop") {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("Failed to init repo: {e}");
                return CmdResult::err(msg);
            }
        };

        let info = RepoInfo {
            path: path.clone(),
            head_branch: Some("main".to_string()),
            patch_count: 1,
            branch_count: 1,
            staged_count: 0,
            unstaged_count: 0,
        };

        *state.repo.lock().unwrap() = Some(repo);
        *state.repo_path.lock().unwrap() = Some(path);
        CmdResult::ok(info)
    }

    pub fn get_status(state: State<'_, AppState>) -> CmdResult<StatusResponse> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        let s = match repo.status() {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("Failed to get status: {e}");
                return CmdResult::err(msg);
            }
        };

        let last_commit = match repo.log(None) {
            Ok(patches) => patches.first().map(|p| LogEntry {
                id: p.id.to_hex(),
                short_id: format!("{:.12}…", p.id),
                author: p.author.clone(),
                message: p.message.clone(),
                timestamp: format_timestamp(p.timestamp),
                is_merge: p.parent_ids.len() > 1,
            }),
            Err(_) => None,
        };

        let staged_files: Vec<FileEntry> = s
            .staged_files
            .iter()
            .map(|(path, status)| FileEntry {
                path: path.clone(),
                status: format!("{:?}", status),
                staged: true,
            })
            .collect();

        CmdResult::ok(StatusResponse {
            head_branch: s.head_branch,
            patch_count: s.patch_count,
            branch_count: s.branch_count,
            staged_count: s.staged_files.len(),
            unstaged_count: 0,
            staged_files,
            unstaged_files: vec![],
            last_commit,
        })
    }

    pub fn list_branches(state: State<'_, AppState>) -> CmdResult<Vec<BranchEntry>> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        let head_branch = repo.status().ok().and_then(|s| s.head_branch);
        let branches = repo
            .dag()
            .list_branches()
            .into_iter()
            .map(|(name, id)| BranchEntry {
                name,
                target: id.to_hex(),
                is_current: head_branch.as_deref() == Some(&name),
            })
            .collect();

        CmdResult::ok(branches)
    }

    pub fn create_branch(
        name: String,
        target: Option<String>,
        state: State<'_, AppState>,
    ) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.create_branch(&name, target.as_deref()) {
            Ok(()) => CmdResult::ok(format!("Created branch: {name}")),
            Err(e) => {
                let msg = format!("Failed to create branch: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn delete_branch(name: String, state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.delete_branch(&name) {
            Ok(()) => CmdResult::ok(format!("Deleted branch: {name}")),
            Err(e) => {
                let msg = format!("Failed to delete branch: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn get_log(state: State<'_, AppState>) -> CmdResult<Vec<LogEntry>> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.log(None) {
            Ok(patches) => {
                let entries = patches
                    .iter()
                    .map(|p| LogEntry {
                        id: p.id.to_hex(),
                        short_id: format!("{}…", &p.id.to_hex()[..12.min(p.id.to_hex().len())]),
                        author: p.author.clone(),
                        message: p.message.clone(),
                        timestamp: format_timestamp(p.timestamp),
                        is_merge: p.parent_ids.len() > 1,
                    })
                    .collect();
                CmdResult::ok(entries)
            }
            Err(e) => {
                let msg = format!("Failed to get log: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn stage_file(path: String, state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.add(&path) {
            Ok(()) => CmdResult::ok(format!("Staged: {path}")),
            Err(e) => {
                let msg = format!("Failed to stage: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn stage_all(state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.add_all() {
            Ok(count) => CmdResult::ok(format!("Staged {count} files")),
            Err(e) => {
                let msg = format!("Failed to stage all: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn commit(message: String, state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.commit(&message) {
            Ok(id) => CmdResult::ok(format!("Committed: {}…", &id.to_hex()[..12])),
            Err(e) => {
                let msg = format!("Failed to commit: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn checkout_branch(name: String, state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.checkout(&name) {
            Ok(_) => CmdResult::ok(format!("Checked out: {name}")),
            Err(e) => {
                let msg = format!("Failed to checkout: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn unstage_all(state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.reset("HEAD", ResetMode::Mixed) {
            Ok(_) => CmdResult::ok("Unstaged all files".to_string()),
            Err(e) => {
                let msg = format!("Failed to unstage: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub async fn suture_command(args: Vec<String>) -> Result<String, String> {
        let output = tokio::process::Command::new("suture")
            .args(&args)
            .output()
            .await
            .map_err(|e| {
                let msg = format!("Failed to execute suture: {e}");
                msg
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub fn merge_branch(name: String, state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.execute_merge(&name) {
            Ok(result) => {
                if result.is_clean {
                    let id = result
                        .merge_patch_id
                        .map(|id| format!(" {}…", &id.to_hex()[..12]))
                        .unwrap_or_default();
                    CmdResult::ok(format!("Merged '{name}':{id}"))
                } else {
                    let count = result.unresolved_conflicts.len();
                    let msg = format!("Merge conflicts in {count} file(s)");
                    CmdResult::err(msg)
                }
            }
            Err(e) => {
                let msg = format!("Failed to merge: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn get_diff(state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.diff(None, None) {
            Ok(entries) => {
                let text = entries
                    .iter()
                    .map(|e| format!("{e}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                CmdResult::ok(if text.is_empty() {
                    "(no changes)".to_string()
                } else {
                    text
                })
            }
            Err(e) => {
                let msg = format!("Failed to get diff: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn get_tags(state: State<'_, AppState>) -> CmdResult<Vec<TagEntry>> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.list_tags() {
            Ok(tags) => {
                let entries = tags
                    .iter()
                    .map(|(name, id)| TagEntry {
                        name: name.clone(),
                        target: id.to_hex(),
                    })
                    .collect();
                CmdResult::ok(entries)
            }
            Err(e) => {
                let msg = format!("Failed to list tags: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn create_tag(name: String, state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.create_tag(&name, None) {
            Ok(()) => CmdResult::ok(format!("Created tag: {name}")),
            Err(e) => {
                let msg = format!("Failed to create tag: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub async fn push(state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo_path.lock().unwrap();
        let Some(ref cwd) = *guard else {
            return CmdResult::err("No repository open");
        };
        let cwd = cwd.clone();
        drop(guard);

        let output = tokio::process::Command::new("suture")
            .args(&["push"])
            .current_dir(&cwd)
            .output()
            .await
            .map_err(|e| {
                let msg = format!("Failed to execute suture push: {e}");
                msg
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
                .map(CmdResult::ok)
                .unwrap_or_else(CmdResult::err)
        } else {
            CmdResult::err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub async fn pull(state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo_path.lock().unwrap();
        let Some(ref cwd) = *guard else {
            return CmdResult::err("No repository open");
        };
        let cwd = cwd.clone();
        drop(guard);

        let output = tokio::process::Command::new("suture")
            .args(&["pull"])
            .current_dir(&cwd)
            .output()
            .await
            .map_err(|e| {
                let msg = format!("Failed to execute suture pull: {e}");
                msg
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
                .map(CmdResult::ok)
                .unwrap_or_else(CmdResult::err)
        } else {
            CmdResult::err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub async fn sync(state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo_path.lock().unwrap();
        let Some(ref cwd) = *guard else {
            return CmdResult::err("No repository open");
        };
        let cwd = cwd.clone();
        drop(guard);

        let output = tokio::process::Command::new("suture")
            .args(&["sync"])
            .current_dir(&cwd)
            .output()
            .await
            .map_err(|e| {
                let msg = format!("Failed to execute suture sync: {e}");
                msg
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
                .map(CmdResult::ok)
                .unwrap_or_else(CmdResult::err)
        } else {
            CmdResult::err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub fn get_remotes(state: State<'_, AppState>) -> CmdResult<Vec<RemoteEntry>> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.list_remotes() {
            Ok(remotes) => {
                let entries = remotes
                    .iter()
                    .map(|(name, url)| RemoteEntry {
                        name: name.clone(),
                        url: url.clone(),
                    })
                    .collect();
                CmdResult::ok(entries)
            }
            Err(e) => {
                let msg = format!("Failed to list remotes: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn add_remote(
        name: String,
        url: String,
        state: State<'_, AppState>,
    ) -> CmdResult<String> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.add_remote(&name, &url) {
            Ok(()) => CmdResult::ok(format!("Added remote '{name}': {url}")),
            Err(e) => {
                let msg = format!("Failed to add remote: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn get_stash_list(state: State<'_, AppState>) -> CmdResult<Vec<StashEntry>> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.stash_list() {
            Ok(entries) => {
                let mapped = entries
                    .iter()
                    .map(|e| StashEntry {
                        index: e.index,
                        message: e.message.clone(),
                        timestamp: e.head_id.clone(),
                    })
                    .collect();
                CmdResult::ok(mapped)
            }
            Err(e) => {
                let msg = format!("Failed to list stash: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn stash_push(
        message: Option<String>,
        state: State<'_, AppState>,
    ) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.stash_push(message.as_deref()) {
            Ok(index) => {
                let msg = format!("Stashed as stash@{{{index}}}");
                CmdResult::ok(msg)
            }
            Err(e) => {
                let msg = format!("Failed to stash: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub async fn stash_pop(state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.stash_pop() {
            Ok(()) => CmdResult::ok("Stash popped".to_string()),
            Err(e) => {
                let msg = format!("Failed to pop stash: {e}");
                CmdResult::err(msg)
            }
        }
    }

    pub fn get_merge_driver_config() -> CmdResult<MergeDriverConfig> {
        let current = std::process::Command::new("git")
            .args(["config", "--global", "merge.suture-driver.name"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            });

        let driver_cmd = std::process::Command::new("git")
            .args(["config", "--global", "merge.suture-driver.driver"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            });

        CmdResult::ok(MergeDriverConfig {
            is_configured: current.is_some() && driver_cmd.is_some(),
            name: current.unwrap_or_default(),
            driver: driver_cmd.unwrap_or_default(),
        })
    }

    pub fn set_merge_driver_config(state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo_path.lock().unwrap();
        let _ = guard.as_ref().ok_or_else(|| "No repository open".to_string());

        let suture_path = which_suture();
        if suture_path.is_none() {
            return CmdResult::err("suture binary not found in PATH".to_string());
        }

        let name_result = std::process::Command::new("git")
            .args([
                "config",
                "--global",
                "merge.suture-driver.name",
                "Suture merge driver",
            ])
            .output();

        let driver_result = std::process::Command::new("git")
            .args([
                "config",
                "--global",
                "merge.suture-driver.driver",
                "suture merge driver %O %A %B",
            ])
            .output();

        match (name_result, driver_result) {
            (Ok(n), Ok(d)) if n.status.success() && d.status.success() => {
                CmdResult::ok("Merge driver configured globally".to_string())
            }
            (Ok(_), Ok(d)) if !d.status.success() => {
                let stderr = String::from_utf8_lossy(&d.stderr);
                CmdResult::err(format!("Failed to set driver: {stderr}"))
            }
            (Ok(n), Ok(_)) if !n.status.success() => {
                CmdResult::err("Failed to set merge driver name".to_string())
            }
            (Err(e), _) => CmdResult::err(format!("Failed to run git config: {e}")),
            _ => CmdResult::err("Unknown error configuring merge driver".to_string()),
        }
    }

    pub fn unset_merge_driver_config() -> CmdResult<String> {
        let _ = std::process::Command::new("git")
            .args(["config", "--global", "--unset", "merge.suture-driver.name"])
            .output();

        let result = std::process::Command::new("git")
            .args(["config", "--global", "--unset", "merge.suture-driver.driver"])
            .output();

        match result {
            Ok(o) if o.status.success() || String::from_utf8_lossy(&o.stderr).contains("not found") => {
                CmdResult::ok("Merge driver removed".to_string())
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                CmdResult::err(format!("Failed to unset: {stderr}"))
            }
            Err(e) => CmdResult::err(format!("Failed to run git config: {e}")),
        }
    }

    pub async fn check_for_updates() -> CmdResult<UpdateInfo> {
        let current_version = env!("CARGO_PKG_VERSION").to_string();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => return CmdResult::ok(UpdateInfo {
                current_version: current_version.clone(),
                latest_version: current_version,
                update_available: false,
                error: Some(format!("Failed to create HTTP client: {e}")),
            }),
        };

        let url = "https://api.github.com/repos/WyattAu/suture/releases/latest";
        let resp = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => return CmdResult::ok(UpdateInfo {
                current_version: current_version.clone(),
                latest_version: current_version.clone(),
                update_available: false,
                error: Some(format!("Network error: {e}")),
            }),
        };

        if !resp.status().is_success() {
            return CmdResult::ok(UpdateInfo {
                current_version: current_version.clone(),
                latest_version: current_version.clone(),
                update_available: false,
                error: Some(format!("GitHub API returned status {}", resp.status())),
            });
        }

        match resp.json::<serde_json::Value>().await {
            Ok(release) => {
                let latest = release
                    .get("tag_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .trim_start_matches('v')
                    .to_string();

                let update_available = compare_versions(&latest, &current_version)
                    .map(|ord| ord == std::cmp::Ordering::Greater)
                    .unwrap_or(false);

                CmdResult::ok(UpdateInfo {
                    current_version: current_version.clone(),
                    latest_version: latest,
                    update_available,
                    error: None,
                })
            }
            Err(e) => CmdResult::ok(UpdateInfo {
                current_version: current_version.clone(),
                latest_version: current_version.clone(),
                update_available: false,
                error: Some(format!("Failed to parse response: {e}")),
            }),
        }
    }
}

// --- CLI-based Tauri Commands ---
// These shell out to the `suture` binary for operations that don't need
// the in-process library.  Useful when the desktop app is distributed
// independently of the suture-core crate.

#[cfg(feature = "tauri")]
mod cli_commands {
    use super::*;
    use tauri::State;

    use crate::AppState;

    async fn run_suture(
        args: &[&str],
        cwd: Option<&str>,
    ) -> Result<std::process::Output, String> {
        let mut cmd = tokio::process::Command::new("suture");
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let output = cmd.output().await.map_err(|e| {
            let msg = format!("Failed to execute suture: {e}");
            msg
        })?;
        Ok(output)
    }

    fn repo_cwd(state: &State<'_, AppState>) -> Result<String, String> {
        let guard = state.repo_path.lock().unwrap();
        match guard.as_ref() {
            Some(p) => Ok(p.clone()),
            None => Err("No repository open".to_string()),
        }
    }

    #[tauri::command]
    pub async fn cli_init_repo(path: String, state: State<'_, AppState>) -> CmdResult<RepoInfo> {
        let output = match run_suture(&["init", &path], None).await {
            Ok(o) => o,
            Err(e) => return CmdResult::err(e),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return CmdResult::err(stderr.to_string());
        }

        *state.repo_path.lock().unwrap() = Some(path.clone());

        let info = RepoInfo {
            path,
            head_branch: Some("main".to_string()),
            patch_count: 1,
            branch_count: 1,
            staged_count: 0,
            unstaged_count: 0,
        };
        CmdResult::ok(info)
    }

    #[tauri::command]
    pub async fn cli_get_status(state: State<'_, AppState>) -> CmdResult<StatusResponse> {
        let cwd = match repo_cwd(&state) {
            Ok(c) => c,
            Err(e) => return CmdResult::err(e),
        };

        let output = match run_suture(&["status"], Some(&cwd)).await {
            Ok(o) => o,
            Err(e) => return CmdResult::err(e),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return CmdResult::err(stderr.to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut head_branch = None;
        let mut patch_count = 0usize;
        let mut branch_count = 0usize;
        let mut staged_files: Vec<FileEntry> = Vec::new();
        let mut unstaged_files: Vec<FileEntry> = Vec::new();
        let mut section = "";

        for line in stdout.lines() {
            if line.starts_with("On branch ") {
                head_branch = Some(line["On branch ".len()..].to_string());
            } else if line.contains(" patches, ") && line.contains(" branches") {
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if let Some(counts) = parts.last() {
                    let nums: Vec<usize> = counts
                        .split(|c: char| !c.is_ascii_digit())
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if nums.len() >= 2 {
                        patch_count = nums[0];
                        branch_count = nums[1];
                    }
                }
            } else if line.starts_with("\nStaged changes:") || line.starts_with("Staged changes:") {
                section = "staged";
            } else if line.starts_with("\nUnstaged changes:") || line.starts_with("Unstaged changes:")
            {
                section = "unstaged";
            } else if !line.is_empty() {
                let trimmed = line.trim_start();
                if section == "staged" && trimmed.starts_with("Added ") {
                    staged_files.push(FileEntry {
                        path: trimmed["Added ".len()..].to_string(),
                        status: "Added".to_string(),
                        staged: true,
                    });
                } else if section == "staged" && trimmed.starts_with("Modified ") {
                    staged_files.push(FileEntry {
                        path: trimmed["Modified ".len()..].to_string(),
                        status: "Modified".to_string(),
                        staged: true,
                    });
                } else if section == "staged" && trimmed.starts_with("Deleted ") {
                    staged_files.push(FileEntry {
                        path: trimmed["Deleted ".len()..].to_string(),
                        status: "Deleted".to_string(),
                        staged: true,
                    });
                } else if section == "unstaged" && trimmed.starts_with("modified: ") {
                    unstaged_files.push(FileEntry {
                        path: trimmed["modified: ".len()..].to_string(),
                        status: "Modified".to_string(),
                        staged: false,
                    });
                } else if section == "unstaged" && trimmed.starts_with("deleted: ") {
                    unstaged_files.push(FileEntry {
                        path: trimmed["deleted: ".len()..].to_string(),
                        status: "Deleted".to_string(),
                        staged: false,
                    });
                } else if section == "unstaged" && trimmed.starts_with("untracked: ") {
                    unstaged_files.push(FileEntry {
                        path: trimmed["untracked: ".len()..].to_string(),
                        status: "Untracked".to_string(),
                        staged: false,
                    });
                }
            }
        }

        CmdResult::ok(StatusResponse {
            head_branch,
            patch_count,
            branch_count,
            staged_count: staged_files.len(),
            unstaged_count: unstaged_files.len(),
            staged_files,
            unstaged_files,
            last_commit: None,
        })
    }

    #[tauri::command]
    pub async fn cli_create_branch(
        name: String,
        state: State<'_, AppState>,
    ) -> CmdResult<String> {
        let cwd = match repo_cwd(&state) {
            Ok(c) => c,
            Err(e) => return CmdResult::err(e),
        };

        let output = match run_suture(&["branch", &name], Some(&cwd)).await {
            Ok(o) => o,
            Err(e) => return CmdResult::err(e),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return CmdResult::err(stderr.to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        CmdResult::ok(stdout.trim().to_string())
    }

    #[tauri::command]
    pub async fn cli_get_log(state: State<'_, AppState>) -> CmdResult<Vec<LogEntry>> {
        let cwd = match repo_cwd(&state) {
            Ok(c) => c,
            Err(e) => return CmdResult::err(e),
        };

        let output = match run_suture(&["log", "--oneline"], Some(&cwd)).await {
            Ok(o) => o,
            Err(e) => return CmdResult::err(e),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return CmdResult::err(stderr.to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<LogEntry> = stdout
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with("No commits"))
            .map(|line| {
                let mut parts = line.splitn(2, ' ');
                let hash = parts.next().unwrap_or("").to_string();
                let message = parts.next().unwrap_or("").to_string();
                LogEntry {
                    id: hash.clone(),
                    short_id: format!("{hash}…"),
                    author: String::new(),
                    message,
                    timestamp: String::new(),
                    is_merge: false,
                }
            })
            .collect();

        CmdResult::ok(entries)
    }

    #[tauri::command]
    pub async fn cli_commit_changes(
        message: String,
        state: State<'_, AppState>,
    ) -> CmdResult<String> {
        let cwd = match repo_cwd(&state) {
            Ok(c) => c,
            Err(e) => return CmdResult::err(e),
        };

        let output = match run_suture(&["commit", &message], Some(&cwd)).await {
            Ok(o) => o,
            Err(e) => return CmdResult::err(e),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return CmdResult::err(stderr.to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        CmdResult::ok(stdout.trim().to_string())
    }
}

fn format_timestamp(ts: u64) -> String {
    let secs = ts;
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let (year, month, day) = days_to_date(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

fn days_to_date(mut days: i64) -> (i64, i64, i64) {
    days += 719468;
    let era = if days >= 0 {
        days / 146097
    } else {
        (days - 146096) / 146097
    };
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(feature = "tauri")]
fn main() {
    use cli_commands as cli;
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;
    use tauri_commands as lib;
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            repo: std::sync::Mutex::new(None),
            repo_path: std::sync::Mutex::new(None),
        })
        .setup(|app| {
            let show_item = MenuItemBuilder::with_id("show", "Show Suture").build(app)?;
            let status_item = MenuItemBuilder::with_id("status", "Refresh Status").build(app)?;
            let sync_item = MenuItemBuilder::with_id("sync", "Sync Now").build(app)?;
            let auto_sync_item = MenuItemBuilder::with_id("toggle-auto-sync", "Toggle Auto-Sync (OFF)").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&status_item)
                .item(&sync_item)
                .item(&auto_sync_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let auto_sync_enabled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let auto_sync_for_thread = auto_sync_enabled.clone();

            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Suture — No repository open")
                .on_menu_event(move |app_handle, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "status" => {
                            let _ = app_handle.emit("tray-refresh-status", ());
                        }
                        "sync" => {
                            let _ = app_handle.emit("auto-sync", ());
                        }
                        "toggle-auto-sync" => {
                            let was = auto_sync_enabled.load(std::sync::atomic::Ordering::Relaxed);
                            auto_sync_enabled.store(!was, std::sync::atomic::Ordering::Relaxed);
                        }
                        "quit" => {
                            app_handle.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(300));
                    if auto_sync_for_thread.load(std::sync::atomic::Ordering::Relaxed) {
                        let _ = app_handle.emit("auto-sync", ());
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            lib::open_repo,
            lib::init_repo,
            lib::get_status,
            lib::list_branches,
            lib::create_branch,
            lib::delete_branch,
            lib::get_log,
            lib::stage_file,
            lib::stage_all,
            lib::commit,
            lib::checkout_branch,
            lib::unstage_all,
            lib::suture_command,
            lib::merge_branch,
            lib::get_diff,
            lib::get_tags,
            lib::create_tag,
            lib::push,
            lib::pull,
            lib::sync,
            lib::get_remotes,
            lib::add_remote,
            lib::get_stash_list,
            lib::stash_push,
            lib::stash_pop,
            lib::get_merge_driver_config,
            lib::set_merge_driver_config,
            lib::unset_merge_driver_config,
            lib::check_for_updates,
            cli::cli_init_repo,
            cli::cli_get_status,
            cli::cli_create_branch,
            cli::cli_get_log,
            cli::cli_commit_changes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running suture-desktop");
}

#[cfg(not(feature = "tauri"))]
fn main() {
    eprintln!("suture-desktop requires the 'tauri' feature.");
    eprintln!("Install system dependencies and rebuild with: cargo build --features tauri");
    eprintln!();
    eprintln!("Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \\");
    eprintln!("                libayatana-appindicator3-dev librsvg2-dev");
    eprintln!("Fedora:        sudo dnf install webkit2gtk4.1-devel gtk3-devel \\");
    eprintln!("                libappindicator-gtk3-devel librsvg2-devel");
    eprintln!("Nix:           add webkitgtk_4_1 gtk3 libappindicator-gtk3 librsvg");
    std::process::exit(1);
}
