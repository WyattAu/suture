use std::path::PathBuf;

pub(crate) async fn cmd_init(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = PathBuf::from(path);
    let repo = suture_core::repository::Repository::init(&repo_path, "unknown")?;
    println!(
        "Initialized empty Suture repository in {}",
        repo_path.display()
    );
    println!("Hint: run `suture config user.name=\"Your Name\"` to set your identity");
    drop(repo);
    Ok(())
}
