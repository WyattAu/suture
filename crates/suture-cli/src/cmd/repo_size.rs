pub async fn cmd_repo_size() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let patches = repo.dag().patch_ids().len();
    let branches = repo.list_branches().len();
    let tags = repo.list_tags().unwrap_or_default().len();
    let blobs = repo.cas().list_blobs().unwrap_or_default().len();
    let total_size = repo.cas().total_size().unwrap_or(0);
    let dag_nodes = repo.dag().patch_ids().len();

    println!("Repository size statistics:");
    println!("  Patches:       {patches}");
    println!("  Branches:      {branches}");
    println!("  Tags:          {tags}");
    println!("  Blobs:         {blobs}");
    println!("  Total blob size: {total_size} bytes");
    println!("  DAG nodes:     {dag_nodes}");

    Ok(())
}
