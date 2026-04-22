use std::path::PathBuf;

use crate::remote_proto::do_pull_with_depth;

pub(crate) async fn cmd_clone(
    url: &str,
    dir: Option<&str>,
    depth: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_name = dir.unwrap_or_else(|| {
        url.trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("suture-repo")
    });

    let repo_path = PathBuf::from(repo_name);
    if repo_path.exists() {
        return Err(format!("directory '{}' already exists", repo_name).into());
    }

    std::fs::create_dir_all(&repo_path)?;
    let mut repo = suture_core::repository::Repository::init(&repo_path, "unknown")?;
    repo.add_remote("origin", url)?;

    eprintln!("Cloning into '{}'...", repo_name);
    let new_patches = do_pull_with_depth(&mut repo, "origin", depth).await?;

    println!("Cloned into '{}'", repo_name);
    if new_patches > 0 {
        println!("  {} patch(es) pulled", new_patches);
    }
    Ok(())
}
