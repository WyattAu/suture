use crate::StashAction;

pub(crate) async fn cmd_stash(
    action: &crate::StashAction,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    match action {
        StashAction::Push { message } => {
            let idx = repo.stash_push(message.as_deref())?;
            println!("Saved as stash@{{{}}}", idx);
        }
        StashAction::Pop => {
            let stashes_before = repo.stash_list()?;
            if stashes_before.is_empty() {
                println!("No stashes to pop.");
            } else {
                let highest = stashes_before.iter().map(|s| s.index).max().unwrap_or(0);
                repo.stash_pop()?;
                let message = stashes_before
                    .iter()
                    .find(|s| s.index == highest)
                    .map(|s| s.message.as_str())
                    .unwrap_or("unknown");
                println!("Restored stash@{{{}}}: {}", highest, message);
            }
        }
        StashAction::Apply { index } => {
            repo.stash_apply(*index)?;
            println!("Applied stash@{{{}}}", index);
        }
        StashAction::List => {
            let stashes = repo.stash_list()?;
            if stashes.is_empty() {
                println!("No stashes found.");
            } else {
                for s in &stashes {
                    println!("stash@{{{}}}: {} ({})", s.index, s.message, s.branch);
                }
            }
        }
        StashAction::Drop { index } => {
            repo.stash_drop(*index)?;
            println!("Dropped stash@{{{}}}", index);
        }
    }
    Ok(())
}
