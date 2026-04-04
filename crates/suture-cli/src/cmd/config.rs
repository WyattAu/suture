pub(crate) async fn cmd_config(key_value: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if key_value.is_empty() {
        let entries = repo.list_config()?;
        if entries.is_empty() {
            println!("No configuration set.");
        } else {
            for (key, value) in &entries {
                if key.starts_with("pending_merge_parents") || key.starts_with("head_branch") {
                    continue;
                }
                println!("{}={}", key, value);
            }
        }
        return Ok(());
    }

    let kv = &key_value[0];
    if let Some((key, value)) = kv.split_once('=') {
        repo.set_config(key.trim(), value.trim())?;
        println!("{}={}", key.trim(), value.trim());
    } else {
        let key = kv.trim();
        match repo.get_config(key)? {
            Some(value) => println!("{}", value),
            None => {
                let all_keys: Vec<String> = repo
                    .list_config()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(k, _)| k)
                    .filter(|k| {
                        !k.starts_with("pending_merge_parents") && !k.starts_with("head_branch")
                    })
                    .collect();
                if let Some(suggestion) = crate::fuzzy::suggest(key, &all_keys) {
                    eprintln!(
                        "config key '{}' not found (did you mean '{}'?)",
                        key, suggestion
                    );
                } else {
                    eprintln!("config key '{}' not found", key);
                }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
