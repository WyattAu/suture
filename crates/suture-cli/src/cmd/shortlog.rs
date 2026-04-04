pub(crate) async fn cmd_shortlog(
    branch: Option<&str>,
    number: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let mut patches = repo.log(branch)?;

    if let Some(n) = number {
        patches.truncate(n);
    }

    let mut by_author: std::collections::BTreeMap<String, Vec<&suture_core::patch::types::Patch>> =
        std::collections::BTreeMap::new();
    for patch in &patches {
        by_author
            .entry(patch.author.clone())
            .or_default()
            .push(patch);
    }

    for (author, commits) in &by_author {
        let count = commits.len();
        let short_hash = commits
            .last()
            .map(|p| p.id.to_hex().chars().take(8).collect::<String>())
            .unwrap_or_default();
        let first_msg = commits
            .first()
            .map(|p| p.message.trim().lines().next().unwrap_or(""))
            .unwrap_or("");
        println!("{} ({}) {} {}", short_hash, count, author, first_msg);
    }

    Ok(())
}
