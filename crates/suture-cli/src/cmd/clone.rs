use std::path::PathBuf;

use crate::cmd::user_error;
use crate::remote_proto::do_pull_with_depth;

pub(crate) async fn cmd_clone(
    url: &str,
    dir: Option<&str>,
    depth: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    if url.trim().is_empty() {
        return Err("repository URL is required (e.g., http://localhost:50051/my-repo)".into());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(
            format!("invalid URL '{url}' (URLs must start with http:// or https://)").into(),
        );
    }

    let repo_name = dir.unwrap_or_else(|| {
        url.trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("suture-repo")
    });

    let repo_path = PathBuf::from(repo_name);
    if repo_path.exists() {
        return Err(format!(
            "directory '{repo_name}' already exists (remove it or specify a different directory)"
        )
        .into());
    }

    std::fs::create_dir_all(&repo_path)
        .map_err(|e| user_error(&format!("failed to create directory '{repo_name}'"), e))?;
    let mut repo = suture_core::repository::Repository::init(&repo_path, "unknown")
        .map_err(|e| user_error("failed to initialize repository", e))?;
    repo.add_remote("origin", url)
        .map_err(|e| user_error("failed to configure remote 'origin'", e))?;

    eprintln!("Cloning into '{}'...", repo_name);
    let new_patches = do_pull_with_depth(&mut repo, "origin", depth)
        .await
        .map_err(|e| user_error(&format!("failed to clone from '{url}'"), e))?;

    println!("Cloned into '{}'", repo_name);
    if new_patches > 0 {
        println!("  {} patch(es) pulled", new_patches);
    }
    Ok(())
}
