use crate::display::format_timestamp;
use crate::ref_utils::resolve_ref;
pub(crate) async fn cmd_show(commit_ref: &str, stat: bool) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit_ref, &patches)?;
    let target = target.clone();

    println!("commit {}", target.id.to_hex());
    println!("Author: {}", target.author);
    println!("Date:    {}", format_timestamp(target.timestamp));
    println!();
    println!("    {}", target.message);

    if !target.payload.is_empty()
        && let Some(path) = &target.target_path
    {
        println!("\n  {} {}", target.operation_type, path);
    }

    if !target.parent_ids.is_empty() {
        print!("\nParents:");
        for pid in &target.parent_ids {
            print!(" {}", pid);
        }
        println!();
    }

    if stat {
        print_stat(&repo, &target)?;
    }

    Ok(())
}

fn print_stat(
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
    let mut file_list: Vec<(&str, &str)> = Vec::new();

    for file in &files {
        let in_parent = parent_tree.as_ref().is_some_and(|t| t.contains(file));
        let in_commit = commit_tree.as_ref().is_some_and(|t| t.contains(file));

        let (label, icon) = if !in_parent && in_commit {
            added += 1;
            ("added", "\u{1F4C4}")
        } else if in_parent && !in_commit {
            deleted += 1;
            ("deleted", "\u{1F5D1}")
        } else {
            modified += 1;
            ("modified", "\u{1F4C4}")
        };
        file_list.push((file, icon));

        let _ = label;
    }

    println!();
    println!(
        "{} file{} changed, {} added, {} modified, {} deleted",
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        added,
        modified,
        deleted,
    );

    for (file, icon) in &file_list {
        println!("{} {} ({})", icon, file, classify_file(file, &parent_tree, &commit_tree));
    }

    Ok(())
}

fn classify_file(
    file: &str,
    parent_tree: &Option<suture_core::engine::tree::FileTree>,
    commit_tree: &Option<suture_core::engine::tree::FileTree>,
) -> &'static str {
    let in_parent = parent_tree.as_ref().is_some_and(|t| t.contains(file));
    let in_commit = commit_tree.as_ref().is_some_and(|t| t.contains(file));

    if !in_parent && in_commit {
        "added"
    } else if in_parent && !in_commit {
        "deleted"
    } else {
        "modified"
    }
}
