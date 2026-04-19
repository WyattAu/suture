pub(crate) async fn cmd_gc() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.gc()?;
    println!("Garbage collection complete (transactional).");
    println!("  {} unreachable patch(es) removed", result.patches_removed);
    println!("  {} orphaned blob(s) removed", result.blobs_removed);
    if result.patches_removed > 0 || result.blobs_removed > 0 {
        println!("  Hint: reopen the repository to fully update the in-memory DAG");
    }
    Ok(())
}
