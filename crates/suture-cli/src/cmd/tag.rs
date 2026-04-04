pub(crate) async fn cmd_tag(
    name: Option<&str>,
    target: Option<&str>,
    delete: bool,
    list: bool,
    annotate: bool,
    message: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if list || name.is_none() {
        let tags = repo.list_tags()?;
        if tags.is_empty() {
            println!("No tags.");
        } else {
            for (tname, target_id) in &tags {
                if let Some(msg) = repo.get_config(&format!("tag.{}.message", tname))? {
                    println!("{} (annotated)  {}  {}", tname, target_id, msg);
                } else {
                    println!("{}  {}", tname, target_id);
                }
            }
        }
        return Ok(());
    }

    let name =
        name.ok_or_else(|| "branch name required (use --list to show branches)".to_string())?;
    if delete {
        repo.delete_tag(name)?;
        let msg_key = format!("tag.{}.message", name);
        let _ = repo.meta().delete_config(&msg_key);
        println!("Deleted tag '{}'", name);
    } else {
        repo.create_tag(name, target)?;
        let target_id = repo
            .resolve_tag(name)?
            .ok_or_else(|| format!("created tag '{}', but could not resolve it", name))?;
        if annotate {
            let msg = message.ok_or_else(|| {
                eprintln!("Error: --annotate requires a message (-m)");
                std::process::exit(1);
            })?;
            repo.set_config(&format!("tag.{}.message", name), msg)?;
            println!("Tag '{}' (annotated) -> {}", name, target_id);
        } else {
            println!("Tag '{}' -> {}", name, target_id);
        }
    }
    Ok(())
}
