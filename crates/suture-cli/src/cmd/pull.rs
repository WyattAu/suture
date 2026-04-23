use crate::remote_proto::{do_fetch, do_pull};

pub(crate) async fn cmd_pull(remote: &str, rebase: bool, autostash: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    eprintln!("Pulling from {}...", remote);

    let had_changes = if autostash {
        let status = repo.status()?;
        let dirty = !status.staged_files.is_empty();
        if dirty {
            eprintln!("Auto-stashing uncommitted changes...");
            repo.stash_push(Some("auto-stash before pull"))?;
            true
        } else {
            false
        }
    } else {
        false
    };

    let pull_result: Result<(), Box<dyn std::error::Error>> = async {
        if rebase {
            let (head_branch, head_id) = repo.head()?;
            let current_branch = head_branch.clone();

            let new_patches = do_fetch(&mut repo, remote, None).await?;

            if new_patches == 0 {
                println!("Already up to date.");
                return Ok(());
            }

            let (_, new_head_id) = repo.head()?;
            if new_head_id == head_id {
                let result = repo.rebase("main")?;
                if result.patches_replayed == 0 && result.new_tip != head_id {
                    println!(
                        "Fast-forward pull successful ({} new patch(es))",
                        new_patches
                    );
                } else if result.patches_replayed > 0 {
                    println!(
                        "Pull with rebase successful: {} new remote patch(es), {} local patch(es) rebased",
                        new_patches, result.patches_replayed
                    );
                } else {
                    println!("Already up to date.");
                }
            } else {
                println!("Pull successful: {} new patch(es)", new_patches);
            }

            let (final_branch, _) = repo.head()?;
            if final_branch != current_branch {
                repo.checkout(&current_branch)?;
            }
        } else {
            let new_patches = do_pull(&mut repo, remote).await?;
            println!("Pull successful: {} new patch(es)", new_patches);
        }
        Ok(())
    }
    .await;

    if autostash && had_changes {
        if pull_result.is_ok() {
            eprintln!("Auto-stashing succeeded, popping stash...");
            if let Err(e) = repo.stash_pop() {
                eprintln!("warning: failed to pop stash: {e}");
                eprintln!("Run `suture stash pop` manually.");
            }
        } else {
            eprintln!("Pull failed. Your changes are safely stashed.");
            eprintln!("Run `suture stash pop` manually after resolving the issue.");
        }
    }

    pull_result
}
