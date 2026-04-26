use crate::cmd::user_error;

pub(crate) async fn cmd_checkout(
    branch: Option<&str>,
    new_branch: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;
    if let Some(name) = new_branch {
        let source = branch.filter(|b| *b != "HEAD");

        let existing: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
        if existing.iter().any(|b| b == name) {
            return Err(format!(
                "branch '{name}' already exists (use 'suture checkout {name}' to switch to it)"
            )
            .into());
        }

        repo.create_branch(name, source)
            .map_err(|e| user_error(&format!("failed to create branch '{name}'"), e))?;
        repo.checkout(name)
            .map_err(|e| user_error(&format!("failed to checkout branch '{name}'"), e))?;
        println!("Created and switched to branch '{}'", name);
    } else {
        let target = branch.ok_or("no branch specified (use -b to create one)")?;

        let branches: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
        if !branches.contains(&target.to_string()) && repo.resolve_ref(target).is_err() {
            let hint = if let Some(suggestion) = crate::fuzzy::suggest(target, &branches) {
                format!(" (did you mean '{}'?)", suggestion)
            } else {
                String::new()
            };
            return Err(format!(
                "branch '{target}' not found{hint} (use 'suture branch' to create it)"
            )
            .into());
        }

        if let Err(e) = repo.checkout(target) {
            return Err(user_error(&format!("failed to checkout '{target}'"), e));
        }
        if repo.is_detached() {
            if let Ok(Some(id)) = repo.get_detached_head() {
                let short = &id.to_hex()[..12];
                println!("Note: checking out '{}'.", short);
                println!(
                    "You are in 'detached HEAD' state. You can look around, make experimental"
                );
                println!(
                    "changes and commit them, and you can discard any commits you make in this"
                );
                println!("state without impacting any branches by switching back to a branch.");
            }
        } else {
            println!("Switched to branch '{}'", target);
        }
    }

    match crate::cmd::lfs::resolve_lfs_pointers_in_workdir() {
        Ok((resolved, missing)) => {
            if resolved > 0 {
                println!("Resolved {} LFS object(s)", resolved);
            }
            if missing > 0 {
                eprintln!(
                    "{} LFS object(s) not found locally (run `suture lfs pull`)",
                    missing
                );
            }
        }
        Err(e) => {
            eprintln!("warning: LFS pointer resolution failed: {e}");
        }
    }

    Ok(())
}
