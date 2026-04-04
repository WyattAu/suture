pub(crate) async fn cmd_gc() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.gc()?;
    println!("Garbage collection complete.");
    println!("  {} patch(es) removed", result.patches_removed);
    if result.patches_removed > 0 {
        println!("  Hint: reopen the repository to fully update the in-memory DAG");
    }
    Ok(())
}
