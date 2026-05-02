use std::path::Path as StdPath;

pub async fn cmd_restore(
    source: Option<&str>,
    paths: &[String],
    staged: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if paths.is_empty() {
        return Err("error: you must specify path(s) to restore".into());
    }

    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;

    if staged {
        let head_tree = repo.snapshot_head()?;
        for path in paths {
            let repo_path = suture_common::RepoPath::new(path)?;
            repo.meta().working_set_remove(&repo_path)?;
            if head_tree.contains(path) {
                repo.meta()
                    .working_set_add(&repo_path, suture_common::FileStatus::Modified)?;
            }
            println!("Unstaged {path}");
        }
    } else {
        let source_ref = source.unwrap_or("HEAD");
        let patch_id = resolve_ref(&repo, source_ref)?;
        let source_tree = repo.snapshot(&patch_id)?;

        for path in paths {
            if let Some(hash) = source_tree.get(path) {
                let data = repo.cas().get_blob(hash)?;
                let full_path = repo.root().join(path);
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&full_path, data)?;
                println!("Restored {path}");
            } else {
                let full_path = repo.root().join(path);
                if full_path.exists() {
                    std::fs::remove_file(&full_path)?;
                    println!("Removed {path}");
                }
            }
        }
    }

    Ok(())
}

fn resolve_ref(
    repo: &suture_core::repository::Repository,
    target: &str,
) -> Result<suture_common::Hash, Box<dyn std::error::Error>> {
    use suture_common::Hash;

    if target == "HEAD" {
        let (_, id) = repo.head()?;
        return Ok(id);
    }

    if let Some(rest) = target.strip_prefix("HEAD~") {
        let n: usize = rest
            .parse()
            .map_err(|_| format!("invalid HEAD~N: {target}"))?;
        let (_, head_id) = repo.head()?;
        let mut current = head_id;
        for _ in 0..n {
            let patches = repo.log(None)?;
            let patch = patches
                .iter()
                .find(|p| p.id == current)
                .ok_or("HEAD ancestor not found")?;
            current = patch
                .parent_ids
                .first()
                .ok_or("HEAD has no parent")?
                .to_owned();
        }
        return Ok(current);
    }

    if let Ok(hash) = Hash::from_hex(target)
        && repo.dag().has_patch(&hash)
    {
        return Ok(hash);
    }

    let branches = repo.list_branches();
    if let Some((_, patch_id)) = branches.iter().find(|(name, _)| name == target) {
        return Ok(*patch_id);
    }

    Err(format!(
        "error: could not resolve '{target}': not a valid branch, tag, or commit hash"
    )
    .into())
}
