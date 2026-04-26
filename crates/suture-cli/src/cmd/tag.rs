use crate::cmd::user_error;

pub(crate) async fn cmd_tag(
    name: Option<&str>,
    target: Option<&str>,
    delete: bool,
    list: bool,
    annotate: bool,
    message: Option<&str>,
    sort: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;

    if list || name.is_none() {
        let mut tags = repo.list_tags()?;

        if let Some(pattern) = name {
            tags.retain(|(tname, _)| matches_pattern(tname, pattern));
        }

        match sort {
            Some("date") => {
                tags.sort_by(|a, b| {
                    let ts_a = repo.dag().get_patch(&a.1).map(|p| p.timestamp).unwrap_or(0);
                    let ts_b = repo.dag().get_patch(&b.1).map(|p| p.timestamp).unwrap_or(0);
                    ts_b.cmp(&ts_a)
                });
            }
            Some("name") | None => {
                tags.sort_by_key(|a| a.0.clone());
            }
            Some(other) => {
                eprintln!("error: unsupported sort order '{other}' (use 'date' or 'name')");
                std::process::exit(1);
            }
        }

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

    let name = name.ok_or_else(|| "tag name required (use --list to show tags)".to_string())?;
    if delete {
        let tags = repo
            .list_tags()
            .map_err(|e| user_error("failed to list tags", e))?;
        if !tags.iter().any(|(t, _)| t == name) {
            return Err(format!(
                "tag '{name}' not found (use 'suture tag --list' to see available tags)"
            )
            .into());
        }
        repo.delete_tag(name)
            .map_err(|e| user_error(&format!("failed to delete tag '{name}'"), e))?;
        let msg_key = format!("tag.{}.message", name);
        let _ = repo.meta().delete_config(&msg_key);
        println!("Deleted tag '{}'", name);
    } else {
        let tags = repo
            .list_tags()
            .map_err(|e| user_error("failed to list tags", e))?;
        if tags.iter().any(|(t, _)| t == name) {
            return Err(format!(
                "tag '{name}' already exists (delete it first with 'suture tag -d {name}')"
            )
            .into());
        }
        repo.create_tag(name, target)
            .map_err(|e| user_error(&format!("failed to create tag '{name}'"), e))?;
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

fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() != 2 {
            return name == pattern;
        }
        name.starts_with(parts[0]) && name.ends_with(parts[1])
    } else {
        name == pattern
    }
}
