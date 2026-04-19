//! `suture doctor` — Check repository health and configuration.
//!
//! Runs a series of diagnostic checks and reports the overall health
//! of the repository, similar to `git doctor` or `cargo doctor`.

pub(crate) async fn cmd_doctor() -> Result<(), Box<dyn std::error::Error>> {
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
    let repo = match suture_core::repository::Repository::open(root) {
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

    // Check 3: Is user configured?
    let author = repo.author().to_string();
    if author.is_empty() || author == "unknown" {
        println!("✗ No user configured (run 'suture config user.name <name>')");
        issues += 1;
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
            println!("✗ HEAD is corrupted: {}", e);
            issues += 1;
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

    // Check 6: Working set status
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

    // Check 7: Reflog
    match repo.reflog_entries() {
        Ok(entries) => {
            println!("✓ Reflog: {} entries", entries.len());
        }
        Err(e) => {
            println!("⚠ Reflog error: {}", e);
            warnings += 1;
        }
    }

    // Check 8: fsck (lightweight integrity)
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

    // Check 9: DAG health
    let dag = repo.dag();
    let patch_count = dag.patch_count();
    println!("✓ DAG: {} patch(es)", patch_count);

    // Check 10: .sutureignore exists?
    let ignore_path = root.join(".sutureignore");
    if ignore_path.exists() {
        println!("✓ .sutureignore present");
    } else {
        println!("ℹ No .sutureignore (optional)");
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

    Ok(())
}
