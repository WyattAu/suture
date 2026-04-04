pub(crate) async fn cmd_fsck() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.fsck()?;
    println!("Repository integrity check complete.");
    println!("  {} check(s) passed", result.checks_passed);
    if !result.warnings.is_empty() {
        println!("\nWarnings:");
        for w in &result.warnings {
            println!("  WARNING: {}", w);
        }
    }
    if !result.errors.is_empty() {
        println!("\nErrors:");
        for e in &result.errors {
            println!("  ERROR: {}", e);
        }
    }
    Ok(())
}
