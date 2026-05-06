use std::path::Path as StdPath;

pub async fn cmd_clean(
    dry_run: bool,
    dirs: bool,
    paths: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
    let status = repo.status()?;

    let head_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());

    let staged_paths: std::collections::HashSet<&str> = status
        .staged_files
        .iter()
        .map(|(p, _)| p.as_str())
        .collect();

    let repo_dir = StdPath::new(".");
    let disk_files = crate::display::walk_repo_files(repo_dir);

    let mut untracked: Vec<String> = Vec::new();

    for rel_path in &disk_files {
        if let Ok(data) = std::fs::read(repo_dir.join(rel_path)) {
            let current_hash = suture_common::Hash::from_data(&data);
            if let Some(head_hash) = head_tree.get(rel_path) {
                if &current_hash != head_hash {
                    continue;
                }
            } else if !staged_paths.contains(rel_path.as_str()) {
                untracked.push(rel_path.clone());
            }
        }
    }

    if !paths.is_empty() {
        untracked.retain(|f| paths.iter().any(|p| f.starts_with(p) || f == p));
    }

    if untracked.is_empty() {
        if dry_run {
            println!("Would remove 0 files");
        } else {
            println!("Nothing to clean");
        }
        return Ok(());
    }

    if dry_run {
        println!("Would remove:");
        for path in &untracked {
            println!("  {path}");
        }
        println!(
            "\nWould remove {} file{}",
            untracked.len(),
            if untracked.len() == 1 { "" } else { "s" }
        );
        return Ok(());
    }

    let mut removed = 0usize;
    for path in &untracked {
        if !crate::util::is_path_within_repo(repo_dir, StdPath::new(path)) {
            continue;
        }
        let full_path = repo_dir.join(path);
        if std::fs::remove_file(&full_path).is_ok() {
            removed += 1;
        }
    }

    if dirs {
        let mut dirs_removed = 0usize;
        remove_empty_dirs(repo_dir, repo_dir, &mut dirs_removed);
        if dirs_removed > 0 {
            println!("Removed {removed} files, {dirs_removed} directories");
        } else {
            println!(
                "Removed {} file{}",
                removed,
                if removed == 1 { "" } else { "s" }
            );
        }
    } else {
        println!(
            "Removed {} file{}",
            removed,
            if removed == 1 { "" } else { "s" }
        );
    }

    Ok(())
}

fn remove_empty_dirs(root: &StdPath, current: &StdPath, removed: &mut usize) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(std::result::Result::ok).collect();

    for entry in &entries {
        let path = entry.path();
        if path.is_dir() && path.file_name().is_none_or(|n| n != ".suture") {
            remove_empty_dirs(root, &path, removed);
        }
    }

    entries.retain(|e| e.path().is_dir() && e.path().file_name().is_none_or(|n| n != ".suture"));
    let is_empty = entries.iter().all(|e| {
        let Ok(inner) = std::fs::read_dir(e.path()) else {
            return true;
        };
        inner.count() == 0
    });

    if is_empty && current != root {
        if current.join(".suture").exists() {
            return;
        }
        let _ = std::fs::remove_dir(current);
        *removed += 1;
    }
}
