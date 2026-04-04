use crate::remote_proto::{do_fetch, do_pull};

pub(crate) async fn cmd_pull(remote: &str, rebase: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if rebase {
        // Save current branch for later
        let (head_branch, head_id) = repo.head()?;
        let current_branch = head_branch.clone();

        // Fetch new patches from remote (no working tree update)
        let new_patches = do_fetch(&mut repo, remote, None).await?;

        if new_patches == 0 {
            println!("Already up to date.");
            return Ok(());
        }

        // Rebase current branch onto main (which now has remote patches)
        let (_, new_head_id) = repo.head()?;
        if new_head_id == head_id {
            // Fetch didn't move our branch — rebase onto main
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

        // Ensure we're on the correct branch
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
