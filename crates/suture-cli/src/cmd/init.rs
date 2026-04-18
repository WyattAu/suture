use std::path::PathBuf;

pub(crate) async fn cmd_init(
    path: &str,
    repo_type: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = PathBuf::from(path);
    let repo = suture_core::repository::Repository::init(&repo_path, "unknown")?;

    let resolved_type = if let Some(ty) = repo_type {
        suture_core::file_type::RepoType::from_str_value(ty)
            .ok_or_else(|| format!("unknown repo type: {ty} (expected: video, document, data)"))?
    } else {
        let detected = suture_core::file_type::auto_detect_repo_type(&repo_path);
        if let Some(rt) = detected {
            println!("Auto-detected repo type: {}", rt.as_str());
            rt
        } else {
            println!("No specific repo type detected (generic repository)");
            drop(repo);
            println!(
                "Initialized empty Suture repository in {}",
                repo_path.display()
            );
            println!("Hint: run `suture config user.name \"Your Name\"` to set your identity");
            return Ok(());
        }
    };

    let config_dir = repo_path.join(".suture");
    let config_path = config_dir.join("config");
    let config_entry = format!("repo.type = \"{}\"", resolved_type.as_str());

    if config_path.exists() {
        let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
        if !existing.contains("repo.type") {
            let updated = format!("{existing}\n{config_entry}\n");
            std::fs::write(&config_path, updated)?;
        }
    } else {
        std::fs::write(&config_path, format!("{config_entry}\n"))?;
    }

    println!(
        "Initialized {} Suture repository in {}",
        resolved_type.as_str(),
        repo_path.display()
    );
    println!("Hint: run `suture config user.name \"Your Name\"` to set your identity");

    drop(repo);
    Ok(())
}
