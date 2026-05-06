use crate::BisectAction;
use crate::ref_utils::resolve_ref;

pub async fn cmd_bisect(action: &crate::BisectAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        BisectAction::Start {
            good: good_ref,
            bad: bad_ref,
        } => {
            let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
            let all_patches = repo.all_patches();

            let good_patch = resolve_ref(&repo, good_ref, &all_patches)?;
            let bad_patch = resolve_ref(&repo, bad_ref, &all_patches)?;

            let log = repo.log(None)?;

            let good_idx = log
                .iter()
                .position(|p| p.id == good_patch.id)
                .ok_or_else(|| format!("'{good_ref}' not found in history"))?;
            let bad_idx = log
                .iter()
                .position(|p| p.id == bad_patch.id)
                .ok_or_else(|| format!("'{bad_ref}' not found in history"))?;

            let bad_ancestors = repo.dag().ancestors(&bad_patch.id);
            if !bad_ancestors.contains(&good_patch.id) && good_patch.id != bad_patch.id {
                return Err("'good' must be an ancestor of 'bad'".into());
            }

            // log returns newest first, so higher index = older commit
            let (older_idx, newer_idx) = if good_idx > bad_idx {
                (good_idx, bad_idx) // good is older (higher idx), bad is newer (lower idx)
            } else {
                (bad_idx, good_idx) // bad is older, good is newer
            };

            let remaining = older_idx - newer_idx - 1;
            if remaining == 0 {
                println!("Only one commit between good and bad:");
                println!(
                    "  {} {}",
                    &log[newer_idx + 1].id.to_hex()[..8],
                    log[newer_idx + 1].message.lines().next().unwrap_or("")
                );
                println!("  This is the first bad commit.");
                return Ok(());
            }

            let midpoint_idx = usize::midpoint(older_idx, newer_idx);
            let midpoint = &log[midpoint_idx];

            println!(
                "Bisecting: {} commit(s) remaining between good ({}) and bad ({})",
                remaining,
                &good_patch.id.to_hex()[..8],
                &bad_patch.id.to_hex()[..8]
            );
            println!();
            println!("  Step: test commit {}", midpoint.id.to_hex());
            println!("  {}", midpoint.message.lines().next().unwrap_or(""));
            println!();
            println!("To test this commit:");
            println!("  suture reset {} --hard", midpoint.id.to_hex());
            println!();
            println!("Then mark as:");
            if midpoint_idx > newer_idx + 1 {
                println!(
                    "  suture bisect start {} {}   (if this commit is GOOD)",
                    good_ref,
                    &midpoint.id.to_hex()[..8]
                );
            } else {
                println!("  First bad commit found: {}", midpoint.id.to_hex());
            }
            if midpoint_idx < older_idx - 1 {
                println!(
                    "  suture bisect start {} {}   (if this commit is BAD)",
                    &midpoint.id.to_hex()[..8],
                    bad_ref
                );
            } else {
                println!("  First bad commit found: {}", midpoint.id.to_hex());
            }
        }
        BisectAction::Reset => {
            let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
            let (branch_name, _) = repo.head().map_err(|e| e.to_string())?;
            println!("Bisect reset. You are on branch '{branch_name}'.");
        }
        BisectAction::Run {
            good: good_ref,
            bad: bad_ref,
            cmd,
        } => {
            if cmd.is_empty() {
                return Err("bisect run requires a command to execute".into());
            }

            let repo_path = std::path::Path::new(".");

            // Save the branch name for restoration
            let (original_branch, original_head) = {
                let repo = suture_core::repository::Repository::open(repo_path)?;
                repo.head()?
            };

            // Resolve refs and get the full ordered log BEFORE any modifications
            // Use patch_chain (first-parent) for deterministic ordering
            let (ordered_log, good_idx, bad_idx) = {
                let repo = suture_core::repository::Repository::open(repo_path)?;
                let all_patches = repo.all_patches();

                let good_patch = resolve_ref(&repo, good_ref, &all_patches)?;
                let bad_patch = resolve_ref(&repo, bad_ref, &all_patches)?;

                // Use log (first-parent chain) for deterministic ordering by ancestry
                let log = repo.log(None)?;

                let good_idx = log
                    .iter()
                    .position(|p| p.id == good_patch.id)
                    .ok_or_else(|| format!("'{good_ref}' not found in history"))?;
                let bad_idx = log
                    .iter()
                    .position(|p| p.id == bad_patch.id)
                    .ok_or_else(|| format!("'{bad_ref}' not found in history"))?;

                // Verify ancestry
                let bad_ancestors = repo.dag().ancestors(&bad_patch.id);
                if !bad_ancestors.contains(&good_patch.id) && good_patch.id != bad_patch.id {
                    return Err("'good' must be an ancestor of 'bad'".into());
                }

                (log, good_idx, bad_idx)
            };

            // Determine older/newer indices (log is newest first, so higher index = older)
            let (older_idx, newer_idx) = if good_idx > bad_idx {
                (good_idx, bad_idx) // good is older (higher idx), bad is newer (lower idx)
            } else {
                (bad_idx, good_idx) // bad is older, good is newer
            };

            // Extract the program and arguments
            let program = &cmd[0];
            let args = &cmd[1..];

            println!(
                "bisect run '{}' with good={} bad={}",
                cmd.join(" "),
                &ordered_log[older_idx].id.to_hex()[..8],
                &ordered_log[newer_idx].id.to_hex()[..8]
            );
            println!();

            let mut current_good = older_idx;
            let mut current_bad = newer_idx;
            let mut step = 0u32;

            loop {
                step += 1;
                // current_good > current_bad (higher index = older commit)
                let remaining = current_good.saturating_sub(current_bad + 1);

                if remaining == 0 {
                    // Only one commit between good and bad — that's the first bad commit
                    // The first bad is one step newer than the last known good
                    let first_bad = &ordered_log[current_good - 1];
                    println!("First bad commit found after {step} step(s):");
                    println!(
                        "  {} {}",
                        first_bad.id.to_hex(),
                        first_bad.message.lines().next().unwrap_or("(no message)")
                    );
                    break;
                }

                let midpoint_idx = usize::midpoint(current_good, current_bad);
                let midpoint = &ordered_log[midpoint_idx];

                // Reset to the midpoint commit
                {
                    let mut repo = suture_core::repository::Repository::open(repo_path)?;
                    repo.reset(
                        &midpoint.id.to_hex(),
                        suture_core::repository::ResetMode::Hard,
                    )?;
                }

                println!(
                    "[step {}] Testing {} ({} remaining)...",
                    step,
                    &midpoint.id.to_hex()[..8],
                    remaining
                );

                // Run the test command
                let result = std::process::Command::new(program)
                    .args(args)
                    .current_dir(repo_path)
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .status();

                match result {
                    Ok(status) => {
                        let is_good = status.success();
                        if is_good {
                            println!("  -> GOOD (exit 0)");
                            // Midpoint is good; bad commit must be newer (lower index)
                            current_good = midpoint_idx - 1;
                        } else {
                            let code = status.code().unwrap_or(1);
                            println!("  -> BAD (exit {code})");
                            // Midpoint is bad; good commit must be older (higher index)
                            current_bad = midpoint_idx + 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("  -> Command failed to execute: {e}");
                        eprintln!("  Aborting bisect run.");
                        break;
                    }
                }
                println!();
            }

            // Restore the original branch to its original state
            let mut repo = suture_core::repository::Repository::open(repo_path)?;
            repo.reset(
                &original_head.to_hex(),
                suture_core::repository::ResetMode::Hard,
            )?;
            if let Err(e) = repo.checkout(&original_branch) {
                eprintln!(
                    "suture: warning: failed to restore branch '{}': {e}",
                    original_branch
                );
            }
        }
    }

    Ok(())
}
