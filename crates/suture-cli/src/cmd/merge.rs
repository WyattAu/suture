use crate::style::run_hook_if_exists;

pub(crate) async fn cmd_merge(
    source: &str,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path as StdPath;
    use suture_core::repository::ConflictInfo;

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
            "DRY RUN: previewing merge of '{}' into current branch",
            source
        );
    } else {
        run_hook_if_exists(repo.root(), "pre-merge", pre_extra)?;
    }

    let result = repo.execute_merge(source)?;

    if result.is_clean {
        if dry_run {
            if result.patches_applied > 0 {
                if let Some(id) = &result.merge_patch_id {
                    println!(
                        "Merge would create: {} ({} patch(es) applied from '{}')",
                        id, result.patches_applied, source
                    );
                } else {
                    println!(
                        "Fast-forward merge: would apply {} patch(es) from '{}'",
                        result.patches_applied, source
                    );
                }
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

    let conflicts = result.unresolved_conflicts;
    let mut remaining: Vec<ConflictInfo> = Vec::new();
    let mut resolved_count = 0usize;

    {
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

            if dry_run {
                let driver_name = driver.name().to_lowercase();
                println!("Would resolve {} via {} driver", conflict.path, driver_name);
                resolved_count += 1;
                continue;
            }

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

    if dry_run {
        if resolved_count > 0 {
            println!(
                "Would resolve {} conflict(s) via semantic drivers",
                resolved_count
            );
        }
        if remaining.is_empty() {
            println!("All conflicts would be resolved via semantic drivers.");
        } else {
            println!("{} conflict(s) would remain unresolved:", remaining.len());
            for conflict in &remaining {
                println!(
                    "  CONFLICT in '{}': would need manual resolution",
                    conflict.path
                );
            }
        }
        println!("DRY RUN — no files were modified.");
        return Ok(());
    }

    if resolved_count > 0 {
        println!("Resolved {resolved_count} conflict(s) via semantic drivers");
    }

    if remaining.is_empty() {
        println!("All conflicts resolved via semantic drivers.");
        println!("Run `suture commit` to finalize the merge.");
    } else {
        println!("Merge has {} conflict(s):", remaining.len());
        for conflict in &remaining {
            let ours_preview = conflict
                .our_content_hash
                .as_ref()
                .and_then(|h| repo.cas().get_blob(h).ok())
                .map(|b| String::from_utf8_lossy(&b).to_string());
            let theirs_preview = conflict
                .their_content_hash
                .as_ref()
                .and_then(|h| repo.cas().get_blob(h).ok())
                .map(|b| String::from_utf8_lossy(&b).to_string());

            println!("  CONFLICT in '{}':", conflict.path);

            if let Some(ref ours) = ours_preview {
                let lines: Vec<&str> = ours.lines().take(5).collect();
                println!("    ours:");
                for line in &lines {
                    println!("      {}", line);
                }
                if ours.lines().count() > 5 {
                    println!("      ...");
                }
            }

            if let Some(ref theirs) = theirs_preview {
                let lines: Vec<&str> = theirs.lines().take(5).collect();
                println!("    theirs:");
                for line in &lines {
                    println!("      {}", line);
                }
                if theirs.lines().count() > 5 {
                    println!("      ...");
                }
            }

            println!("    Edit the file, then run `suture commit` to resolve");
        }
        println!("Hint: resolve conflicts, then run `suture commit`");
    }

    Ok(())
}
