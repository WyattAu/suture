use crate::ref_utils::parse_time_filter;

fn verify_status(
    repo: &suture_core::repository::Repository,
    patch: &suture_core::patch::types::Patch,
) -> &'static str {
    let Ok(Some(pub_key)) = repo.meta().get_public_key(&patch.author) else { return "\u{2014} unsigned" };
    let Ok(Some(sig)) = repo.meta().get_signature(&patch.id.to_hex()) else { return "\u{2014} unsigned" };
    let pub_key_arr: [u8; 32] = match pub_key.try_into() {
        Ok(k) => k,
        Err(_) => return "\u{2717} INVALID",
    };
    let sig_arr: [u8; 64] = match sig.try_into() {
        Ok(s) => s,
        Err(_) => return "\u{2717} INVALID",
    };
    let canonical = suture_core::signing::canonical_patch_bytes(
        &patch.operation_type.to_string(),
        &patch.touch_set.addresses(),
        &patch.target_path,
        &patch.payload,
        &patch.parent_ids,
        &patch.author,
        &patch.message,
        patch.timestamp,
    );
    match suture_core::signing::verify_signature(&pub_key_arr, &canonical, &sig_arr) {
        Ok(()) => "\u{2713} VALID",
        Err(_) => "\u{2717} INVALID",
    }
}

fn relative_time(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let diff = now.saturating_sub(timestamp);
    match diff {
        0..=60 => "just now".to_owned(),
        61..=3600 => format!("{}m ago", diff / 60),
        3601..=86400 => format!("{}h ago", diff / 3600),
        86401..=2592000 => format!("{}d ago", diff / 86400),
        _ => format!("{}mo ago", diff / 2592000),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn cmd_log(
    branch: Option<&str>,
    graph: bool,
    first_parent: bool,
    oneline: bool,
    author: Option<&str>,
    grep: Option<&str>,
    all: bool,
    since: Option<&str>,
    until: Option<&str>,
    stat: bool,
    diff: bool,
    audit: bool,
    audit_format: &str,
    verify: bool,
    diff_filter: Option<&str>,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    fn format_stat(patch: &suture_core::patch::types::Patch) -> String {
        let files = patch.touch_set.addresses();
        let count = files.len();
        if count == 0 {
            return String::new();
        }
        if count == 1 {
            format!(" {} file changed: {}", count, files[0])
        } else {
            format!(" {} files changed: {}", count, files.join(", "))
        }
    }

    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let since_ts = since.map(parse_time_filter).transpose()?;
    let until_ts = until.map(parse_time_filter).transpose()?;

    if audit {
        return cmd_audit(&repo, audit_format, since_ts, until_ts, author, grep).await;
    }

    let show_graph = graph && !all;

    if !show_graph {
        let mut patches = if all {
            let branches = repo.list_branches();
            let mut seen = std::collections::HashSet::new();
            let mut all_patches = Vec::with_capacity(32);
            for (_, tip_id) in &branches {
                let chain = repo.dag().patch_chain(tip_id);
                for pid in &chain {
                    if seen.insert(*pid)
                        && let Some(patch) = repo.dag().get_patch(pid)
                    {
                        all_patches.push(patch.clone());
                    }
                }
            }
            all_patches.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
            all_patches
        } else if first_parent {
            use suture_common::Hash;
            let _branch_name = branch.unwrap_or("HEAD");
            let (_head_branch, head_id) = repo
                .head()
                .unwrap_or_else(|_| ("main".to_owned(), Hash::ZERO));
            let mut chain = Vec::with_capacity(32);
            let mut current = head_id;
            while current != Hash::ZERO {
                chain.push(current);
                if let Some(patch) = repo.dag().get_patch(&current) {
                    current = patch.parent_ids.first().copied().unwrap_or(Hash::ZERO);
                } else {
                    break;
                }
            }
            let mut patches = Vec::with_capacity(32);
            for pid in &chain {
                if let Some(patch) = repo.dag().get_patch(pid) {
                    patches.push(patch.clone());
                }
            }
            patches
        } else {
            repo.log_all(branch)?
        };

        if let Some(since) = since_ts {
            patches.retain(|p| p.timestamp >= since);
        }
        if let Some(until) = until_ts {
            patches.retain(|p| p.timestamp <= until);
        }
        if let Some(author_filter) = author {
            patches.retain(|p| p.author.contains(author_filter));
        }
        if let Some(grep_filter) = grep {
            let grep_lower = grep_filter.to_lowercase();
            patches.retain(|p| p.message.to_lowercase().contains(&grep_lower));
        }
        if let Some(filter) = diff_filter {
            let filter_chars: std::collections::HashSet<char> = filter.chars().collect();
            let has_a = filter_chars.contains(&'A');
            let has_m = filter_chars.contains(&'M');
            let has_d = filter_chars.contains(&'D');
            patches.retain(|p| {
                let (added, modified, deleted) = classify_files_fast(p);
                if has_a && !added.is_empty() {
                    return true;
                }
                if has_m && !modified.is_empty() {
                    return true;
                }
                if has_d && !deleted.is_empty() {
                    return true;
                }
                false
            });
        }

        if patches.is_empty() {
            println!("No commits.");
            return Ok(());
        }

        let total = patches.len();

        if oneline {
            let display_patches: Vec<_> = if limit > 0 {
                patches.iter().take(limit).collect()
            } else {
                patches.iter().collect()
            };
            for patch in display_patches {
                let short_hash = patch.id.to_hex().chars().take(8).collect::<String>();
                if verify {
                    let vstatus = verify_status(&repo, patch);
                    if stat {
                        let stat_str = format_stat(patch);
                        println!(
                            "{} {} | {} [{}]",
                            short_hash,
                            patch.message,
                            stat_str.trim(),
                            vstatus
                        );
                    } else {
                        println!("{} {} [{}]", short_hash, patch.message, vstatus);
                    }
                } else if stat {
                    let stat_str = format_stat(patch);
                    println!("{} {} | {}", short_hash, patch.message, stat_str.trim());
                } else {
                    println!("{} {}", short_hash, patch.message);
                }
            }
            if limit > 0 && total > limit {
                println!(
                    "... and {} more commits (use --limit 0 to show all)",
                    total - limit
                );
            }
            return Ok(());
        }

        for (i, patch) in patches.iter().enumerate() {
            let prefix = if i == 0 { "* " } else { "  " };
            if verify {
                let vstatus = verify_status(&repo, patch);
                println!(
                    "{}{} {} [{}]",
                    prefix,
                    patch.id.to_hex(),
                    patch.message,
                    vstatus
                );
            } else {
                println!("{}{} {}", prefix, patch.id.to_hex(), patch.message);
            }
            if stat {
                println!("{}", format_stat(patch));
            }
            if diff {
                let parent_hex = patch
                    .parent_ids
                    .first()
                    .map(suture_core::Hash::to_hex)
                    .unwrap_or_default();
                let commit_hex = patch.id.to_hex();
                let from = if parent_hex.is_empty() {
                    None
                } else {
                    Some(parent_hex.as_str())
                };
                let entries = repo
                    .diff(from, Some(commit_hex.as_str()))
                    .unwrap_or_default();
                if !entries.is_empty() {
                    use suture_core::engine::diff::DiffType;
                    for entry in &entries {
                        match &entry.diff_type {
                            DiffType::Renamed { old_path, new_path } => {
                                println!("  renamed {old_path} → {new_path}");
                            }
                            DiffType::Added => {
                                println!("  added {}", entry.path);
                            }
                            DiffType::Deleted => {
                                println!("  deleted {}", entry.path);
                            }
                            DiffType::Modified => {
                                println!("  modified {}", entry.path);
                            }
                        }
                    }
                }
            }
        }

        return Ok(());
    }

    let mut branches = repo.list_branches();
    if branches.is_empty() {
        println!("No commits.");
        return Ok(());
    }
    // Sort branches for deterministic graph column assignment
    branches.sort_by_key(|a| a.0.clone());
    let head_branch = repo.head().map(|(name, _)| name).unwrap_or_default();

    let all_patches = repo.all_patches_ref();
    let mut commit_groups: Vec<(Vec<suture_core::patch::types::PatchId>, String, u64)> = Vec::with_capacity(32);
    let mut seen_messages: std::collections::HashMap<(String, u64), usize> =
        std::collections::HashMap::new();

    for patch in &all_patches {
        let key = (patch.message.clone(), patch.timestamp);
        if let Some(&idx) = seen_messages.get(&key) {
            commit_groups[idx].0.push(patch.id);
        } else {
            seen_messages.insert(key, commit_groups.len());
            commit_groups.push((vec![patch.id], patch.message.clone(), patch.timestamp));
        }
    }

    commit_groups.sort_by(|a, b| {
        // Primary: newest first (descending timestamp)
        b.2.cmp(&a.2)
            // Secondary: message for consistent grouping
            .then_with(|| a.1.cmp(&b.1))
            // Tertiary: first patch ID for total determinism
            .then_with(|| {
                let a_id = a.0.first().copied().unwrap_or(suture_common::Hash::ZERO);
                let b_id = b.0.first().copied().unwrap_or(suture_common::Hash::ZERO);
                a_id.cmp(&b_id)
            })
    });

    let branch_tips: std::collections::HashSet<suture_core::patch::types::PatchId> =
        branches.iter().map(|(_, id)| *id).collect();

    let tip_list: Vec<_> = branches.iter().collect();
    let mut col_assign: std::collections::HashMap<suture_core::patch::types::PatchId, usize> =
        std::collections::HashMap::new();
    for (i, (_, id)) in tip_list.iter().enumerate() {
        col_assign.insert(*id, i);
    }

    let mut next_col = tip_list.len();

    let num_cols = tip_list.len() + 5;
    for (patch_ids, message, _ts) in &commit_groups {
        let mut row = vec![' '; num_cols];
        let mut used_cols: std::collections::HashSet<usize> = std::collections::HashSet::new();

        for pid in patch_ids {
            if let Some(&col) = col_assign.get(pid) {
                row[col] = '*';
                used_cols.insert(col);
            } else {
                row[next_col % num_cols] = '*';
                used_cols.insert(next_col % num_cols);
                col_assign.insert(*pid, next_col % num_cols);
                next_col += 1;
            }
        }

        let is_tip = patch_ids.iter().any(|pid| branch_tips.contains(pid));
        if !is_tip {
            for &col in &used_cols {
                row[col] = '|';
            }
        }

        let row_str: String = row.iter().collect();
        let short_hash = patch_ids.first().map_or_else(
            || "????????".to_owned(),
            |pid| pid.to_hex().chars().take(8).collect(),
        );

        let first_patch = patch_ids.first().and_then(|pid| repo.dag().get_patch(pid));
        let author_truncated = first_patch
            .map(|p| {
                let name = p.author.trim();
                if name.len() > 12 {
                    format!("{}...", &name[..12])
                } else {
                    name.to_owned()
                }
            })
            .unwrap_or_default();
        let time_str = first_patch
            .map(|p| relative_time(p.timestamp))
            .unwrap_or_default();

        let is_merge = first_patch.is_some_and(|p| p.parent_ids.len() > 1);
        let merge_col = patch_ids
            .first()
            .and_then(|pid| col_assign.get(pid).copied())
            .unwrap_or(0);

        let labels: Vec<String> = branches
            .iter()
            .filter(|(_, id)| patch_ids.contains(id))
            .map(|(name, _)| {
                if name == &head_branch {
                    format!("HEAD -> {name}")
                } else {
                    name.clone()
                }
            })
            .collect();
        let label_str = if labels.is_empty() {
            String::new()
        } else {
            format!(" ({})", labels.join(", "))
        };

        println!(
            "{row_str} {short_hash} {author_truncated} {time_str} {message}{label_str}"
        );

        if is_merge {
            let mut merge_open = vec![' '; num_cols];
            let mut merge_close = vec![' '; num_cols];
            for &col in &used_cols {
                merge_open[col] = '|';
                merge_close[col] = '|';
            }
            if merge_col + 1 < num_cols {
                merge_open[merge_col + 1] = '\\';
                merge_close[merge_col + 1] = '/';
            }
            println!("{}", merge_open.iter().collect::<String>());
            println!("{}", merge_close.iter().collect::<String>());
        }

        if stat
            && let Some(pid) = patch_ids.first()
            && let Some(patch) = repo.dag().get_patch(pid)
        {
            println!("{}", format_stat(patch));
        }
    }

    Ok(())
}

#[derive(serde::Serialize)]
struct AuditEntry {
    timestamp: String,
    author: String,
    commit: String,
    parents: Vec<String>,
    message: String,
    files_changed: Vec<String>,
    files_added: Vec<String>,
    files_modified: Vec<String>,
    files_deleted: Vec<String>,
}

async fn cmd_audit(
    repo: &suture_core::repository::Repository,
    audit_format: &str,
    since_ts: Option<u64>,
    until_ts: Option<u64>,
    author: Option<&str>,
    grep: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let branches = repo.list_branches();
    let mut seen = std::collections::HashSet::new();
    let mut all_patches = Vec::with_capacity(32);

    for (_, tip_id) in &branches {
        let chain = repo.dag().patch_chain(tip_id);
        for pid in &chain {
            if seen.insert(*pid)
                && let Some(patch) = repo.dag().get_patch(pid)
            {
                all_patches.push(patch.clone());
            }
        }
    }

    if let Some(since) = since_ts {
        all_patches.retain(|p| p.timestamp >= since);
    }
    if let Some(until) = until_ts {
        all_patches.retain(|p| p.timestamp <= until);
    }
    if let Some(author_filter) = author {
        all_patches.retain(|p| p.author.contains(author_filter));
    }
    if let Some(grep_filter) = grep {
        let grep_lower = grep_filter.to_lowercase();
        all_patches.retain(|p| p.message.to_lowercase().contains(&grep_lower));
    }

    all_patches.sort_by_key(|a| a.timestamp);

    let entries: Vec<AuditEntry> = all_patches
        .iter()
        .map(|patch| {
            let dt = chrono::DateTime::from_timestamp(patch.timestamp as i64, 0)
                .unwrap_or_default()
                .to_rfc3339();
            let files_changed: Vec<String> = patch.touch_set.addresses();

            let (files_added, files_modified, files_deleted) = classify_files_fast(patch);

            AuditEntry {
                timestamp: dt,
                author: patch.author.clone(),
                commit: patch.id.to_hex(),
                parents: patch.parent_ids.iter().map(suture_core::Hash::to_hex).collect(),
                message: patch.message.clone(),
                files_changed,
                files_added,
                files_modified,
                files_deleted,
            }
        })
        .collect();

    match audit_format {
        "json" => {
            let json = serde_json::to_string_pretty(&entries)?;
            println!("{json}");
        }
        "csv" => {
            println!("timestamp,author,commit_hash,message,files_changed");
            for entry in &entries {
                let ts = csv_escape(&entry.timestamp);
                let author = csv_escape(&entry.author);
                let hash = csv_escape(&entry.commit);
                let msg = csv_escape(&entry.message);
                let files = csv_escape(&entry.files_changed.join("; "));
                println!("{ts},{author},{hash},{msg},{files}");
            }
        }
        _ => {
            let repo_path = std::env::current_dir()
                .unwrap_or_default()
                .display()
                .to_string();
            let generated = chrono::Utc::now().to_rfc3339();
            let total = entries.len();
            println!("=== AUDIT TRAIL ===");
            println!("Repository: {repo_path}");
            println!("Generated:  {generated}");
            println!("Total commits: {total}");
            println!();
            for entry in &entries {
                let short_hash = entry.commit.chars().take(12).collect::<String>();
                println!("--- Commit {short_hash} ---");
                println!("Timestamp:   {}", entry.timestamp);
                println!("Author:      {}", entry.author);
                println!("Message:     {}", entry.message);
                for f in &entry.files_changed {
                    let op = classify_file_label(
                        f,
                        &entry.files_added,
                        &entry.files_modified,
                        &entry.files_deleted,
                    );
                    println!("Files:       {f} ({op})");
                }
                if entry.files_changed.is_empty() {
                    println!("Files:       (none)");
                }
                println!("Parents:     {}", entry.parents.join(", "));
                println!();
            }
        }
    }

    Ok(())
}

fn classify_files_fast(
    patch: &suture_core::patch::types::Patch,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    use suture_core::patch::types::OperationType;

    let change_type = match patch.operation_type {
        OperationType::Create => "added",
        OperationType::Delete => "deleted",
        OperationType::Move => "renamed",
        OperationType::Modify
        | OperationType::Metadata
        | OperationType::Merge
        | OperationType::Identity
        | OperationType::Batch => "modified",
    };

    let files = patch.touch_set.addresses();
    match change_type {
        "added" => (files, Vec::new(), Vec::new()),
        "deleted" => (Vec::new(), Vec::new(), files),
        _ => (Vec::new(), files, Vec::new()),
    }
}

fn classify_file_label(
    file: &str,
    added: &[String],
    modified: &[String],
    deleted: &[String],
) -> &'static str {
    if added.iter().any(|f| f == file) {
        "added"
    } else if modified.iter().any(|f| f == file) {
        "modified"
    } else if deleted.iter().any(|f| f == file) {
        "deleted"
    } else {
        "changed"
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}
