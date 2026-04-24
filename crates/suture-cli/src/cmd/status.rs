use crate::display::walk_repo_files;
use std::collections::HashSet;
use std::path::Path as StdPath;

pub(crate) async fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
    let status = repo.status()?;

    if let Some(ref branch_name) = status.head_branch {
        println!("On branch {}", branch_name);
    } else if let Some(ref id) = status.head_patch {
        println!("HEAD detached at {}", &id.to_hex()[..12]);
    }
    if let Some(id) = status.head_patch {
        println!("HEAD: {}", id);
    }

    if let Some(ref branch_name) = status.head_branch {
        if let Ok(Some(remote_ref)) = find_remote_ref(&repo, branch_name) {
            if let Some(id) = status.head_patch {
                let (ahead, behind) = compute_ahead_behind(&repo, &id, &remote_ref.tip);
                match (ahead, behind) {
                    (0, 0) => {
                        println!(
                            "Your branch is up to date with '{}'.",
                            remote_ref.label
                        );
                    }
                    (a, 0) => {
                        println!(
                            "Your branch is ahead of '{}' by {} commit{}.",
                            remote_ref.label,
                            a,
                            if a == 1 { "" } else { "s" }
                        );
                    }
                    (0, b) => {
                        println!(
                            "Your branch is behind '{}' by {} commit{}.",
                            remote_ref.label,
                            b,
                            if b == 1 { "" } else { "s" }
                        );
                    }
                    (a, b) => {
                        println!(
                            "Your branch and '{}' have diverged, and have {} and {} different commit{} each, respectively.",
                            remote_ref.label,
                            a,
                            b,
                            if a + b == 2 { "" } else { "s" }
                        );
                    }
                }
            }
        } else {
            let remotes = repo.list_remotes().unwrap_or_default();
            if !remotes.is_empty() {
                let names: Vec<&str> = remotes.iter().map(|(n, _)| n.as_str()).collect();
                println!(
                    "Connected to remote{} {}. Use `suture fetch` to check for updates.",
                    if names.len() == 1 { "" } else { "s" },
                    names.join(", "),
                );
            }
        }
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

struct RemoteRef {
    label: String,
    tip: suture_common::Hash,
}

fn find_remote_ref(
    repo: &suture_core::repository::Repository,
    branch_name: &str,
) -> Result<Option<RemoteRef>, Box<dyn std::error::Error>> {
    let remotes = repo.list_remotes().unwrap_or_default();
    for (remote_name, _url) in &remotes {
        let ref_key = format!("remote.{}.ref.{}", remote_name, branch_name);
        if let Ok(Some(hex)) = repo.get_config(&ref_key) && let Ok(tip) = suture_common::Hash::from_hex(&hex) {
            return Ok(Some(RemoteRef {
                label: format!("{}/{}", remote_name, branch_name),
                tip,
            }));
        }
    }
    Ok(None)
}

fn compute_ahead_behind(
    repo: &suture_core::repository::Repository,
    local_head: &suture_common::Hash,
    remote_tip: &suture_common::Hash,
) -> (usize, usize) {
    if local_head == remote_tip {
        return (0, 0);
    }

    let local_ancestors = repo.dag().ancestors(local_head);
    let remote_ancestors = repo.dag().ancestors(remote_tip);

    let local_reachable: HashSet<_> = (*local_ancestors)
        .iter()
        .copied()
        .chain(std::iter::once(*local_head))
        .collect();
    let remote_reachable: HashSet<_> = (*remote_ancestors)
        .iter()
        .copied()
        .chain(std::iter::once(*remote_tip))
        .collect();

    let ahead = local_reachable.difference(&remote_reachable).count();
    let behind = remote_reachable.difference(&local_reachable).count();

    (ahead, behind)
}
