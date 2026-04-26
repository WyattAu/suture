pub(crate) async fn cmd_worktree(
    action: &crate::WorktreeAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        crate::WorktreeAction::Add { path, branch, b } => {
            let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
            if repo.is_worktree() {
                return Err("cannot add worktree from a linked worktree; use the main repo".into());
            }

            let wt_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("worktree");

            let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

            if let Some(new_branch) = b {
                repo.create_branch(new_branch, branch.as_deref())?;
                repo.add_worktree(wt_name, std::path::Path::new(path), Some(new_branch))?;
            } else {
                repo.add_worktree(wt_name, std::path::Path::new(path), branch.as_deref())?;
            }

            println!("Worktree '{}' created at {}", wt_name, path);
            Ok(())
        }
        crate::WorktreeAction::List => {
            let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
            let worktrees = repo.list_worktrees()?;
            for wt in &worktrees {
                let kind = if wt.is_main { "main" } else { &wt.name };
                println!("{:<20} {:<40} {}", kind, wt.path, wt.branch);
            }
            Ok(())
        }
        crate::WorktreeAction::Remove { name } => {
            let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
            repo.remove_worktree(name)?;
            println!("Worktree '{}' removed", name);
            Ok(())
        }
        crate::WorktreeAction::Prune => {
            let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
            let worktrees = repo.list_worktrees()?;
            let mut pruned = 0usize;

            for wt in &worktrees {
                if wt.is_main {
                    continue;
                }
                let wt_path = std::path::Path::new(&wt.path);
                if !wt_path.exists() {
                    let mut repo =
                        suture_core::repository::Repository::open(std::path::Path::new("."))?;
                    repo.remove_worktree(&wt.name)?;
                    pruned += 1;
                }
            }

            if pruned > 0 {
                println!("Pruned {} stale worktree entries", pruned);
            } else {
                println!("No stale worktree entries found.");
            }
            Ok(())
        }
    }
}
