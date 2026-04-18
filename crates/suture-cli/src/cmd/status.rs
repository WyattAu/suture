use crate::display::walk_repo_files;
use std::path::Path as StdPath;

pub(crate) async fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
    let status = repo.status()?;

    println!(
        "On branch {}",
        status.head_branch.as_deref().unwrap_or("detached")
    );
    if let Some(id) = status.head_patch {
        println!("HEAD: {}", id);
    }
    println!(
        "{} patches, {} branches",
        status.patch_count, status.branch_count
    );

    if !status.staged_files.is_empty() {
        println!("\nStaged changes:");
        for (path, file_status) in &status.staged_files {
            let icon = file_type_icon(path);
            println!("  {:?} {} {}", file_status, icon, path);
        }
    }

    let head_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());
    let staged_paths: std::collections::HashSet<&str> = status
        .staged_files
        .iter()
        .map(|(p, _)| p.as_str())
        .collect();

    let mut unstaged_modified: Vec<String> = Vec::new();
    let mut unstaged_deleted: Vec<String> = Vec::new();
    let mut untracked: Vec<String> = Vec::new();

    let repo_dir = StdPath::new(".");
    let disk_files = walk_repo_files(repo_dir);

    for rel_path in &disk_files {
        let full_path = repo_dir.join(rel_path);
        if let Ok(data) = std::fs::read(&full_path) {
            let current_hash = suture_common::Hash::from_data(&data);
            if let Some(head_hash) = head_tree.get(rel_path) {
                if &current_hash != head_hash {
                    unstaged_modified.push(rel_path.clone());
                }
            } else if !staged_paths.contains(rel_path.as_str()) {
                untracked.push(rel_path.clone());
            }
        }
    }

    for (path, _) in head_tree.iter() {
        if !disk_files.iter().any(|f| f == path) && !staged_paths.contains(path.as_str()) {
            unstaged_deleted.push(path.clone());
        }
    }

    if !unstaged_modified.is_empty() || !unstaged_deleted.is_empty() || !untracked.is_empty() {
        println!("\nUnstaged changes:");
        for path in &unstaged_modified {
            let icon = file_type_icon(path);
            let marker = if staged_paths.contains(path.as_str()) {
                " [staged+unstaged]"
            } else {
                ""
            };
            println!("  modified: {}{}{}", icon, path, marker);
        }
        for path in &unstaged_deleted {
            println!("  deleted:  {}", path);
        }
        for path in &untracked {
            let icon = file_type_icon(path);
            println!("  untracked: {}{}", icon, path);
        }
    }

    Ok(())
}

fn file_type_icon(path: &str) -> &'static str {
    suture_core::file_type::detect_file_type(StdPath::new(path)).icon()
}
