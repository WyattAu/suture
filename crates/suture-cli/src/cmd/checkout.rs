use crate::cmd::user_error;
use crate::cmd::lfs::{is_lfs_pointer, parse_lfs_pointer, read_lfs_object};

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
            return Err(format!("branch '{name}' already exists (use 'suture checkout {name}' to switch to it)").into());
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
            return Err(format!("branch '{target}' not found{hint} (use 'suture branch' to create it)").into());
        }

        if let Err(e) = repo.checkout(target) {
            return Err(user_error(&format!("failed to checkout '{target}'"), e));
        }
        if repo.is_detached() {
            if let Ok(Some(id)) = repo.get_detached_head() {
                let short = &id.to_hex()[..12];
                println!("Note: checking out '{}'.", short);
                println!("You are in 'detached HEAD' state. You can look around, make experimental");
                println!("changes and commit them, and you can discard any commits you make in this");
                println!("state without impacting any branches by switching back to a branch.");
            }
        } else {
            println!("Switched to branch '{}'", target);
        }
    }

    resolve_lfs_pointers();

    Ok(())
}

fn resolve_lfs_pointers() {
    let repo_root = std::path::Path::new(".");
    let tree = match suture_core::repository::Repository::open(repo_root)
        .ok()
        .and_then(|r| r.snapshot_head().ok())
    {
        Some(t) => t,
        None => return,
    };

    for (path, _hash) in tree.iter() {
        let full_path = repo_root.join(path);
        if !full_path.exists() {
            continue;
        }
        let blob = match std::fs::read(&full_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if !is_lfs_pointer(&blob) {
            continue;
        }
        let text = match std::str::from_utf8(&blob) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let pointer = match parse_lfs_pointer(text) {
            Some(p) => p,
            None => continue,
        };
        match read_lfs_object(repo_root, &pointer.oid) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&full_path, &data) {
                    eprintln!("warning: failed to restore LFS object for '{}': {}", path, e);
                }
            }
            Err(_) => {
                eprintln!("warning: LFS object not found for '{}'. Run 'suture lfs pull'.", path);
            }
        }
    }
}
