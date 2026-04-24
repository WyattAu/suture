//! `suture doctor` — Check repository health and configuration.
//!
//! Runs a series of diagnostic checks and reports the overall health
//! of the repository, similar to `git doctor` or `cargo doctor`.

pub(crate) async fn cmd_doctor(fix: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;
    use suture_common::FileStatus;

    let root = Path::new(".");

    // Check 1: Are we in a Suture repository?
    let suture_dir = root.join(".suture");
    if !suture_dir.exists() {
        println!("✗ Not a Suture repository (no .suture/ directory)");
        println!("  Run 'suture init .' to create one");
        return Ok(());
    }
    println!("✓ Suture repository detected");

    // Check 2: Can we open the repository?
    let mut repo = match suture_core::repository::Repository::open(root) {
        Ok(r) => {
            println!("✓ Repository opened successfully");
            r
        }
        Err(e) => {
            println!("✗ Failed to open repository: {}", e);
            return Ok(());
        }
    };

    let mut issues = 0usize;
    let mut warnings = 0usize;
    let mut fixed = 0usize;
    let mut remaining = 0usize;

    // Check 3: Is user configured?
    let author = repo.author().to_string();
    if author.is_empty() || author == "unknown" {
        if fix {
            let name = std::env::var("GIT_AUTHOR_NAME")
                .or_else(|_| std::env::var("GIT_COMMITTER_NAME"))
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_else(|_| "Suture User".to_string());
            let email = std::env::var("GIT_AUTHOR_EMAIL")
                .or_else(|_| std::env::var("GIT_COMMITTER_EMAIL"))
                .unwrap_or_else(|_| "user@example.com".to_string());

            let name_default = name == "Suture User";
            let email_default = email == "user@example.com";

            repo.set_config("user.name", &name)?;
            repo.set_config("user.email", &email)?;
            if name_default || email_default {
                println!(
                    "✓ Set user config to defaults (name='{}', email='{}')",
                    name, email
                );
                println!(
                    "  Update with: suture config user.name \"Your Name\""
                );
            } else {
                println!("✓ Set user config (name='{}', email='{}')", name, email);
            }
            fixed += 1;
        } else {
            println!("✗ No user configured (run 'suture config user.name <name>')");
            issues += 1;
            remaining += 1;
        }
    } else {
        println!("✓ User configured: {}", author);
    }

    // Check 4: Is there a HEAD?
    match repo.head() {
        Ok((branch, id)) => {
            if id == suture_common::Hash::ZERO {
                println!("✓ Empty repository (no commits yet) on branch '{}'", branch);
            } else {
                println!(
                    "✓ HEAD: {} at {}",
                    branch,
                    &id.to_hex()[..8]
                );
            }
        }
        Err(e) => {
            if fix {
                repo.invalidate_head_cache();
                match repo.head() {
                    Ok((branch, id)) => {
                        if id == suture_common::Hash::ZERO {
                            println!(
                                "✓ HEAD cache invalidated — empty repository on branch '{}'",
                                branch
                            );
                        } else {
                            println!(
                                "✓ HEAD cache invalidated — HEAD: {} at {}",
                                branch,
                                &id.to_hex()[..8]
                            );
                        }
                        fixed += 1;
                    }
                    Err(e2) => {
                        println!("✗ HEAD is corrupted (cache invalidation did not help): {}", e2);
                        issues += 1;
                        remaining += 1;
                    }
                }
            } else {
                println!("✗ HEAD is corrupted: {}", e);
                issues += 1;
                remaining += 1;
            }
        }
    }

    // Check 5: Branch count
    let branches = repo.list_branches();
    let branch_count = branches.len();
    if branch_count == 0 {
        println!("⚠ No branches found");
        warnings += 1;
    } else {
        println!("✓ {} branch(es)", branch_count);
    }

    // Check 6: Branch protection — if 'main' exists, is it protected?
    let has_main = branches.iter().any(|(n, _)| n == "main");
    if has_main {
        let main_protected = repo
            .get_config("branch.main.protected")?
            .is_some_and(|v| v == "true");
        if !main_protected {
            if fix {
                repo.set_config("branch.main.protected", "true")?;
                println!("✓ Protected 'main' branch");
                fixed += 1;
            } else {
                println!("⚠ 'main' branch is not protected");
                println!("  Run 'suture branch --protect main'");
                warnings += 1;
            }
        } else {
            println!("✓ 'main' branch is protected");
        }
    }

    // Check 7: Working set status
    let working_set = repo.meta().working_set().unwrap_or_default();
    let staged: Vec<_> = working_set
        .iter()
        .filter(|(_, s)| {
            matches!(s, FileStatus::Added | FileStatus::Modified | FileStatus::Deleted)
        })
        .collect();
    if staged.is_empty() {
        println!("✓ Clean working tree");
    } else {
        println!("⚠ {} staged change(s)", staged.len());
        warnings += 1;
    }

    // Check 8: Reflog
    match repo.reflog_entries() {
        Ok(entries) => {
            println!("✓ Reflog: {} entries", entries.len());
        }
        Err(e) => {
            println!("⚠ Reflog error: {}", e);
            warnings += 1;
        }
    }

    // Check 9: fsck (lightweight integrity)
    match repo.fsck() {
        Ok(result) => {
            if result.errors.is_empty() {
                println!(
                    "✓ Integrity check passed ({} check(s))",
                    result.checks_passed
                );
            } else {
                println!("✗ Integrity check found {} error(s):", result.errors.len());
                for e in &result.errors {
                    println!("  ERROR: {}", e);
                }
                issues += result.errors.len();
                remaining += result.errors.len();
            }
            for w in &result.warnings {
                println!("⚠ fsck warning: {}", w);
                warnings += 1;
            }
        }
        Err(e) => {
            println!("⚠ Could not run integrity check: {}", e);
            warnings += 1;
        }
    }

    // Check 10: DAG health
    let dag = repo.dag();
    let patch_count = dag.patch_count();
    println!("✓ DAG: {} patch(es)", patch_count);

    // Check 11: .sutureignore exists?
    let ignore_path = root.join(".sutureignore");
    if ignore_path.exists() {
        println!("✓ .sutureignore present");
    } else if fix {
        const DEFAULT_SUTUREIGNORE: &str = "\
*.tmp
~$*
.DS_Store
Thumbs.db
__pycache__/
node_modules/
target/
";
        std::fs::write(&ignore_path, DEFAULT_SUTUREIGNORE)?;
        println!("✓ Created .sutureignore with default patterns");
        fixed += 1;
    } else {
        println!("ℹ No .sutureignore (optional)");
        println!("  Run 'suture doctor --fix' to create one");
    }

    // Check 12: Orphaned objects — suggest GC
    if patch_count > 0 {
        let all_ids: std::collections::HashSet<_> = dag.patch_ids().into_iter().collect();
        let mut reachable: std::collections::HashSet<_> = std::collections::HashSet::new();
        for (_name, tip_id) in &branches {
            reachable.insert(*tip_id);
            for anc in dag.ancestors(tip_id).iter() {
                reachable.insert(*anc);
            }
        }
        let orphan_count = all_ids.iter().filter(|id| !reachable.contains(id)).count();
        if orphan_count > 0 {
            if fix {
                match repo.gc() {
                    Ok(result) => {
                        println!(
                            "✓ Garbage collected {} patch(es) and {} blob(s)",
                            result.patches_removed, result.blobs_removed
                        );
                        fixed += 1;
                    }
                    Err(e) => {
                        println!("✗ Garbage collection failed: {}", e);
                        issues += 1;
                        remaining += 1;
                    }
                }
            } else {
                println!("⚠ {} unreachable patch(es) — run 'suture gc' or 'suture doctor --fix'", orphan_count);
                warnings += 1;
            }
        }
    }

    // Check 13: Audit chain compliance
    println!();
    println!("── Compliance ──");
    let audit_path = root.join(".suture").join("audit").join("chain.log");
    if audit_path.exists() {
        println!("✓ Audit chain exists");
        let audit = suture_core::audit::AuditLog::open(&audit_path).unwrap();
        match audit.verify_chain() {
            Ok((total, first_invalid)) => match first_invalid {
                None => println!("✓ Audit chain valid ({} entries)", total),
                Some(i) => {
                    println!("✗ Audit chain TAMPERED at entry {}", i);
                    issues += 1;
                    remaining += 1;
                }
            },
            Err(e) => {
                println!("✗ Audit chain verification failed: {}", e);
                issues += 1;
                remaining += 1;
            }
        }
    } else {
        println!("⚠ No audit chain (run commits to create one)");
        warnings += 1;
    }

    match repo.get_config("signing.key") {
        Ok(Some(key_name)) => {
            println!("✓ Signing key configured: {}", key_name);
        }
        _ => {
            println!("⚠ No signing.key configured (non-repudiation disabled)");
            warnings += 1;
        }
    }

    // Summary
    println!();
    if issues == 0 && warnings == 0 {
        println!("Repository is healthy. No issues found.");
    } else if issues == 0 {
        println!("Repository is functional with {} warning(s).", warnings);
    } else {
        println!(
            "Repository has {} issue(s) and {} warning(s).",
            issues, warnings
        );
    }
    if fix && (fixed > 0 || remaining > 0) {
        println!();
        println!("{} issue(s) fixed, {} remaining.", fixed, remaining);
    }

    Ok(())
}
