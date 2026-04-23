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

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

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

    let result = if dry_run {
        repo.preview_merge(source)?
    } else {
        repo.execute_merge(source)?
    };

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

        let (branch, head_id) = repo.head()?;
        let mut post_extra = std::collections::HashMap::new();
        post_extra.insert("SUTURE_BRANCH".to_string(), branch);
        post_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
        post_extra.insert("SUTURE_MERGE_SOURCE".to_string(), source.to_string());
        run_hook_if_exists(repo.root(), "post-merge", post_extra)?;

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
        let (ooxml, text): (Vec<_>, Vec<_>) = conflicts
            .into_iter()
            .partition(|c| is_ooxml_file(&c.path));

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
                    .and_then(|h| repo.cas().get_blob(&h).ok())
                    .map(|b| String::from_utf8_lossy(&b).to_string());
                let ours_content = conflict
                    .our_content_hash
                    .and_then(|h| repo.cas().get_blob(&h).ok())
                    .map(|b| String::from_utf8_lossy(&b).to_string());
                let theirs_content = conflict
                    .their_content_hash
                    .and_then(|h| repo.cas().get_blob(&h).ok())
                    .map(|b| String::from_utf8_lossy(&b).to_string());

                let base_str = base_content.as_deref().unwrap_or("");
                let Some(ours_str) = ours_content.as_deref() else {
                    remaining.push(conflict.clone());
                    continue;
                };
                let Some(theirs_str) = theirs_content.as_deref() else {
                    remaining.push(conflict.clone());
                    continue;
                };

                let Ok(merged) = driver.merge(base_str, ours_str, theirs_str) else {
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

    println!("Merge has {} conflict(s):", remaining.len());
    for conflict in &remaining {
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

    if remaining.is_empty() {
        let via = match merge_strategy {
            MergeStrategy::Semantic => "semantic drivers",
            MergeStrategy::Ours => "keeping our version",
            MergeStrategy::Theirs => "keeping their version",
            MergeStrategy::Manual => unreachable!(),
        };
        if ooxml_resolved > 0 && resolved_count > ooxml_resolved {
            println!(
                "All conflicts resolved. {} via {}, {} OOXML preserved.",
                resolved_count - ooxml_resolved, via, ooxml_resolved
            );
        } else if ooxml_resolved > 0 {
            println!(
                "All {} conflict(s) resolved. {} OOXML file(s) preserved (your version).",
                resolved_count, ooxml_resolved
            );
        } else {
            println!(
                "All conflicts resolved. {} via {}.",
                resolved_count, via
            );
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
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .is_some_and(|ext| matches!(ext.as_str(), "docx" | "xlsx" | "pptx"))
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
        report.push_str(
            "  Could not automatically merge. Your version has been preserved.\n\n",
        );

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
