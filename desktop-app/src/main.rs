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

        let output = match run_suture(&["commit", "-m", &message], Some(&cwd)).await {
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
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&status_item)
                .separator()
                .item(&quit_item)
                .build()?;

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
                        "quit" => {
                            app_handle.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

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
