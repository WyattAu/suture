pub(crate) async fn cmd_config(
    key_value: &[String],
    global: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if global {
        cmd_config_global(key_value).await
    } else {
        cmd_config_repo(key_value).await
    }
}

async fn cmd_config_repo(key_value: &[String]) -> Result<(), Box<dyn std::error::Error>> {
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

    let (key, value) = if key_value.len() >= 2 {
        (
            key_value[0].trim().to_string(),
            key_value[1..].join(" ").trim().to_string(),
        )
    } else if let Some((k, v)) = key_value[0].split_once('=') {
        (k.trim().to_string(), v.trim().to_string())
    } else {
        let key = key_value[0].trim();
        match repo.get_config(key)? {
            Some(value) => {
                println!("{}", value);
                return Ok(());
            }
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
    };

    repo.set_config(&key, &value)?;
    println!("{}={}", key, value);

    Ok(())
}

async fn cmd_config_global(key_value: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = suture_core::metadata::global_config::GlobalConfig::config_path();

    let mut table: toml::Table = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        toml::from_str(&content).unwrap_or_default()
    } else {
        toml::Table::new()
    };

    if key_value.is_empty() {
        if table.is_empty() {
            println!("No global configuration set.");
        } else {
            for (key, value) in &table {
                println!("{}={}", key, value);
            }
        }
        return Ok(());
    }

    let (key, value) = if key_value.len() >= 2 {
        (
            key_value[0].trim().to_string(),
            key_value[1..].join(" ").trim().to_string(),
        )
    } else if let Some((k, v)) = key_value[0].split_once('=') {
        (k.trim().to_string(), v.trim().to_string())
    } else {
        let key = key_value[0].trim();
        match table
            .get(key)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
        {
            Some(value) => {
                println!("{}", value);
                return Ok(());
            }
            None => {
                let all_keys: Vec<String> = table.keys().cloned().collect();
                if let Some(suggestion) = crate::fuzzy::suggest(key, &all_keys) {
                    eprintln!(
                        "global config key '{}' not found (did you mean '{}'?)",
                        key, suggestion
                    );
                } else {
                    eprintln!("global config key '{}' not found", key);
                }
                std::process::exit(1);
            }
        }
    };

    table.insert(key.clone(), toml::Value::String(value.clone()));

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config_path, toml::to_string_pretty(&table)?)?;

    println!("{}={}", key, value);

    Ok(())
}
