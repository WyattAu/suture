use crate::ref_utils::resolve_ref;
use std::path::Path;

pub(crate) async fn cmd_export(
    destination: &str,
    commit: Option<&str>,
    zip: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;
    let patches = repo.all_patches();

    let tree = match commit {
        Some(ref_str) => {
            let patch = resolve_ref(&repo, ref_str, &patches)?;
            repo.snapshot(&patch.id)?
        }
        None => repo.snapshot_head()?,
    };

    if zip {
        export_as_zip(&repo, &tree, destination)?;
    } else {
        export_as_dir(&repo, &tree, destination)?;
    }

    Ok(())
}

fn export_as_dir(
    repo: &suture_core::repository::Repository,
    tree: &suture_core::engine::tree::FileTree,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dest = Path::new(destination);
    if dest.exists() {
        return Err(format!("destination '{}' already exists", destination).into());
    }

    let mut file_count = 0usize;
    for (path, hash) in tree.iter() {
        if path.starts_with(".suture/") {
            continue;
        }
        let full_path = dest.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = repo.cas().get_blob(hash).map_err(|e| e.to_string())?;
        std::fs::write(&full_path, data)?;
        file_count += 1;
    }

    println!("Exported {} files to {}", file_count, destination);
    Ok(())
}

fn export_as_zip(
    repo: &suture_core::repository::Repository,
    tree: &suture_core::engine::tree::FileTree,
    output: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("suture_export_{}", std::process::id()));

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }
    std::fs::create_dir_all(&temp_dir)?;

    let mut file_count = 0usize;
    for (path, hash) in tree.iter() {
        if path.starts_with(".suture/") {
            continue;
        }
        let data = repo.cas().get_blob(hash).map_err(|e| e.to_string())?;
        let full_path = temp_dir.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, data)?;
        file_count += 1;
    }

    let status = std::process::Command::new("zip")
        .arg("-r")
        .arg(output)
        .arg(".")
        .current_dir(&temp_dir)
        .status()?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    if !status.success() {
        return Err("zip command failed".into());
    }

    println!("Exported {} files to {}", file_count, output);
    Ok(())
}
