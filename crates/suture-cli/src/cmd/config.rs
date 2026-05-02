pub async fn cmd_config(
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
                println!("{key}={value}");
            }
        }
        return Ok(());
    }

    let (key, value) = if key_value.len() >= 2 {
        (
            key_value[0].trim().to_owned(),
            key_value[1..].join(" ").trim().to_owned(),
        )
    } else if let Some((k, v)) = key_value[0].split_once('=') {
        (k.trim().to_owned(), v.trim().to_owned())
    } else {
        let key = key_value[0].trim();
        if let Some(value) = repo.get_config(key)? {
            println!("{value}");
            return Ok(());
        } else {
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
                    "config key '{key}' not found (did you mean '{suggestion}'?)"
                );
            } else {
                eprintln!("config key '{key}' not found");
            }
            std::process::exit(1);
        }
    };

    repo.set_config(&key, &value)?;
    println!("{key}={value}");

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
                println!("{key}={value}");
            }
        }
        return Ok(());
    }

    let (key, value) = if key_value.len() >= 2 {
        (
            key_value[0].trim().to_owned(),
            key_value[1..].join(" ").trim().to_owned(),
        )
    } else if let Some((k, v)) = key_value[0].split_once('=') {
        (k.trim().to_owned(), v.trim().to_owned())
    } else {
        let key = key_value[0].trim();
        if let Some(value) = table
            .get(key)
            .and_then(|v| v.as_str().map(std::borrow::ToOwned::to_owned)) {
            println!("{value}");
            return Ok(());
        } else {
            let all_keys: Vec<String> = table.keys().cloned().collect();
            if let Some(suggestion) = crate::fuzzy::suggest(key, &all_keys) {
                eprintln!(
                    "global config key '{key}' not found (did you mean '{suggestion}'?)"
                );
            } else {
                eprintln!("global config key '{key}' not found");
            }
            std::process::exit(1);
        }
    };

    table.insert(key.clone(), toml::Value::String(value.clone()));

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config_path, toml::to_string_pretty(&table)?)?;

    println!("{key}={value}");

    Ok(())
}
