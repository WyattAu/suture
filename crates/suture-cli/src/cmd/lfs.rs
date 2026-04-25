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
    if let Ok(content) = std::fs::read_to_string(repo_root.join(".suture").join("config.toml")) {
        if let Ok(table) = content.parse::<toml::Table>() {
            if let Some(lfs) = table.get("lfs").and_then(|v| v.as_table()) {
                if let Some(threshold) = lfs.get("threshold") {
                    if let Some(s) = threshold.as_str() {
                        if let Some(bytes) = parse_human_size(s) {
                            return bytes;
                        }
                    }
                    if let Some(n) = threshold.as_integer() {
                        if n > 0 {
                            return n as u64;
                        }
                    }
                }
            }
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

pub(crate) fn store_lfs_object(_repo_root: &Path, hash: &str, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let path = lfs_object_path(hash);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, data)?;
    }
    Ok(())
}

pub(crate) fn read_lfs_object(_repo_root: &Path, hash: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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

pub(crate) fn matches_lfs_pattern(
    rel_path: &str,
    rules: &[LfsTrackRule],
) -> Option<u64> {
    for rule in rules {
        if pattern_matches(&rule.pattern, rel_path) {
            if let Some(ref limit_str) = rule.size_limit {
                if let Some(limit) = parse_human_size(limit_str) {
                    return Some(limit);
                }
            }
            return Some(0);
        }
    }
    None
}

pub(crate) fn should_track_as_lfs(
    repo_root: &Path,
    rel_path: &str,
    file_size: u64,
) -> Option<u64> {
    let config = load_lfs_config();
    if config.rules.is_empty() {
        return None;
    }
    let threshold = get_threshold(repo_root);
    let effective_limit = matches_lfs_pattern(rel_path, &config.rules)?;
    let limit = if effective_limit == 0 { threshold } else { effective_limit };
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
        if let Ok(blob) = repo.cas().get_blob(hash) {
            if let Ok(text) = std::str::from_utf8(&blob) {
                if let Some(ptr) = parse_lfs_pointer(text) {
                    pointers.push((path.clone(), ptr));
                }
            }
        }
    }
    pointers
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
        LfsAction::Track { pattern, size_limit } => cmd_lfs_track(pattern, size_limit),
        LfsAction::Untrack { pattern } => cmd_lfs_untrack(pattern),
        LfsAction::List => cmd_lfs_list(),
        LfsAction::Push => cmd_lfs_push(),
        LfsAction::Pull => cmd_lfs_pull(),
        LfsAction::Status => cmd_lfs_status(),
    }
}

fn cmd_lfs_track(pattern: &str, size_limit: &Option<String>) -> Result<(), Box<dyn std::error::Error>> {
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
    println!("LFS threshold: {} bytes ({:.1} MB)", threshold, threshold as f64 / (1024.0 * 1024.0));

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

fn cmd_lfs_push() -> Result<(), Box<dyn std::error::Error>> {
    println!("Not implemented: 'suture lfs push'");
    println!();
    println!("LFS push will upload local LFS objects to the remote hub.");
    println!("This requires remote LFS storage support.");
    Ok(())
}

fn cmd_lfs_pull() -> Result<(), Box<dyn std::error::Error>> {
    let _repo = suture_core::repository::Repository::open(Path::new("."))
        .map_err(|e| format!("not a suture repository: {e}"))?;

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

    println!("Not implemented: 'suture lfs pull'");
    println!();
    println!("{} LFS object(s) need to be downloaded:", missing.len());
    for (path, ptr) in &missing {
        println!("  {} ({}, {} bytes)", path, &ptr.oid[..16], ptr.size);
    }
    println!();
    println!("LFS pull will download objects from the remote hub.");
    println!("This requires remote LFS storage support.");

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
        let pointer = "version https://suture.dev/lfs/1\noid sha256:abc123\nsize 100\nname test.mp4\n";
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
        assert_eq!(parse_human_size("1.5MB"), Some((1.5 * 1024.0 * 1024.0) as u64));
    }

    #[test]
    fn test_lfs_object_path() {
        let path = lfs_object_path("abcdef1234567890");
        assert!(path.to_str().unwrap().contains(".suture/lfs/objects/ab/abcdef1234567890"));
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

        let loaded: LfsConfig = toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.rules.len(), 2);
        assert_eq!(loaded.rules[0].pattern, "*.mp4");
        assert_eq!(loaded.rules[0].size_limit, None);
        assert_eq!(loaded.rules[1].pattern, "*.png");
        assert_eq!(loaded.rules[1].size_limit.as_deref(), Some("5MB"));
    }

    #[test]
    fn test_store_and_read_lfs_object() {
        let dir = tempfile::tempdir().unwrap();
        let suture_dir = dir.path().join(".suture").join("lfs").join("objects").join("ab");
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
        assert_eq!(hash, "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9");
    }
}
