pub async fn cmd_reset(target: &str, mode: &str) -> Result<(), Box<dyn std::error::Error>> {
    use suture_core::repository::ResetMode;

    let reset_mode = match mode {
        "soft" => ResetMode::Soft,
        "mixed" => ResetMode::Mixed,
        "hard" => ResetMode::Hard,
        _ => {
            return Err(format!(
                "invalid reset mode: '{mode}' (expected soft, mixed, hard)"
            )
            .into());
        }
    };

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let target_id = repo.reset(target, reset_mode)?;
    println!("HEAD is now at {target_id}");
    Ok(())
}
