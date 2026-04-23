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
        StashAction::Branch { name, index } => {
            let stashes = repo.stash_list()?;
            let entry = stashes
                .iter()
                .find(|s| s.index == *index)
                .ok_or_else(|| format!("stash@{{{}}} not found", index))?;
            repo.create_branch(name, if entry.head_id.is_empty() { None } else { Some(&entry.head_id) })?;
            repo.checkout(name)?;
            repo.stash_apply(*index)?;
            println!(
                "Created branch '{}' from stash@{{{}}}: {}",
                name, index, entry.message
            );
        }
        StashAction::Show { index } => {
            stash_show(&repo, *index)?;
        }
    }
    Ok(())
}

fn stash_show(
    repo: &suture_core::repository::Repository,
    index: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let stashes = repo.stash_list()?;
    let entry = stashes
        .iter()
        .find(|s| s.index == index)
        .ok_or_else(|| format!("error: stash@{{{}}} not found", index))?;

    let patches = repo.all_patches();
    let stash_patch = if entry.head_id.is_empty() {
        let (_branch_name, head_id) = repo.head().map_err(|e| e.to_string())?;
        patches.iter().find(|p| p.id == head_id)
    } else {
        let parent_id = suture_common::Hash::from_hex(&entry.head_id)?;
        patches.iter().find(|p| p.id == parent_id)
    };

    if entry.branch.is_empty() || entry.branch == "(no branch)" {
        println!("stash@{{{}}}: {}", index, entry.message);
    } else {
        println!("On {}: {}", entry.branch, entry.message);
    }

    if let Some(patch) = stash_patch {
        print_stash_stat(repo, patch)?;
    }

    Ok(())
}

fn print_stash_stat(
    repo: &suture_core::repository::Repository,
    patch: &suture_core::patch::types::Patch,
) -> Result<(), Box<dyn std::error::Error>> {
    let files: Vec<String> = patch.touch_set.addresses();
    if files.is_empty() {
        return Ok(());
    }

    let parent_tree = if !patch.parent_ids.is_empty() {
        repo.snapshot(&patch.parent_ids[0]).ok()
    } else {
        None
    };
    let commit_tree = repo.snapshot(&patch.id).ok();

    let mut added = 0usize;
    let mut modified = 0usize;
    let mut deleted = 0usize;

    for file in &files {
        let in_parent = parent_tree.as_ref().is_some_and(|t| t.contains(file));
        let in_commit = commit_tree.as_ref().is_some_and(|t| t.contains(file));

        if !in_parent && in_commit {
            added += 1;
        } else if in_parent && !in_commit {
            deleted += 1;
        } else {
            modified += 1;
        }
    }

    println!(
        " {} file{} changed, {} added, {} modified, {} deleted",
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        added,
        modified,
        deleted,
    );

    Ok(())
}
