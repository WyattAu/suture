pub async fn cmd_tui() -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = std::path::Path::new(".");
    suture_tui::run(repo_path)?;
    Ok(())
}
