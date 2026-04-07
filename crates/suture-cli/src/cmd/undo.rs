pub(crate) async fn cmd_undo(steps: Option<usize>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let n = steps.unwrap_or(1);
    let target = format!("HEAD~{}", n);
    let target_id = repo.reset(&target, suture_core::repository::ResetMode::Soft)?;
    println!("Undid {} commit(s): HEAD is now at {}", n, target_id);
    Ok(())
}
