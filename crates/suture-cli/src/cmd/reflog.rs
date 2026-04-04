pub(crate) async fn cmd_reflog() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let entries = repo.reflog_entries()?;
    if entries.is_empty() {
        println!("No reflog entries.");
        return Ok(());
    }
    for (head_hash, entry) in entries.iter().rev() {
        let short_hash = if head_hash.len() >= 8 {
            &head_hash[..8]
        } else {
            head_hash
        };
        println!("{} {}", short_hash, entry);
    }
    Ok(())
}
