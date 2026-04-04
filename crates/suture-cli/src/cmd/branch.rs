pub(crate) async fn cmd_branch(
    name: Option<&str>,
    target: Option<&str>,
    delete: bool,
    list: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if list || name.is_none() {
        let branches = repo.list_branches();
        if branches.is_empty() {
            println!("No branches.");
        } else {
            let head = repo.head().ok();
            let head_branch = head.as_ref().map(|(n, _)| n.as_str());
            for (bname, _target) in &branches {
                let marker = if head_branch == Some(bname.as_str()) {
                    "* "
                } else {
                    "  "
                };
                println!("{}{}", marker, bname);
            }
        }
        return Ok(());
    }

    let name =
        name.ok_or_else(|| "branch name required (use --list to show branches)".to_string())?;
    if delete {
        let branches: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
        if !branches.iter().any(|b| b == name) {
            if let Some(suggestion) = crate::fuzzy::suggest(name, &branches) {
                return Err(format!(
                    "branch '{}' not found (did you mean '{}'?)",
                    name, suggestion
                )
                .into());
            } else {
                return Err(format!("branch '{}' not found", name).into());
            }
        }
        repo.delete_branch(name)?;
        println!("Deleted branch '{}'", name);
    } else {
        repo.create_branch(name, target)?;
        println!("Created branch '{}'", name);
    }
    Ok(())
}
