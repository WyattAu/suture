pub(crate) async fn cmd_squash(
    count: usize,
    message: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let msg = message.unwrap_or("squashed commit");
    let new_id = repo.squash(count, msg)?;
    println!("Squashed {} commits into {}", count, new_id);
    Ok(())
}
