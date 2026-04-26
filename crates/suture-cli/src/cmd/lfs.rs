use std::path::{Path, PathBuf};

const LFS_POINTER_HEADER: &str = "version https://suture.dev/lfs/1";
const DEFAULT_THRESHOLD: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct LfsPointer {
    pub oid: String,
    pub size: u64,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct LfsTrackRule {
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_limit: Option<String>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct LfsConfig {
    #[serde(rename = "track", default)]
    pub rules: Vec<LfsTrackRule>,
}

fn lfs_config_path() -> PathBuf {
    Path::new(".suture").join("lfsconfig")
}

fn lfs_objects_dir() -> PathBuf {
    Path::new(".suture").join("lfs").join("objects")
}

fn load_lfs_config() -> LfsConfig {
    let path = lfs_config_path();
    if !path.exists() {
        return LfsConfig::default();
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    toml::from_str(&content).unwrap_or_default()
}

fn save_lfs_config(config: &LfsConfig) -> Result<(), Box<dyn std::error::Error>> {
    let path = lfs_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

fn parse_human_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num_str, multiplier) = if let Some(n) = s.strip_suffix("GB") {
        (n.trim(), 1024u64 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("MB") {
        (n.trim(), 1024u64 * 1024)
    } else if let Some(n) = s.strip_suffix("KB") {
        (n.trim(), 1024u64)
    } else if let Some(n) = s.strip_suffix("B") {
        (n.trim(), 1)
    } else {
        (s, 1)
    };
    let num: f64 = num_str.parse().ok()?;
    Some((num * multiplier as f64) as u64)
}

fn get_threshold(repo_root: &Path) -> u64 {
    if let Ok(content) = std::fs::read_to_string(repo_root.join(".suture").join("config.toml"))
        && let Ok(table) = content.parse::<toml::Table>()
        && let Some(lfs) = table.get("lfs").and_then(|v| v.as_table())
        && let Some(threshold) = lfs.get("threshold")
    {
        if let Some(s) = threshold.as_str()
            && let Some(bytes) = parse_human_size(s)
        {
            return bytes;
        }
        if let Some(n) = threshold.as_integer()
            && n > 0
        {
            return n as u64;
        }
    }
    DEFAULT_THRESHOLD
}

pub(crate) fn lfs_object_path(hash: &str) -> PathBuf {
    let prefix = &hash[..2.min(hash.len())];
    lfs_objects_dir().join(prefix).join(hash)
}

pub(crate) fn is_lfs_pointer(content: &[u8]) -> bool {
    if let Ok(text) = std::str::from_utf8(content) {
        text.starts_with(LFS_POINTER_HEADER)
    } else {
        false
    }
}

pub(crate) fn parse_lfs_pointer(content: &str) -> Option<LfsPointer> {
    let mut oid = None;
    let mut size = None;
    let mut name = None;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("oid sha256:") {
            oid = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("oid blake3:") {
            oid = Some(format!("blake3:{}", val.trim()));
        } else if let Some(val) = line.strip_prefix("size ") {
            size = val.trim().parse().ok();
        } else if let Some(val) = line.strip_prefix("name ") {
            name = Some(val.trim().to_string());
        }
    }

    Some(LfsPointer {
        oid: oid?,
        size: size?,
        name: name?,
    })
}

pub(crate) fn create_lfs_pointer(hash: &str, size: u64, name: &str) -> String {
    format!(
        "{}\noid sha256:{}\nsize {}\nname {}\n",
        LFS_POINTER_HEADER, hash, size, name
    )
}

pub(crate) fn store_lfs_object(
    _repo_root: &Path,
    hash: &str,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let path = lfs_object_path(hash);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, data)?;
    }
    Ok(())
}

pub(crate) fn read_lfs_object(
    _repo_root: &Path,
    hash: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = lfs_object_path(hash);
    if !path.exists() {
        return Err(format!("LFS object not found: {}", hash).into());
    }
    Ok(std::fs::read(&path)?)
}

fn simple_glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut dp = vec![vec![false; t.len() + 1]; p.len() + 1];
    dp[0][0] = true;
    for i in 1..=p.len() {
        if p[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=p.len() {
        for j in 1..=t.len() {
            if p[i - 1] == '*' {
                dp[i][j] = dp[i - 1][j] || dp[i][j - 1];
            } else if p[i - 1] == '?' || p[i - 1] == t[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            }
        }
    }
    dp[p.len()][t.len()]
}

pub(crate) fn pattern_matches(pattern: &str, rel_path: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix('*') {
        rel_path.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        rel_path.starts_with(prefix)
    } else if pattern.contains('*') {
        simple_glob_match(pattern, rel_path)
    } else {
        rel_path == pattern || rel_path.starts_with(&format!("{}/", pattern))
    }
}

pub(crate) fn matches_lfs_pattern(rel_path: &str, rules: &[LfsTrackRule]) -> Option<u64> {
    for rule in rules {
        if pattern_matches(&rule.pattern, rel_path) {
            if let Some(ref limit_str) = rule.size_limit
                && let Some(limit) = parse_human_size(limit_str)
            {
                return Some(limit);
            }
            return Some(0);
        }
    }
    None
}

pub(crate) fn should_track_as_lfs(repo_root: &Path, rel_path: &str, file_size: u64) -> Option<u64> {
    let config = load_lfs_config();
    if config.rules.is_empty() {
        return None;
    }
    let threshold = get_threshold(repo_root);
    let effective_limit = matches_lfs_pattern(rel_path, &config.rules)?;
    let limit = if effective_limit == 0 {
        threshold
    } else {
        effective_limit
    };
    if file_size > limit {
        return Some(limit);
    }
    None
}

pub(crate) fn compute_sha256(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn count_local_lfs_objects() -> usize {
    let dir = lfs_objects_dir();
    if !dir.exists() {
        return 0;
    }
    walk_lfs_objects(&dir)
}

fn walk_lfs_objects(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += walk_lfs_objects(&path);
            } else if path.is_file() {
                count += 1;
            }
        }
    }
    count
}

fn list_lfs_pointers_in_tree() -> Vec<(String, LfsPointer)> {
    let mut pointers = Vec::new();
    let repo = match suture_core::repository::Repository::open(Path::new(".")) {
        Ok(r) => r,
        Err(_) => return pointers,
    };
    let tree = match repo.snapshot_head() {
        Ok(t) => t,
        Err(_) => return pointers,
    };
    for (path, hash) in tree.iter() {
        if let Ok(blob) = repo.cas().get_blob(hash)
            && let Ok(text) = std::str::from_utf8(&blob)
            && let Some(ptr) = parse_lfs_pointer(text)
        {
            pointers.push((path.clone(), ptr));
        }
    }
    pointers
}

pub(crate) fn resolve_lfs_pointers_in_workdir() -> Result<(usize, usize), Box<dyn std::error::Error>>
{
    let mut resolved = 0usize;
    let mut missing = 0usize;

    for entry in walkdir::WalkDir::new(".")
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.starts_with(".suture") || !path.is_file() {
            continue;
        }

        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !is_lfs_pointer(&content) {
            continue;
        }

        let text = std::str::from_utf8(&content).unwrap_or("");
        let ptr = match parse_lfs_pointer(text) {
            Some(p) => p,
            None => continue,
        };

        match read_lfs_object(Path::new("."), &ptr.oid) {
            Ok(data) => {
                std::fs::write(path, &data)?;
                resolved += 1;
            }
            Err(_) => {
                missing += 1;
                eprintln!(
                    "warning: LFS object not found: {} (run `suture lfs pull`)",
                    path.display()
                );
            }
        }
    }

    Ok((resolved, missing))
}

pub(crate) enum LfsAction {
    Track {
        pattern: String,
        size_limit: Option<String>,
    },
    Untrack {
        pattern: String,
    },
    List,
    Push,
    Pull,
    Status,
}

pub(crate) async fn cmd_lfs(action: &LfsAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        LfsAction::Track {
            pattern,
            size_limit,
        } => cmd_lfs_track(pattern, size_limit),
        LfsAction::Untrack { pattern } => cmd_lfs_untrack(pattern),
        LfsAction::List => cmd_lfs_list(),
        LfsAction::Push => cmd_lfs_push().await,
        LfsAction::Pull => cmd_lfs_pull().await,
        LfsAction::Status => cmd_lfs_status(),
    }
}

fn cmd_lfs_track(
    pattern: &str,
    size_limit: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let _repo = suture_core::repository::Repository::open(Path::new("."))
        .map_err(|e| format!("not a suture repository: {e}"))?;

    let mut config = load_lfs_config();

    if config.rules.iter().any(|r| r.pattern == pattern) {
        return Err(format!("pattern '{}' is already tracked", pattern).into());
    }

    let rule = LfsTrackRule {
        pattern: pattern.to_string(),
        size_limit: size_limit.clone(),
    };

    config.rules.push(rule);
    save_lfs_config(&config)?;

    let limit_info = match size_limit {
        Some(limit) => format!(" (size limit: {})", limit),
        None => String::new(),
    };
    println!("Tracking {}{}", pattern, limit_info);

    let threshold = get_threshold(Path::new("."));
    println!(
        "LFS threshold: {} bytes ({:.1} MB)",
        threshold,
        threshold as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}

fn cmd_lfs_untrack(pattern: &str) -> Result<(), Box<dyn std::error::Error>> {
    let _repo = suture_core::repository::Repository::open(Path::new("."))
        .map_err(|e| format!("not a suture repository: {e}"))?;

    let mut config = load_lfs_config();

    let before = config.rules.len();
    config.rules.retain(|r| r.pattern != pattern);

    if config.rules.len() == before {
        return Err(format!("pattern '{}' is not tracked", pattern).into());
    }

    save_lfs_config(&config)?;
    println!("Stopped tracking {}", pattern);

    Ok(())
}

fn cmd_lfs_list() -> Result<(), Box<dyn std::error::Error>> {
    let _repo = suture_core::repository::Repository::open(Path::new("."))
        .map_err(|e| format!("not a suture repository: {e}"))?;

    let config = load_lfs_config();

    if config.rules.is_empty() {
        println!("No LFS tracking patterns configured.");
        println!();
        println!("Use 'suture lfs track <pattern>' to start tracking large files.");
        return Ok(());
    }

    println!("LFS tracking patterns ({}):", config.rules.len());
    let threshold = get_threshold(Path::new("."));
    for rule in &config.rules {
        let effective = match &rule.size_limit {
            Some(limit) => {
                if let Some(bytes) = parse_human_size(limit) {
                    format!("(> {} bytes)", bytes)
                } else {
                    format!("(> {})", limit)
                }
            }
            None => format!("(> {} bytes, default threshold)", threshold),
        };
        println!("  {} {}", rule.pattern, effective);
    }

    Ok(())
}

async fn cmd_lfs_push() -> Result<(), Box<dyn std::error::Error>> {
    let repo_dir = std::env::current_dir()?;
    let repo = suture_core::repository::Repository::open(&repo_dir)
        .map_err(|e| format!("not a suture repository: {e}"))?;

    let remotes = repo.list_remotes().unwrap_or_default();
    if remotes.is_empty() {
        return Err("no remotes configured. Use `suture remote add origin <url>` first.".into());
    }
    let remote_name = "origin";
    let url = repo
        .get_remote_url(remote_name)
        .map_err(|e| format!("remote '{}' not found: {e}", remote_name))?;

    let pointers = list_lfs_pointers_in_tree();
    if pointers.is_empty() {
        println!("No LFS objects to push.");
        return Ok(());
    }

    let mut to_upload = Vec::new();
    for (path, ptr) in &pointers {
        let obj_path = lfs_object_path(&ptr.oid);
        if obj_path.exists() {
            to_upload.push((path.clone(), ptr.clone()));
        } else {
            eprintln!(
                "warning: LFS object missing locally: {} ({})",
                path,
                &ptr.oid[..16]
            );
        }
    }

    if to_upload.is_empty() {
        println!("All LFS objects already on remote or missing locally.");
        return Ok(());
    }

    println!("Pushing {} LFS object(s) to {}...", to_upload.len(), url);

    let client = reqwest::Client::new();
    let objects: Vec<serde_json::Value> = to_upload
        .iter()
        .map(|(_, ptr)| serde_json::json!({"oid": ptr.oid, "size": ptr.size}))
        .collect();

    let batch_body = serde_json::json!({
        "repo_id": crate::remote_proto::derive_repo_id(&url, remote_name),
        "operation": "upload",
        "objects": objects,
    });

    let batch_resp: serde_json::Value = client
        .post(format!("{}/lfs/batch", url))
        .json(&batch_body)
        .send()
        .await?
        .json()
        .await?;

    let actions = batch_resp
        .get("objects")
        .and_then(|o| o.as_array())
        .ok_or("invalid batch response: missing 'objects'")?;

    let mut uploaded = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for action_val in actions {
        let oid = action_val.get("oid").and_then(|v| v.as_str()).unwrap_or("");
        let action = action_val
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("none");

        match action {
            "none" => {
                skipped += 1;
            }
            "upload" => {
                let obj_path = lfs_object_path(oid);
                let data = match std::fs::read(&obj_path) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("  error: failed to read {}: {}", &oid[..16], e);
                        failed += 1;
                        continue;
                    }
                };

                let href = action_val
                    .get("href")
                    .and_then(|v| v.as_str())
                    .ok_or("upload action missing href")?;
                let upload_url = if href.starts_with('/') {
                    format!("{}{}", url, href)
                } else {
                    href.to_string()
                };

                let upload_resp = client
                    .put(&upload_url)
                    .header("Content-Type", "application/octet-stream")
                    .body(data)
                    .send()
                    .await?;

                if upload_resp.status().is_success() {
                    uploaded += 1;
                    println!(
                        "  uploaded: {} ({} bytes)",
                        &oid[..16],
                        action_val.get("size").and_then(|v| v.as_u64()).unwrap_or(0)
                    );
                } else {
                    eprintln!(
                        "  error: upload failed for {}: {}",
                        &oid[..16],
                        upload_resp.status()
                    );
                    failed += 1;
                }
            }
            "error" => {
                let msg = action_val
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                eprintln!("  error: server rejected {}: {}", &oid[..16], msg);
                failed += 1;
            }
            other => {
                eprintln!("  warning: unknown action '{}' for {}", other, &oid[..16]);
            }
        }
    }

    println!();
    println!(
        "LFS push complete: {} uploaded, {} skipped, {} failed",
        uploaded, skipped, failed
    );
    if failed > 0 {
        return Err(format!("{} LFS object(s) failed to upload", failed).into());
    }
    Ok(())
}

async fn cmd_lfs_pull() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))
        .map_err(|e| format!("not a suture repository: {e}"))?;

    let remotes = repo.list_remotes().unwrap_or_default();
    if remotes.is_empty() {
        return Err("no remotes configured. Use `suture remote add origin <url>` first.".into());
    }
    let remote_name = "origin";
    let url = repo
        .get_remote_url(remote_name)
        .map_err(|e| format!("remote '{}' not found: {e}", remote_name))?;

    let pointers = list_lfs_pointers_in_tree();
    let mut missing = Vec::new();

    for (path, ptr) in &pointers {
        let obj_path = lfs_object_path(&ptr.oid);
        if !obj_path.exists() {
            missing.push((path.clone(), ptr.clone()));
        }
    }

    if missing.is_empty() {
        println!("All LFS objects are present locally.");
        return Ok(());
    }

    println!(
        "Downloading {} LFS object(s) from {}...",
        missing.len(),
        url
    );

    let client = reqwest::Client::new();
    let objects: Vec<serde_json::Value> = missing
        .iter()
        .map(|(_, ptr)| serde_json::json!({"oid": ptr.oid, "size": ptr.size}))
        .collect();

    let batch_body = serde_json::json!({
        "repo_id": crate::remote_proto::derive_repo_id(&url, remote_name),
        "operation": "download",
        "objects": objects,
    });

    let batch_resp: serde_json::Value = client
        .post(format!("{}/lfs/batch", url))
        .json(&batch_body)
        .send()
        .await?
        .json()
        .await?;

    let actions = batch_resp
        .get("objects")
        .and_then(|o| o.as_array())
        .ok_or("invalid batch response: missing 'objects'")?;

    let mut downloaded = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for action_val in actions {
        let oid = action_val.get("oid").and_then(|v| v.as_str()).unwrap_or("");
        let action = action_val
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("none");

        match action {
            "none" => {
                eprintln!("  warning: object not available on remote: {}", &oid[..16]);
                skipped += 1;
            }
            "download" => {
                let href = action_val
                    .get("href")
                    .and_then(|v| v.as_str())
                    .ok_or("download action missing href")?;
                let download_url = if href.starts_with('/') {
                    format!("{}{}", url, href)
                } else {
                    href.to_string()
                };

                let download_resp = client.get(&download_url).send().await?;

                if download_resp.status().is_success() {
                    let data = download_resp.bytes().await?;
                    let repo_root = std::env::current_dir()?;
                    match store_lfs_object(&repo_root, oid, &data) {
                        Ok(()) => {
                            downloaded += 1;
                            println!("  downloaded: {} ({} bytes)", &oid[..16], data.len());
                        }
                        Err(e) => {
                            eprintln!("  error: failed to store {}: {}", &oid[..16], e);
                            failed += 1;
                        }
                    }
                } else {
                    eprintln!(
                        "  error: download failed for {}: {}",
                        &oid[..16],
                        download_resp.status()
                    );
                    failed += 1;
                }
            }
            "error" => {
                let msg = action_val
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                eprintln!("  error: server rejected {}: {}", &oid[..16], msg);
                failed += 1;
            }
            other => {
                eprintln!("  warning: unknown action '{}' for {}", other, &oid[..16]);
            }
        }
    }

    println!();
    println!(
        "LFS pull complete: {} downloaded, {} skipped, {} failed",
        downloaded, skipped, failed
    );
    if failed > 0 {
        return Err(format!("{} LFS object(s) failed to download", failed).into());
    }
    Ok(())
}

fn cmd_lfs_status() -> Result<(), Box<dyn std::error::Error>> {
    let _repo = suture_core::repository::Repository::open(Path::new("."))
        .map_err(|e| format!("not a suture repository: {e}"))?;

    let config = load_lfs_config();
    let local_count = count_local_lfs_objects();
    let pointers = list_lfs_pointers_in_tree();
    let mut missing_count = 0;

    for (_, ptr) in &pointers {
        let obj_path = lfs_object_path(&ptr.oid);
        if !obj_path.exists() {
            missing_count += 1;
        }
    }

    println!("LFS Status");
    println!("===========");
    println!("Tracked patterns: {}", config.rules.len());
    for rule in &config.rules {
        let limit = match &rule.size_limit {
            Some(l) => l.clone(),
            None => "default".to_string(),
        };
        println!("  {} (limit: {})", rule.pattern, limit);
    }
    println!();
    println!("Local LFS objects: {}", local_count);
    println!("LFS pointers in tree: {}", pointers.len());
    println!("Missing LFS objects: {}", missing_count);

    if missing_count > 0 {
        println!();
        println!("Run 'suture lfs pull' to download missing objects.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_lfs_pointer() {
        let pointer =
            "version https://suture.dev/lfs/1\noid sha256:abc123\nsize 100\nname test.mp4\n";
        assert!(is_lfs_pointer(pointer.as_bytes()));
        assert!(!is_lfs_pointer(b"not a pointer"));
        assert!(!is_lfs_pointer(b""));
    }

    #[test]
    fn test_parse_lfs_pointer() {
        let pointer = "version https://suture.dev/lfs/1\noid sha256:abcdef123456\nsize 1024\nname video.mp4\n";
        let parsed = parse_lfs_pointer(pointer).unwrap();
        assert_eq!(parsed.oid, "abcdef123456");
        assert_eq!(parsed.size, 1024);
        assert_eq!(parsed.name, "video.mp4");
    }

    #[test]
    fn test_create_lfs_pointer() {
        let pointer = create_lfs_pointer("abc123", 2048, "test.bin");
        assert!(pointer.starts_with(LFS_POINTER_HEADER));
        assert!(pointer.contains("oid sha256:abc123"));
        assert!(pointer.contains("size 2048"));
        assert!(pointer.contains("name test.bin"));
    }

    #[test]
    fn test_parse_lfs_pointer_roundtrip() {
        let hash = "deadbeefcafebabe1234567890abcdef";
        let pointer_str = create_lfs_pointer(hash, 99999, "movie.mp4");
        let parsed = parse_lfs_pointer(&pointer_str).unwrap();
        assert_eq!(parsed.oid, hash);
        assert_eq!(parsed.size, 99999);
        assert_eq!(parsed.name, "movie.mp4");
    }

    #[test]
    fn test_parse_human_size() {
        assert_eq!(parse_human_size("10MB"), Some(10 * 1024 * 1024));
        assert_eq!(parse_human_size("500KB"), Some(500 * 1024));
        assert_eq!(parse_human_size("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_human_size("1024"), Some(1024));
        assert_eq!(parse_human_size("5B"), Some(5));
        assert_eq!(
            parse_human_size("1.5MB"),
            Some((1.5 * 1024.0 * 1024.0) as u64)
        );
    }

    #[test]
    fn test_lfs_object_path() {
        let path = lfs_object_path("abcdef1234567890");
        let path_str = path.to_str().unwrap();
        // Use forward-slash replacement for cross-platform comparison (Windows uses \)
        let normalized = path_str.replace('\\', "/");
        assert!(normalized.contains(".suture/lfs/objects/ab/abcdef1234567890"));
    }

    #[test]
    fn test_pattern_matches() {
        assert!(pattern_matches("*.mp4", "video.mp4"));
        assert!(pattern_matches("*.mp4", "dir/video.mp4"));
        assert!(!pattern_matches("*.mp4", "video.txt"));
        assert!(pattern_matches("assets/*", "assets/image.png"));
        assert!(!pattern_matches("assets/*", "other/image.png"));
        assert!(pattern_matches("*.png", "thumbnail.png"));
    }

    #[test]
    fn test_matches_lfs_pattern() {
        let rules = vec![
            LfsTrackRule {
                pattern: "*.mp4".to_string(),
                size_limit: None,
            },
            LfsTrackRule {
                pattern: "*.png".to_string(),
                size_limit: Some("5MB".to_string()),
            },
        ];

        assert!(matches_lfs_pattern("video.mp4", &rules).is_some());
        assert!(matches_lfs_pattern("image.png", &rules).is_some());
        assert!(matches_lfs_pattern("readme.txt", &rules).is_none());
    }

    #[test]
    fn test_matches_lfs_pattern_with_size_limit() {
        let rules = vec![LfsTrackRule {
            pattern: "*.png".to_string(),
            size_limit: Some("100B".to_string()),
        }];

        let result = matches_lfs_pattern("image.png", &rules);
        assert_eq!(result, Some(100));
    }

    #[test]
    fn test_lfs_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".suture").join("lfsconfig");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        let config = LfsConfig {
            rules: vec![
                LfsTrackRule {
                    pattern: "*.mp4".to_string(),
                    size_limit: None,
                },
                LfsTrackRule {
                    pattern: "*.png".to_string(),
                    size_limit: Some("5MB".to_string()),
                },
            ],
        };

        let content = toml::to_string_pretty(&config).unwrap();
        std::fs::write(&config_path, &content).unwrap();

        let loaded: LfsConfig =
            toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.rules.len(), 2);
        assert_eq!(loaded.rules[0].pattern, "*.mp4");
        assert_eq!(loaded.rules[0].size_limit, None);
        assert_eq!(loaded.rules[1].pattern, "*.png");
        assert_eq!(loaded.rules[1].size_limit.as_deref(), Some("5MB"));
    }

    #[test]
    fn test_store_and_read_lfs_object() {
        let dir = tempfile::tempdir().unwrap();
        let suture_dir = dir
            .path()
            .join(".suture")
            .join("lfs")
            .join("objects")
            .join("ab");
        std::fs::create_dir_all(&suture_dir).unwrap();

        let hash = "abcdef1234567890";
        let data = b"hello lfs world";

        let path = lfs_object_path(hash);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, data).unwrap();

        let read_back = std::fs::read(&path).unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn test_compute_sha256() {
        let hash = compute_sha256(b"test data");
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9"
        );
    }
}
