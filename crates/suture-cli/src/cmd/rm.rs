pub(crate) async fn cmd_rm(
    paths: &[String],
    cached: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    for path in paths {
        if !cached {
            let file_path = std::path::Path::new(path);
            if file_path.exists() {
                std::fs::remove_file(file_path)?;
            }
        }
        if cached {
            let repo_path = suture_common::RepoPath::new(path)?;
            repo.meta()
                .working_set_add(&repo_path, suture_common::FileStatus::Deleted)?;
        } else {
            repo.add(path)?;
        }
        println!("Removed {}", path);
    }
    Ok(())
}
