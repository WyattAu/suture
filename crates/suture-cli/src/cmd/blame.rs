pub(crate) async fn cmd_blame(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let entries = repo.blame(path)?;
    for entry in &entries {
        let short_hash = entry.patch_id.to_hex().chars().take(8).collect::<String>();
        if entry.patch_id == suture_common::Hash::ZERO {
            println!("{:>4} | {}", entry.line_number, entry.line);
        } else {
            println!(
                "{:>4} | {} ({}) {}",
                entry.line_number, short_hash, entry.author, entry.line
            );
        }
    }
    Ok(())
}
