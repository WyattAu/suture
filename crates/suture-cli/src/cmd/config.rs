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

    // Support: suture config key=value  (single arg with =)
    //          suture config key value  (two args, joined with space)
    let (key, value) = if key_value.len() >= 2 {
        // Multi-arg form: suture config user.name "Your Name"
        (
            key_value[0].trim().to_string(),
            key_value[1..].join(" ").trim().to_string(),
        )
    } else if let Some((k, v)) = key_value[0].split_once('=') {
        // Single-arg form: suture config user.name=Your
        (k.trim().to_string(), v.trim().to_string())
    } else {
        // No value provided — treat as a get
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
