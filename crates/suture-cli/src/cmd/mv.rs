pub async fn cmd_mv(
    source: &str,
    destination: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    repo.rename_file(source, destination)?;
    println!("Renamed {source} -> {destination}");
    Ok(())
}
