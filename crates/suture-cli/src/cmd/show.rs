use crate::display::format_timestamp;
use crate::ref_utils::resolve_ref;

pub(crate) async fn cmd_show(commit_ref: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit_ref, &patches)?;

    println!("commit {}", target.id.to_hex());
    println!("Author: {}", target.author);
    println!("Date:    {}", format_timestamp(target.timestamp));
    println!();
    println!("    {}", target.message);

    if !target.payload.is_empty()
        && let Some(path) = &target.target_path
    {
        println!("\n  {} {}", target.operation_type, path);
    }

    if !target.parent_ids.is_empty() {
        print!("\nParents:");
        for pid in &target.parent_ids {
            print!(" {}", pid);
        }
        println!();
    }

    Ok(())
}
