use crate::style::run_hook_if_exists;

pub(crate) async fn cmd_commit(message: &str, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    if all {
        let count = repo.add_all()?;
        if count > 0 {
            println!("Staged {} file(s)", count);
        }
    }

    // Run pre-commit hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let author = repo
        .get_config("user.name")
        .unwrap_or(None)
        .unwrap_or_default();
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_AUTHOR".to_string(), author);
    extra.insert("SUTURE_BRANCH".to_string(), branch.clone());
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    run_hook_if_exists(repo.root(), "pre-commit", extra)?;

    let patch_id = repo.commit(message)?;
    println!("Committed: {}", patch_id);

    // Run post-commit hook
    let (branch, head_id) = repo.head()?;
    let author = repo
        .get_config("user.name")
        .unwrap_or(None)
        .unwrap_or_default();
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_AUTHOR".to_string(), author);
    extra.insert("SUTURE_BRANCH".to_string(), branch);
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    extra.insert("SUTURE_COMMIT".to_string(), patch_id.to_hex());
    run_hook_if_exists(repo.root(), "post-commit", extra)?;

    Ok(())
}
