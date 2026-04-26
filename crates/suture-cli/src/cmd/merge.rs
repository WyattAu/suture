use crate::cmd::user_error;
use crate::style::run_hook_if_exists;

/// Merge conflict resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeStrategy {
    /// Try semantic drivers first, fall back to conflict markers (default).
    Semantic,
    /// Keep our version for all conflicts.
    Ours,
    /// Keep their version for all conflicts.
    Theirs,
    /// Leave all conflicts as conflict markers (skip semantic drivers).
    Manual,
}

impl MergeStrategy {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "semantic" | "auto" => Some(MergeStrategy::Semantic),
            "ours" | "keep-ours" => Some(MergeStrategy::Ours),
            "theirs" | "keep-theirs" => Some(MergeStrategy::Theirs),
            "manual" | "none" => Some(MergeStrategy::Manual),
            _ => None,
        }
    }
}

pub(crate) async fn cmd_merge_abort() -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;

    let parents_json = match repo.get_config("pending_merge_parents") {
        Ok(Some(v)) => v,
        _ => {
            println!("no merge in progress");
            return Ok(());
        }
    };

    let parents: Vec<suture_common::Hash> = serde_json::from_str(&parents_json)
        .map_err(|e| format!("failed to parse pending_merge_parents: {}", e))?;

    if parents.len() != 2 {
        return Err(format!(
            "invalid pending_merge_parents (expected 2 entries, got {})",
            parents.len()
        )
        .into());
    }

    let original_head = parents[0];

    let (branch, _) = repo
        .head()
        .map_err(|e| user_error("failed to get HEAD", e))?;

    let old_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::FileTree::empty());

    let bn = suture_common::BranchName::new(&branch)
        .map_err(|e| format!("invalid branch name '{}': {}", branch, e))?;
    repo.dag_mut()
        .update_branch(&bn, original_head)
        .map_err(|e| user_error("failed to update branch", e))?;
    repo.invalidate_head_cache();

    repo.sync_working_tree(&old_tree)
        .map_err(|e| user_error("failed to sync working tree", e))?;

    repo.set_config("pending_merge_parents", "")?;
    let _ = repo
        .meta()
        .conn()
        .execute("DELETE FROM config WHERE key = 'pending_merge_parents'", []);
    let _ = repo
        .meta()
        .conn()
        .execute("DELETE FROM config WHERE key = 'pending_merge_branch'", []);

    let conflicts_dir = repo.root().join(".suture").join("conflicts");
    if conflicts_dir.exists() {
        let _ = std::fs::remove_dir_all(&conflicts_dir);
    }
    let suture_conflicts_dir = repo.root().join(".suture_conflicts");
    if suture_conflicts_dir.exists() {
        let _ = std::fs::remove_dir_all(&suture_conflicts_dir);
    }

    println!("Merge aborted, restored to pre-merge state");
    Ok(())
}

pub(crate) async fn cmd_merge_continue() -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;

    match repo.get_config("pending_merge_parents") {
        Ok(Some(_)) => {}
        _ => {
            println!("no merge in progress");
            return Ok(());
        }
    }

    let report_path = repo
        .root()
        .join(".suture")
        .join("conflicts")
        .join("report.md");
    if report_path.exists() {
        println!("there are still unresolved conflicts");
        println!("resolve conflicts in the affected files, then run `suture merge --continue`");
        return Ok(());
    }

    let count = repo
        .add_all()
        .map_err(|e| user_error("failed to stage changes", e))?;
    if count == 0 {
        return Err("no changes to commit".into());
    }

    let branch_name = repo
        .get_config("pending_merge_branch")
        .ok()
        .flatten()
        .unwrap_or_else(|| "unknown".to_string());
    let message = format!("Merge branch '{}' (conflicts resolved)", branch_name);

    let patch_id = repo
        .commit(&message)
        .map_err(|e| user_error("failed to commit", e))?;
    println!("Merge complete: {}", patch_id);

    let _ = repo
        .meta()
        .conn()
        .execute("DELETE FROM config WHERE key = 'pending_merge_branch'", []);

    let conflicts_dir = repo.root().join(".suture").join("conflicts");
    if conflicts_dir.exists() {
        let _ = std::fs::remove_dir_all(&conflicts_dir);
    }
    let suture_conflicts_dir = repo.root().join(".suture_conflicts");
    if suture_conflicts_dir.exists() {
        let _ = std::fs::remove_dir_all(&suture_conflicts_dir);
    }

    Ok(())
}

pub(crate) async fn cmd_merge(
    source: &str,
    dry_run: bool,
    strategy: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path as StdPath;
    use suture_core::repository::ConflictInfo;

    let merge_strategy = MergeStrategy::parse(strategy).ok_or_else(|| {
        format!(
            "unknown merge strategy '{}' (expected: semantic, ours, theirs, manual)",
            strategy
        )
    })?;

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;

    let status = repo
        .status()
        .map_err(|e| user_error("failed to check repository status", e))?;
    if !status.staged_files.is_empty() {
        return Err("cannot merge with staged changes (commit or stash them first)".into());
    }

    let branches: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
    if !branches.contains(&source.to_string()) && repo.resolve_ref(source).is_err() {
        let hint = if let Some(suggestion) = crate::fuzzy::suggest(source, &branches) {
            format!(" (did you mean '{}'?)", suggestion)
        } else {
            String::new()
        };
        return Err(format!(
            "branch '{}' not found{hint} (use 'suture branch' to create it)",
            source
        )
        .into());
    }

    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_string(), branch.clone());
    pre_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    pre_extra.insert("SUTURE_MERGE_SOURCE".to_string(), source.to_string());

    if dry_run {
        println!(
            "DRY RUN: previewing merge of '{}' into current branch (strategy: {})",
            source, strategy
        );
    } else {
        run_hook_if_exists(repo.root(), "pre-merge", pre_extra)?;
    }

    // Save pre-merge working tree content for semantic re-merge.
    let pre_merge_files: std::collections::HashMap<String, String> = {
        let root = repo.root();
        let mut map = std::collections::HashMap::new();
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Ok(relative) = path.strip_prefix(root)
                    && let Ok(content) = std::fs::read_to_string(&path)
                {
                    map.insert(relative.to_string_lossy().to_string(), content);
                }
            }
        }
        map
    };

    // Capture the pre-merge HEAD ID so we can compute the correct LCA
    // for semantic re-merge. After execute_merge(), HEAD points to the
    // merge commit, which would make the LCA trivially the source branch.
    let pre_merge_head_id = repo.head().ok().map(|(_, id)| id);

    let result = if dry_run {
        repo.preview_merge(source)
            .map_err(|e| user_error("failed to preview merge", e))?
    } else {
        repo.execute_merge(source)
            .map_err(|e| user_error("merge failed", e))?
    };

    if !result.is_clean && !dry_run {
        let _ = repo.set_config("pending_merge_branch", source);
    }

    if result.is_clean {
        if dry_run {
            if result.patches_applied > 0 {
                println!(
                    "Would apply {} patch(es) from '{}'",
                    result.patches_applied, source
                );
            } else {
                println!("Already up to date.");
            }
            println!("DRY RUN — no files were modified.");
            return Ok(());
        }

        if let Some(id) = result.merge_patch_id {
            println!("Merge successful: {}", id);
        }
        if result.patches_applied > 0 {
            println!(
                "Applied {} patch(es) from '{}'",
                result.patches_applied, source
            );
        }

        // ── Semantic re-merge for clean merges ──
        // When both branches edit different parts of the same structured file,
        // the text-based clean merge may silently overwrite one side's changes.
        // We detect this, re-merge with semantic drivers, and rewrite the
        // merge commit's file tree to include the corrected content.
        if matches!(merge_strategy, MergeStrategy::Semantic) && result.patches_applied > 0 {
            match semantic_remerge_both_modified(
                &repo,
                source,
                &pre_merge_files,
                &pre_merge_head_id,
            ) {
                Ok(semantic_count) if semantic_count > 0 => {
                    println!(
                        "Re-merged {} file(s) via semantic drivers (both branches modified)",
                        semantic_count
                    );
                    // Rewrite the merge commit's stored tree to match the
                    // corrected working tree. This ensures diff, checkout,
                    // and subsequent merges see the semantically correct content.
                    if let Err(e) = repo.rewrite_head_tree() {
                        eprintln!("  warning: could not update merge tree: {}", e);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("  warning: semantic re-merge skipped: {}", e);
                }
            }
        }

        let (branch, head_id) = repo.head()?;
        let mut post_extra = std::collections::HashMap::new();
        post_extra.insert("SUTURE_BRANCH".to_string(), branch.clone());
        post_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
        post_extra.insert("SUTURE_MERGE_SOURCE".to_string(), source.to_string());
        run_hook_if_exists(repo.root(), "post-merge", post_extra)?;

        {
            let author = repo
                .get_config("user.name")
                .unwrap_or(None)
                .unwrap_or_default();
            let audit_dir = repo.root().join(".suture").join("audit").join("chain.log");
            if let Ok(audit) = suture_core::audit::AuditLog::open(&audit_dir) {
                let merge_id = result.merge_patch_id.map(|id| id.to_hex());
                let details = serde_json::json!({
                    "source": source,
                    "merge_patch_id": merge_id,
                    "patches_applied": result.patches_applied,
                    "strategy": strategy,
                })
                .to_string();
                let _ = audit.append(&author, "merge", &details);
            }
        }

        return Ok(());
    }

    // Handle conflicts
    let conflicts = result.unresolved_conflicts;
    let mut remaining: Vec<ConflictInfo> = Vec::new();
    let mut resolved_count = 0usize;
    let mut ooxml_resolved = 0usize;

    // OOXML files (.docx, .xlsx, .pptx) are ZIP-based binary formats.
    // Writing text conflict markers inside a ZIP corrupts the file,
    // making it impossible to open in Word/Excel/PowerPoint.
    // For non-"theirs" strategies, restore "ours" version and generate a report.
    let conflicts = if dry_run {
        let ooxml_count = conflicts.iter().filter(|c| is_ooxml_file(&c.path)).count();
        if ooxml_count > 0 {
            println!(
                "Note: {} OOXML file(s) will have your version preserved (conflict markers would corrupt binary format)",
                ooxml_count
            );
        }
        conflicts
    } else if matches!(merge_strategy, MergeStrategy::Theirs) {
        // Theirs strategy writes correct binary blobs — include OOXML in normal flow
        conflicts
    } else {
        let (ooxml, text): (Vec<_>, Vec<_>) =
            conflicts.into_iter().partition(|c| is_ooxml_file(&c.path));

        if !ooxml.is_empty() {
            for conflict in &ooxml {
                if let Some(hash) = conflict.our_content_hash
                    && let Ok(blob) = repo.cas().get_blob(&hash)
                {
                    if let Err(e) = std::fs::write(&conflict.path, &blob) {
                        eprintln!("Warning: could not restore '{}': {}", conflict.path, e);
                    } else {
                        ooxml_resolved += 1;
                        let _ = repo.add(&conflict.path);
                    }
                }
            }
            if let Err(e) = generate_ooxml_conflict_report(&ooxml, repo.root()) {
                eprintln!("Warning: could not write conflict report: {}", e);
            }
            println!(
                "Preserved your version for {} OOXML file(s) (see .suture_conflicts/report.md)",
                ooxml.len()
            );
        }

        text
    };

    if dry_run {
        if result.patches_applied > 0 {
            println!(
                "Would apply {} patch(es) from '{}'",
                result.patches_applied, source
            );
        }

        // Preview conflict resolution based on strategy
        match merge_strategy {
            MergeStrategy::Ours => {
                println!(
                    "Would resolve {} conflict(s) by keeping our version",
                    conflicts.len()
                );
            }
            MergeStrategy::Theirs => {
                println!(
                    "Would resolve {} conflict(s) by keeping their version",
                    conflicts.len()
                );
            }
            MergeStrategy::Semantic => {
                let registry = crate::driver_registry::builtin_registry();
                for conflict in &conflicts {
                    let path = StdPath::new(&conflict.path);
                    if registry.get_for_path(path).is_ok() {
                        resolved_count += 1;
                    } else {
                        remaining.push(conflict.clone());
                    }
                }
                if resolved_count > 0 {
                    println!(
                        "Would resolve {} conflict(s) via semantic drivers",
                        resolved_count
                    );
                }
                if remaining.is_empty() && resolved_count > 0 {
                    println!("All conflicts would be resolved via semantic drivers.");
                } else if !remaining.is_empty() {
                    println!("{} conflict(s) would remain unresolved:", remaining.len());
                    for conflict in &remaining {
                        println!("  CONFLICT in '{}'", conflict.path);
                    }
                }
            }
            MergeStrategy::Manual => {
                println!(
                    "{} conflict(s) left for manual resolution:",
                    conflicts.len()
                );
                for conflict in &conflicts {
                    println!("  CONFLICT in '{}'", conflict.path);
                }
            }
        }
        println!("DRY RUN — no files were modified.");
        return Ok(());
    }

    // Not dry-run — actually resolve conflicts
    match merge_strategy {
        MergeStrategy::Ours => {
            // Keep our version for all conflicts — just leave them as-is
            // (our content is already in the working tree)
            println!(
                "Resolved {} conflict(s) by keeping our version",
                ooxml_resolved + conflicts.len()
            );
            resolved_count += conflicts.len();
        }
        MergeStrategy::Theirs => {
            // Take their version for all conflicts
            for conflict in &conflicts {
                if let Some(hash) = conflict.their_content_hash
                    && let Ok(blob) = repo.cas().get_blob(&hash)
                {
                    if let Err(e) = std::fs::write(&conflict.path, &blob) {
                        eprintln!("Warning: could not write '{}': {}", conflict.path, e);
                    } else {
                        let _ = repo.add(&conflict.path);
                        resolved_count += 1;
                    }
                }
            }
            println!(
                "Resolved {} conflict(s) by keeping their version",
                resolved_count
            );
        }
        MergeStrategy::Semantic => {
            let registry = crate::driver_registry::builtin_registry();
            for conflict in &conflicts {
                let path = StdPath::new(&conflict.path);
                let Ok(driver) = registry.get_for_path(path) else {
                    remaining.push(conflict.clone());
                    continue;
                };

                let base_content = conflict
                    .base_content_hash
                    .and_then(|h| repo.cas().get_blob(&h).ok());
                let ours_content = conflict
                    .our_content_hash
                    .and_then(|h| repo.cas().get_blob(&h).ok());
                let theirs_content = conflict
                    .their_content_hash
                    .and_then(|h| repo.cas().get_blob(&h).ok());

                // Prefer merge_raw (byte-level) for binary formats — avoids
                // unsafe String::from_utf8_unchecked in binary drivers.
                // Fall back to merge (string-level) for text drivers.
                let merged = match (
                    base_content.as_deref(),
                    ours_content.as_deref(),
                    theirs_content.as_deref(),
                ) {
                    (Some(base_b), Some(ours_b), Some(theirs_b)) => {
                        // Try merge_raw first, then fall back to text merge.
                        match driver.merge_raw(base_b, ours_b, theirs_b) {
                            Ok(result) => Ok(result),
                            Err(_) => {
                                let base_s = String::from_utf8_lossy(base_b);
                                let ours_s = String::from_utf8_lossy(ours_b);
                                let theirs_s = String::from_utf8_lossy(theirs_b);
                                driver
                                    .merge(&base_s, &ours_s, &theirs_s)
                                    .map(|opt| opt.map(|s| s.into_bytes()))
                            }
                        }
                    }
                    _ => Err(suture_driver::DriverError::ParseError(
                        "missing content".into(),
                    )),
                };
                let Ok(merged) = merged else {
                    remaining.push(conflict.clone());
                    continue;
                };
                let Some(content) = merged else {
                    remaining.push(conflict.clone());
                    continue;
                };

                if let Err(e) = std::fs::write(&conflict.path, &content) {
                    eprintln!(
                        "Warning: could not write resolved file '{}': {e}",
                        conflict.path
                    );
                    remaining.push(conflict.clone());
                    continue;
                }

                if let Err(e) = repo.add(&conflict.path) {
                    eprintln!(
                        "Warning: could not stage resolved file '{}': {e}",
                        conflict.path
                    );
                    remaining.push(conflict.clone());
                    continue;
                }

                let driver_name = driver.name().to_lowercase();
                println!("Resolved {} via {} driver", conflict.path, driver_name);
                resolved_count += 1;
            }
        }
        MergeStrategy::Manual => {
            // Leave all conflicts as-is (conflict markers are already written)
            remaining = conflicts;
        }
    }

    let total_conflicts = remaining.len();
    println!("Merge has {} conflict(s):", total_conflicts);

    let (binary_conflicts, text_conflicts): (Vec<_>, Vec<_>) =
        remaining.into_iter().partition(|c| {
            is_binary_path(&c.path)
                || c.our_content_hash
                    .as_ref()
                    .and_then(|h| repo.cas().get_blob(h).ok())
                    .is_some_and(|b| b.contains(&0))
                || c.their_content_hash
                    .as_ref()
                    .and_then(|h| repo.cas().get_blob(h).ok())
                    .is_some_and(|b| b.contains(&0))
        });

    if !binary_conflicts.is_empty() {
        let conflicts_dir = repo.root().join(".suture").join("conflicts");
        let _ = std::fs::create_dir_all(&conflicts_dir);

        let mut report = String::new();
        report.push_str("# Merge Conflict Report\n");
        report.push_str(&format!(
            "Generated: {}\n",
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
        ));
        report.push_str(&format!("Branch: {} → {}\n", source, branch));
        report.push_str("\n## Binary File Conflicts\n");

        for conflict in &binary_conflicts {
            if let Some(hash) = conflict.their_content_hash
                && let Ok(blob) = repo.cas().get_blob(&hash)
            {
                let theirs_path = conflicts_dir.join(format!("{}.theirs", conflict.path));
                if let Some(parent) = theirs_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&theirs_path, &blob);
            }

            let theirs_rel = format!(".suture/conflicts/{}.theirs", conflict.path);
            println!("  CONFLICT (binary) in '{}': both modified", conflict.path);
            println!("    Your version preserved. Their version: {}", theirs_rel);

            report.push_str(&format!("\n### {}\n", conflict.path));
            report.push_str("- **Status:** Both sides modified\n");
            report.push_str("- **Your version:** preserved in working tree\n");
            report.push_str(&format!("- **Their version:** saved to `{}`\n", theirs_rel));
            report.push_str(
                "- **Resolution:** Choose which version to keep, or manually merge in an external tool\n",
            );
        }

        if !text_conflicts.is_empty() {
            report.push_str("\n## Text File Conflicts\n\n");
            for conflict in &text_conflicts {
                report.push_str(&format!("### {}\n", conflict.path));
                report.push_str("- **Status:** Both sides modified (conflict markers in file)\n");
            }
        }

        let _ = std::fs::write(conflicts_dir.join("report.md"), report);
    }

    for conflict in &text_conflicts {
        let our_content = conflict
            .our_content_hash
            .and_then(|h| repo.cas().get_blob(&h).ok())
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default();
        let their_content = conflict
            .their_content_hash
            .and_then(|h| repo.cas().get_blob(&h).ok())
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default();
        println!("  CONFLICT in '{}':", conflict.path);
        println!("    ours:\n{}", indent(&our_content, "      "));
        println!("    theirs:\n{}", indent(&their_content, "      "));
    }

    if binary_conflicts.is_empty() && text_conflicts.is_empty() {
        let via = match merge_strategy {
            MergeStrategy::Semantic => "semantic drivers",
            MergeStrategy::Ours => "keeping our version",
            MergeStrategy::Theirs => "keeping their version",
            MergeStrategy::Manual => unreachable!(),
        };
        if ooxml_resolved > 0 && resolved_count > ooxml_resolved {
            println!(
                "All conflicts resolved. {} via {}, {} OOXML preserved.",
                resolved_count - ooxml_resolved,
                via,
                ooxml_resolved
            );
        } else if ooxml_resolved > 0 {
            println!(
                "All {} conflict(s) resolved. {} OOXML file(s) preserved (your version).",
                resolved_count, ooxml_resolved
            );
        } else {
            println!("All conflicts resolved. {} via {}.", resolved_count, via);
        }
        println!("Run `suture commit` to finalize the merge.");
    } else {
        println!("Edit the file(s), then run `suture commit` to resolve");
        println!("Hint: resolve conflicts, then run `suture commit`");
    }

    Ok(())
}

fn indent(s: &str, prefix: &str) -> String {
    s.lines()
        .map(|line| format!("{}{}", prefix, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_ooxml_file(path: &str) -> bool {
    is_binary_path(path)
}

fn is_binary_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .is_some_and(|ext| {
            matches!(
                ext.as_str(),
                "docx"
                    | "xlsx"
                    | "pptx"
                    | "pdf"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "bmp"
                    | "webp"
                    | "svg"
                    | "ico"
                    | "tiff"
            )
        })
}

fn generate_ooxml_conflict_report(
    conflicts: &[suture_core::repository::ConflictInfo],
    root: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let report_dir = root.join(".suture_conflicts");
    std::fs::create_dir_all(&report_dir)?;

    let mut report = String::from("=== Merge Conflicts ===\n\n");

    for conflict in conflicts {
        let ext = std::path::Path::new(&conflict.path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_uppercase())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        report.push_str(&format!("{} ({})\n", conflict.path, ext));
        report.push_str("  Could not automatically merge. Your version has been preserved.\n\n");

        if conflict.our_content_hash.is_some() {
            let our_size = std::fs::metadata(&conflict.path)
                .map(|m| m.len())
                .unwrap_or(0);
            report.push_str(&format!("  Your version: {} bytes\n", our_size));
            if let Some(their_hash) = conflict.their_content_hash {
                let hex = suture_common::Hash::to_hex(&their_hash);
                report.push_str(&format!("  Their version: blob {}\n", &hex[..12]));
            }
            if let Some(base_hash) = conflict.base_content_hash {
                let hex = suture_common::Hash::to_hex(&base_hash);
                report.push_str(&format!("  Base version: blob {}\n", &hex[..12]));
            }
        }

        report.push_str("\n  To resolve:\n");
        report.push_str("    1. Open the file (your version is preserved)\n");
        report.push_str("    2. Contact the other editor to see their changes\n");
        report.push_str(&format!(
            "    3. Make your edits and run: suture add {} && suture commit \"resolved conflict\"\n\n",
            conflict.path
        ));
    }

    std::fs::write(report_dir.join("report.md"), report)?;
    Ok(())
}

/// After a clean merge, detect files modified by both branches and re-merge
/// them using semantic drivers. This prevents silent data loss when both
/// branches edit different parts of the same structured file (YAML, JSON, etc.)
/// and the text-based patch application overwrites one side's changes.
/// Manual line-level merge for when we don't have a reliable base.
///
/// This combines lines from both versions by:
/// 1. Finding lines unique to ours (not in theirs)
/// 2. Finding lines unique to theirs (not in ours)  
/// 3. Keeping shared lines once
/// 4. Inserting theirs-only lines after the corresponding ours context
///
/// This is a best-effort approach — not a proper 3-way merge — but it's
/// better than silently dropping one side's changes.
fn manual_line_merge(ours: &str, theirs: &str) -> String {
    let ours_lines: Vec<&str> = ours.lines().collect();
    let theirs_lines: Vec<&str> = theirs.lines().collect();

    let ours_set: std::collections::HashSet<&str> = ours_lines.iter().copied().collect();
    let theirs_set: std::collections::HashSet<&str> = theirs_lines.iter().copied().collect();

    let ours_only: Vec<&str> = ours_lines
        .iter()
        .filter(|l| !theirs_set.contains(*l))
        .copied()
        .collect();
    let theirs_only: Vec<&str> = theirs_lines
        .iter()
        .filter(|l| !ours_set.contains(*l))
        .copied()
        .collect();

    // If one side has no unique changes, the other side wins
    if ours_only.is_empty() {
        return theirs.to_string();
    }
    if theirs_only.is_empty() {
        return ours.to_string();
    }

    // Both sides have unique changes — build combined output.
    // Strategy: keep ours as the base, insert theirs-only lines at appropriate positions.
    let mut result = Vec::new();
    let mut theirs_idx = 0;

    for line in &ours_lines {
        result.push(line.to_string());

        // After each ours line, check if any theirs-only lines should go here
        while theirs_idx < theirs_only.len() {
            if theirs_idx + 1 < theirs_lines.len() {
                let next_theirs = theirs_lines[theirs_idx + 1];
                if ours_set.contains(next_theirs)
                    && result.iter().any(|r| r.trim() == next_theirs.trim())
                {
                    result.push(theirs_only[theirs_idx].to_string());
                    theirs_idx += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    while theirs_idx < theirs_only.len() {
        result.push(theirs_only[theirs_idx].to_string());
        theirs_idx += 1;
    }

    result.join("\n")
}

fn semantic_remerge_both_modified(
    repo: &suture_core::repository::Repository,
    source_branch: &str,
    pre_merge_files: &std::collections::HashMap<String, String>,
    pre_merge_head_id: &Option<suture_common::Hash>,
) -> Result<usize, Box<dyn std::error::Error>> {
    use std::path::Path as StdPath;

    let registry = crate::driver_registry::builtin_registry();

    // Use the pre-merge HEAD ID (captured before execute_merge) to compute
    // the correct LCA. After the merge, HEAD points to the merge commit,
    // which would make LCA(merge, source) = source (trivially wrong).
    let head_id = pre_merge_head_id
        .as_ref()
        .ok_or_else(|| "pre-merge HEAD ID not available".to_string())?;
    let bn = suture_common::BranchName::new(source_branch)
        .map_err(|e| format!("invalid branch name '{}': {}", source_branch, e))?;
    let source_id = repo
        .dag()
        .get_branch(&bn)
        .ok_or_else(|| format!("branch '{}' not found", source_branch))?;

    // Compute the LCA between HEAD and the source branch tip.
    // The LCA is the true 3-way merge base — it represents the last
    // common ancestor of both branches, which is what a proper 3-way
    // merge needs to correctly combine both sides' changes.
    let lca_id = repo
        .dag()
        .lca(head_id, &source_id)
        .ok_or_else(|| "no common ancestor found for merge".to_string())?;

    // Build LCA file tree snapshot to get the base content for 3-way merge.
    // Use snapshot_fresh to bypass any stale SQLite cache.
    let lca_tree = repo
        .snapshot_fresh(&lca_id)
        .map_err(|e| format!("failed to snapshot LCA: {}", e))?;

    // Build LCA file content map from the snapshot.
    // FileTree maps path → blob hash; we resolve each hash to content via CAS.
    let lca_files: std::collections::HashMap<String, String> = lca_tree
        .iter()
        .filter_map(|(path, hash)| {
            let content = repo.cas().get_blob(hash).ok()?;
            Some((path.clone(), String::from_utf8(content).ok()?))
        })
        .collect();

    // Also get the source branch's file tree (the "theirs" side).
    // Use snapshot_fresh for correctness.
    let source_tree = repo
        .snapshot_fresh(&source_id)
        .map_err(|e| format!("failed to snapshot source: {}", e))?;

    let source_files: std::collections::HashMap<String, String> = source_tree
        .iter()
        .filter_map(|(path, hash)| {
            let content = repo.cas().get_blob(hash).ok()?;
            Some((path.clone(), String::from_utf8(content).ok()?))
        })
        .collect();

    // Find files that changed on the source branch AND exist in the current
    // HEAD (pre-merge). These are candidates for semantic re-merge.
    let both_modified: Vec<String> = source_files
        .keys()
        .filter(|path| {
            pre_merge_files.contains_key(path.as_str())
                && source_files.get(path.as_str()) != pre_merge_files.get(path.as_str())
        })
        .cloned()
        .collect();

    if both_modified.is_empty() {
        return Ok(0);
    }

    let mut merged_count = 0usize;

    for path in &both_modified {
        let file_path = StdPath::new(path);

        // Check if a semantic driver exists for this file
        let Ok(driver) = registry.get_for_path(file_path) else {
            continue;
        };

        // Get the content versions for 3-way merge:
        // - base:  LCA commit's version (the true common ancestor)
        // - ours:  HEAD's version before this merge (pre_merge_files)
        // - theirs: source branch tip's version (from snapshot, NOT post-merge
        //           working tree, because the clean merge may have already
        //           overwritten ours' changes with theirs')
        let base = match lca_files.get(path.as_str()) {
            Some(c) => c.as_str(),
            None => "", // File didn't exist at LCA — treat as empty base
        };
        let ours = match pre_merge_files.get(path.as_str()) {
            Some(c) => c.as_str(),
            None => continue,
        };
        let theirs = match source_files.get(path.as_str()) {
            Some(c) => c.as_str(),
            None => continue,
        };

        // Skip if ours and theirs are identical (no actual change to merge)
        if ours == theirs {
            continue;
        }

        // Also skip if neither side changed from base
        if ours == base && theirs == base {
            continue;
        }

        // Try semantic 3-way merge with the YAML driver
        let merged_content = match driver.merge(base, ours, theirs) {
            Ok(Some(content)) => content,
            _ => manual_line_merge(ours, theirs),
        };

        // Only write if the semantic merge produced something different
        // from the post-merge working tree (i.e., it actually combined
        // both sides' changes instead of just returning one side).
        if merged_content.trim() == theirs.trim() {
            continue;
        }

        // Write the semantically merged content
        let full_path = repo.root().join(path);
        if let Err(_e) = std::fs::write(&full_path, &merged_content) {
            continue;
        }

        merged_count += 1;
    }

    Ok(merged_count)
}
