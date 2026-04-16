// Suture Desktop Application — main entry point.
//
// Provides a native desktop GUI for Suture USVCS using Tauri v2.
// The frontend is a web-based UI served from the `ui/` directory,
// communicating with the Rust backend via Tauri commands.

#![cfg_attr(not(feature = "tauri"), allow(dead_code))]

use serde::{Deserialize, Serialize};

/// Information about a repository for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub path: String,
    pub head_branch: Option<String>,
    pub patch_count: usize,
    pub branch_count: usize,
    pub staged_count: usize,
    pub unstaged_count: usize,
}

/// A branch entry for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchEntry {
    pub name: String,
    pub target: String,
    pub is_current: bool,
}

/// A log entry for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub message: String,
    pub timestamp: String,
    pub is_merge: bool,
}

/// A file status entry for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

/// Result of a Tauri command.
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

// --- Tauri Commands ---
// These are the backend functions exposed to the frontend via IPC.

#[cfg(feature = "tauri")]
mod tauri_commands {
    use super::*;
    use std::sync::Mutex;
    use suture_core::repository::Repository;
    use tauri::State;

    /// Application state holding the open repository.
    pub struct AppState {
        pub repo: Mutex<Option<Repository>>,
    }

    /// Open a repository at the given path.
    #[tauri::command]
    pub fn open_repo(path: String, state: State<'_, AppState>) -> CmdResult<RepoInfo> {
        let repo = match Repository::open(std::path::Path::new(&path)) {
            Ok(r) => r,
            Err(e) => return CmdResult::err(format!("Failed to open repo: {e}")),
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
            Err(e) => return CmdResult::err(format!("Failed to get status: {e}")),
        };

        *state.repo.lock().unwrap() = Some(repo);
        CmdResult::ok(info)
    }

    /// Initialize a new repository at the given path.
    #[tauri::command]
    pub fn init_repo(path: String, state: State<'_, AppState>) -> CmdResult<RepoInfo> {
        let repo = match Repository::init(std::path::Path::new(&path)) {
            Ok(r) => r,
            Err(e) => return CmdResult::err(format!("Failed to init repo: {e}")),
        };

        let info = RepoInfo {
            path,
            head_branch: None,
            patch_count: 0,
            branch_count: 0,
            staged_count: 0,
            unstaged_count: 0,
        };

        *state.repo.lock().unwrap() = Some(repo);
        CmdResult::ok(info)
    }

    /// Get the list of branches.
    #[tauri::command]
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

    /// Create a new branch.
    #[tauri::command]
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
            Err(e) => CmdResult::err(format!("Failed to create branch: {e}")),
        }
    }

    /// Delete a branch.
    #[tauri::command]
    pub fn delete_branch(name: String, state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.delete_branch(&name) {
            Ok(()) => CmdResult::ok(format!("Deleted branch: {name}")),
            Err(e) => CmdResult::err(format!("Failed to delete branch: {e}")),
        }
    }

    /// Get the commit log.
    #[tauri::command]
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
            Err(e) => CmdResult::err(format!("Failed to get log: {e}")),
        }
    }

    /// Stage a file.
    #[tauri::command]
    pub fn stage_file(path: String, state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.add(&path) {
            Ok(()) => CmdResult::ok(format!("Staged: {path}")),
            Err(e) => CmdResult::err(format!("Failed to stage: {e}")),
        }
    }

    /// Stage all files.
    #[tauri::command]
    pub fn stage_all(state: State<'_, AppState>) -> CmdResult<String> {
        let guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_ref() else {
            return CmdResult::err("No repository open");
        };

        match repo.add_all() {
            Ok(count) => CmdResult::ok(format!("Staged {count} files")),
            Err(e) => CmdResult::err(format!("Failed to stage all: {e}")),
        }
    }

    /// Commit staged changes.
    #[tauri::command]
    pub fn commit(message: String, state: State<'_, AppState>) -> CmdResult<String> {
        let mut guard = state.repo.lock().unwrap();
        let Some(repo) = guard.as_mut() else {
            return CmdResult::err("No repository open");
        };

        match repo.commit(&message) {
            Ok(id) => CmdResult::ok(format!("Committed: {}…", &id.to_hex()[..12])),
            Err(e) => CmdResult::err(format!("Failed to commit: {e}")),
        }
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
    use tauri_commands::AppState;

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            repo: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            tauri_commands::open_repo,
            tauri_commands::init_repo,
            tauri_commands::list_branches,
            tauri_commands::create_branch,
            tauri_commands::delete_branch,
            tauri_commands::get_log,
            tauri_commands::stage_file,
            tauri_commands::stage_all,
            tauri_commands::commit,
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
