pub(crate) async fn cmd_checkout(
    branch: Option<&str>,
    new_branch: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    if let Some(name) = new_branch {
        let source = branch.filter(|b| *b != "HEAD");
        repo.create_branch(name, source)?;
        repo.checkout(name)?;
        println!("Created and switched to branch '{}'", name);
    } else {
        let target = branch.ok_or("no branch specified (use -b to create one)")?;
        if let Err(e) = repo.checkout(target) {
            let branches: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
            if let Some(suggestion) = crate::fuzzy::suggest(target, &branches) {
                return Err(format!("{} (did you mean '{}'?)", e, suggestion).into());
            } else {
                return Err(e.into());
            }
        }
        if repo.is_detached() {
            if let Ok(Some(id)) = repo.get_detached_head() {
                let short = &id.to_hex()[..12];
                println!("Note: checking out '{}'.", short);
                println!("You are in 'detached HEAD' state. You can look around, make experimental");
                println!("changes and commit them, and you can discard any commits you make in this");
                println!("state without impacting any branches by switching back to a branch.");
            }
        } else {
            println!("Switched to branch '{}'", target);
        }
    }
    Ok(())
}
