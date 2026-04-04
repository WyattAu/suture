pub(crate) async fn cmd_version() -> Result<(), Box<dyn std::error::Error>> {
    println!("suture {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
