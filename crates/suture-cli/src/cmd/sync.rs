use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::remote_proto::do_pull;

const SYNC_PID_FILE: &str = ".suture/sync.pid";
const SYNC_LAST_SYNC_FILE: &str = ".suture/sync.last_sync";
const DEBOUNCE_SECS: u64 = 2;
const POLL_INTERVAL_SECS: u64 = 1;

pub(crate) async fn cmd_sync(
    remote: &str,
    no_push: bool,
    pull_only: bool,
    message: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(Path::new("."))?;
    let has_remote = has_configured_remote(&repo, remote);

    let mut pulled = false;
    let mut pull_count = 0;

    if has_remote {
        eprintln!("Pulling from {}...", remote);
        match do_pull(&mut repo, remote).await {
            Ok(count) => {
                if count > 0 {
                    pulled = true;
                    pull_count = count;
                }
            }
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                if msg.contains("conflict") || msg.contains("merge") {
                    return Err(format!("merge conflict during pull: {e}").into());
                }
                return Err(format!("pull failed: {e}").into());
            }
        }
    }

    if pull_only {
        if pull_count > 0 {
            println!("Pulled {pull_count} patches from {remote}");
        } else {
            println!("Already up to date.");
        }
        return Ok(());
    }

    let changed_files = detect_changed_files(&repo)?;
    if changed_files.is_empty() && !pulled {
        println!("Everything up to date.");
        return Ok(());
    }

    let committed_files = if !changed_files.is_empty() {
        let count = repo.add_all()?;
        if count == 0 {
            println!("Everything up to date.");
            return Ok(());
        }

        let msg = match message {
            Some(m) => m.to_string(),
            None => generate_sync_message(&changed_files),
        };

        let patch_id = repo.commit(&msg)?;
        println!(
            "Committed {} change{}:",
            changed_files.len(),
            if changed_files.len() == 1 { "" } else { "s" }
        );
        for path in &changed_files {
            let icon = file_type_icon(path);
            let filename = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            println!("  {icon} {filename} (modified)");
        }
        println!("  ({})", &patch_id.to_hex()[..12]);

        Some(changed_files)
    } else {
        None
    };

    if has_remote && !no_push {
        let (branch, _) = repo.head().unwrap_or(("main".to_string(), suture_common::Hash::ZERO));
        match cmd_push_inner(&mut repo, remote).await {
            Ok(()) => {
                println!("Pushed to {remote}/{branch}");
            }
            Err(e) => {
                eprintln!("Push failed: {e}");
                eprintln!("Changes are committed locally.");
            }
        }
    }

    if pulled {
        println!("Pulled {pull_count} patches from {remote}");
    }

    if let Some(files) = committed_files {
        if !has_remote {
            println!("\nNo remote configured. Changes committed locally only.");
            println!("Run `suture remote add <name> <url>` to enable push/pull.");
        }
        let _ = files;
    }

    Ok(())
}

fn has_configured_remote(
    repo: &suture_core::repository::Repository,
    name: &str,
) -> bool {
    let remotes = repo.list_remotes().unwrap_or_default();
    remotes.iter().any(|(n, _)| n == name)
}

fn detect_changed_files(
    repo: &suture_core::repository::Repository,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut changed = Vec::new();

    let head_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());

    let repo_dir = Path::new(".");
    let disk_files = crate::display::walk_repo_files(repo_dir);

    for rel_path in &disk_files {
        let full_path = repo_dir.join(rel_path);
        if let Ok(data) = std::fs::read(&full_path) {
            let current_hash = suture_common::Hash::from_data(&data);
            if let Some(head_hash) = head_tree.get(rel_path) {
                if &current_hash != head_hash {
                    changed.push(rel_path.clone());
                }
            } else {
                changed.push(rel_path.clone());
            }
        }
    }

    for (path, _) in head_tree.iter() {
        if !disk_files.iter().any(|f| f == path) {
            changed.push(path.clone());
        }
    }

    Ok(changed)
}

fn generate_sync_message(changed_files: &[String]) -> String {
    let file_count = changed_files.len();
    if file_count == 0 {
        return "Sync: no changes".to_string();
    }

    if file_count == 1 {
        let filename = Path::new(&changed_files[0])
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&changed_files[0]);
        return format!("Sync: update {filename}");
    }

    let doc_count = changed_files
        .iter()
        .filter(|f| {
            let ext = Path::new(f)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            matches!(ext.as_str(), "docx" | "pdf" | "md" | "html" | "htm")
        })
        .count();
    let xls_count = changed_files
        .iter()
        .filter(|f| {
            let ext = Path::new(f)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            matches!(ext.as_str(), "xlsx" | "csv")
        })
        .count();
    let pptx_count = changed_files
        .iter()
        .filter(|f| {
            let ext = Path::new(f)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            ext == "pptx"
        })
        .count();
    let other_count = file_count - doc_count - xls_count - pptx_count;

    let mut parts = Vec::new();
    if doc_count > 0 {
        parts.push(format!(
            "{} document{}",
            doc_count,
            plural(doc_count)
        ));
    }
    if xls_count > 0 {
        parts.push(format!(
            "{} spreadsheet{}",
            xls_count,
            plural(xls_count)
        ));
    }
    if pptx_count > 0 {
        parts.push(format!(
            "{} presentation{}",
            pptx_count,
            plural(pptx_count)
        ));
    }
    if other_count > 0 {
        parts.push(format!("{} file{}", other_count, plural(other_count)));
    }

    format!("Sync: update {}", parts.join(", "))
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn file_type_icon(path: &str) -> &'static str {
    suture_core::file_type::detect_file_type(Path::new(path)).icon()
}

async fn cmd_push_inner(
    repo: &mut suture_core::repository::Repository,
    remote: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::remote_proto::{
        BranchProto, PushRequest, PushResponse, check_handshake, derive_repo_id,
        hex_to_hash_proto, patch_to_proto, sign_push_request,
    };
    use base64::Engine;

    let url = repo.get_remote_url(remote)?;
    check_handshake(&url).await?;

    let (branch_name, _) = repo.head()?;
    let branches = repo.list_branches();
    let branches_to_push: Vec<(String, suture_common::Hash)> = branches
        .into_iter()
        .filter(|(n, _)| *n == branch_name)
        .collect();

    if branches_to_push.is_empty() {
        return Err(format!("branch '{}' not found", branch_name).into());
    }

    let push_state_key = format!("remote.{}.last_pushed", remote);
    let patches = if let Some(last_pushed_hex) = repo.get_config(&push_state_key)? {
        let last_pushed = suture_common::Hash::from_hex(&last_pushed_hex)?;
        repo.patches_since(&last_pushed)
    } else {
        repo.all_patches()
    };

    let b64 = base64::engine::general_purpose::STANDARD;

    let mut blobs = Vec::new();
    let mut seen_hashes = std::collections::HashSet::new();
    for patch in &patches {
        let file_changes = patch.file_changes();
        let is_batch = patch.operation_type == suture_core::patch::types::OperationType::Batch;

        if is_batch {
            let changes = file_changes.as_deref().unwrap_or(&[]);
            for change in changes {
                if change.payload.is_empty() {
                    continue;
                }
                let hash_hex = String::from_utf8_lossy(&change.payload).to_string();
                if seen_hashes.contains(&hash_hex) {
                    continue;
                }
                let Ok(hash) = suture_common::Hash::from_hex(&hash_hex) else {
                    continue;
                };
                seen_hashes.insert(hash_hex.clone());
                let Ok(blob_data) = repo.cas().get_blob(&hash) else {
                    continue;
                };
                blobs.push(crate::remote_proto::BlobRef {
                    hash: hex_to_hash_proto(&hash_hex),
                    data: b64.encode(&blob_data),
                });
            }
        } else if !patch.payload.is_empty() {
            let hash_hex = String::from_utf8_lossy(&patch.payload).to_string();
            if seen_hashes.contains(&hash_hex) {
                continue;
            }
            let Ok(hash) = suture_common::Hash::from_hex(&hash_hex) else {
                continue;
            };
            seen_hashes.insert(hash_hex.clone());
            let Ok(blob_data) = repo.cas().get_blob(&hash) else {
                continue;
            };
            blobs.push(crate::remote_proto::BlobRef {
                hash: hex_to_hash_proto(&hash_hex),
                data: b64.encode(&blob_data),
            });
        }
    }

    let known_branches = repo
        .list_branches()
        .iter()
        .map(|(name, target_id)| BranchProto {
            name: name.clone(),
            target_id: hex_to_hash_proto(&target_id.to_hex()),
        })
        .collect();

    let push_body = PushRequest {
        repo_id: derive_repo_id(&url, remote),
        patches: patches.iter().map(patch_to_proto).collect(),
        branches: branches_to_push
            .iter()
            .map(|(name, target_id)| BranchProto {
                name: name.clone(),
                target_id: hex_to_hash_proto(&target_id.to_hex()),
            })
            .collect(),
        blobs,
        signature: None,
        known_branches,
        force: false,
    };

    let push_body = sign_push_request(repo, push_body)?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/push", url))
        .json(&push_body)
        .send()
        .await?;

    if resp.status().is_success() {
        let result: PushResponse = resp.json().await?;
        if result.success {
            let (_, head_id) = repo.head()?;
            repo.set_config(&push_state_key, &head_id.to_hex())?;
            Ok(())
        } else {
            Err(format!("server rejected push: {:?}", result.error).into())
        }
    } else {
        let text = resp.text().await?;
        Err(format!("push failed: {text}").into())
    }
}

// ---------------------------------------------------------------------------
// File-watching sync daemon (polling-based)
// ---------------------------------------------------------------------------

pub(crate) async fn cmd_sync_start() -> Result<(), Box<dyn std::error::Error>> {
    if is_daemon_running() {
        return Err("sync daemon is already running (use `suture sync stop` first)".into());
    }

    let repo_dir = std::env::current_dir()?;
    if !repo_dir.join(".suture").exists() {
        return Err("not a suture repository (no .suture directory)".into());
    }

    write_pid_file()?;

    let pid = std::process::id();
    eprintln!("suture sync daemon started (PID: {pid})");
    eprintln!("watching: {}", repo_dir.display());
    eprintln!("debounce: {DEBOUNCE_SECS}s | poll interval: {POLL_INTERVAL_SECS}s");
    eprintln!("press Ctrl+C to stop\n");

    let result = run_polling_watcher(&repo_dir).await;

    if let Err(e) = result {
        eprintln!("sync daemon error: {e}");
    }

    remove_pid_file();
    eprintln!("\nsync daemon stopped");
    Ok(())
}

pub(crate) fn cmd_sync_stop() -> Result<(), Box<dyn std::error::Error>> {
    let pid = read_pid_file()?;

    match pid {
        Some(pid) => {
            #[cfg(unix)]
            {
                let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
                if ret != 0 {
                    let errno = std::io::Error::last_os_error();
                    if errno.raw_os_error() == Some(3) {
                        remove_pid_file();
                        remove_last_sync_file();
                        return Err(format!(
                            "process {pid} is not running (removed stale PID file)"
                        )
                        .into());
                    }
                    return Err(format!("failed to stop daemon (PID {pid}): {errno}").into());
                }
            }
            #[cfg(not(unix))]
            {
                return Err("stopping the daemon is only supported on Unix".into());
            }

            println!("sent SIGTERM to sync daemon (PID: {pid})");

            std::thread::sleep(std::time::Duration::from_secs(1));

            if is_process_alive(pid) {
                println!("warning: daemon may still be running");
            } else {
                remove_pid_file();
                remove_last_sync_file();
                println!("sync daemon stopped");
            }
        }
        None => {
            return Err("sync daemon is not running".into());
        }
    }

    Ok(())
}

pub(crate) fn cmd_sync_status() -> Result<(), Box<dyn std::error::Error>> {
    let daemon_alive = match read_pid_file()? {
        Some(pid) => {
            if is_process_alive(pid) {
                println!("Sync daemon: running (PID {pid})");
                true
            } else {
                println!("Sync daemon: not running (stale PID file for PID: {pid})");
                println!("Run `suture sync stop` to clean up the stale PID file.");
                println!();
                false
            }
        }
        None => {
            println!("Sync daemon: not running");
            println!("Run `suture sync start` to begin syncing.");
            return Ok(());
        }
    };

    let last_sync = read_last_sync();
    match last_sync {
        Some(ts) => {
            let ago = format_ago(&ts);
            println!("Last sync: {ts} ({ago})");
        }
        None => {
            println!("Last sync: never synced");
        }
    }

    let repo = match suture_core::repository::Repository::open(Path::new(".")) {
        Ok(r) => r,
        Err(_) => {
            println!();
            println!("Not a suture repository.");
            return Ok(());
        }
    };

    let remotes = repo.list_remotes().unwrap_or_default();
    if let Some((name, url)) = remotes.first() {
        println!("Remote: {name} ({url})");
    }

    let branch = match repo.head() {
        Ok((b, _)) => b,
        Err(_) => "unknown".to_string(),
    };
    println!("Branch: {branch}");

    let conflicts = detect_pending_conflicts(&repo);
    println!();
    if conflicts.is_empty() {
        println!("No conflicts. Everything is in sync.");
    } else {
        println!("Conflicts: {} unresolved", conflicts.len());
        for c in &conflicts {
            println!("  {c}");
        }
        println!();
        println!("Run `suture merge --strategy semantic` to auto-resolve.");
    }

    let _ = daemon_alive;
    Ok(())
}

fn read_last_sync() -> Option<String> {
    let path = PathBuf::from(SYNC_LAST_SYNC_FILE);
    std::fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}

fn write_last_sync() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(SYNC_LAST_SYNC_FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = chrono::Utc::now();
    let ts = now.format("%Y-%m-%d %H:%M:%S UTC").to_string();
    std::fs::write(&path, &ts)?;
    Ok(())
}

fn format_ago(iso_ts: &str) -> String {
    let parsed = chrono::DateTime::parse_from_str(iso_ts, "%Y-%m-%d %H:%M:%S UTC");
    match parsed {
        Ok(dt) => {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(dt.with_timezone(&chrono::Utc));
            let total_secs = duration.num_seconds().unsigned_abs();
            if total_secs < 60 {
                format!("{}s ago", total_secs)
            } else if total_secs < 3600 {
                format!("{}m ago", total_secs / 60)
            } else if total_secs < 86400 {
                format!("{}h ago", total_secs / 3600)
            } else {
                format!("{}d ago", total_secs / 86400)
            }
        }
        Err(_) => "unknown".to_string(),
    }
}

fn detect_pending_conflicts(
    repo: &suture_core::repository::Repository,
) -> Vec<String> {
    let mut conflicts = Vec::new();

    if let Ok(Some(json)) = repo.meta().get_config("pending_merge_parents")
        && !json.is_empty()
        && json != "[]"
    {
        let parent_ids: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
        if !parent_ids.is_empty() {
            let msg = format!("{} pending merge parents (merge in progress)", parent_ids.len());
            conflicts.push(msg);
        }
    }

    let conflict_report = Path::new(".suture/conflicts/report.md");
    if conflict_report.exists()
        && let Ok(content) = std::fs::read_to_string(conflict_report)
    {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let desc = trimmed.trim_start_matches("- ").trim_start_matches("* ");
                if !desc.is_empty() && !conflicts.iter().any(|c| c.contains(desc)) {
                    conflicts.push(desc.to_string());
                }
            }
        }
    }

    conflicts
}

fn pid_file_path() -> PathBuf {
    PathBuf::from(SYNC_PID_FILE)
}

fn write_pid_file() -> Result<(), Box<dyn std::error::Error>> {
    let pid = std::process::id();
    let path = pid_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, pid.to_string())?;
    Ok(())
}

fn read_pid_file() -> Result<Option<u32>, Box<dyn std::error::Error>> {
    let path = pid_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let pid: u32 = content.trim().parse()?;
    Ok(Some(pid))
}

fn remove_pid_file() {
    let path = pid_file_path();
    let _ = std::fs::remove_file(path);
}

fn remove_last_sync_file() {
    let path = PathBuf::from(SYNC_LAST_SYNC_FILE);
    let _ = std::fs::remove_file(path);
}

fn is_daemon_running() -> bool {
    match read_pid_file() {
        Ok(Some(pid)) => is_process_alive(pid),
        _ => false,
    }
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(pid as i32, 0) };
        ret == 0
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn compute_file_snapshot(
    repo_dir: &Path,
) -> Result<HashMap<String, (u64, u64)>, Box<dyn std::error::Error>> {
    let mut snapshot = HashMap::new();
    snapshot_dir(repo_dir, repo_dir, &mut snapshot)?;
    Ok(snapshot)
}

fn snapshot_dir(
    root: &Path,
    current: &Path,
    snapshot: &mut HashMap<String, (u64, u64)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let entries = std::fs::read_dir(current)?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name();

        if name == ".suture" {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if path.is_dir() {
            snapshot_dir(root, &path, snapshot)?;
        } else if path.is_file() {
            let meta = entry.metadata()?;
            snapshot.insert(rel, (meta.len(), meta.modified()?.elapsed()?.as_millis() as u64));
        }
    }
    Ok(())
}

async fn run_polling_watcher(repo_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut prev_snapshot = compute_file_snapshot(repo_dir)?;

    loop {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)) => {
                let current = compute_file_snapshot(repo_dir)?;

                if current != prev_snapshot {
                    // debounce: wait for changes to settle
                    tokio::time::sleep(std::time::Duration::from_secs(DEBOUNCE_SECS)).await;

                    let settled = compute_file_snapshot(repo_dir)?;
                    if settled != prev_snapshot {
                        auto_commit_changes(repo_dir)?;
                        prev_snapshot = settled;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }

    Ok(())
}

fn auto_commit_changes(repo_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(repo_dir)?;

    let count = repo.add_all()?;
    if count == 0 {
        return Ok(());
    }

    let now = chrono::Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S");
    let message = format!("auto-sync: {count} file(s) changed at {timestamp}");

    let patch_id = repo.commit(&message)?;
    let short_id = &patch_id.to_hex()[..12.min(patch_id.to_hex().len())];
    eprintln!("[{timestamp}] auto-committed {count} file(s) -> {short_id}");

    write_last_sync()?;

    Ok(())
}
