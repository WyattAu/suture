use crate::KeyAction;

pub async fn cmd_key(action: &crate::KeyAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    match action {
        KeyAction::Generate { name } => {
            let keypair = suture_core::signing::SigningKeypair::generate();
            let keys_dir = std::path::Path::new(".suture").join("keys");
            std::fs::create_dir_all(&keys_dir)?;

            let priv_path = keys_dir.join(format!("{name}.ed25519"));
            std::fs::write(&priv_path, keypair.private_key_bytes())?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&priv_path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| format!("failed to set key permissions: {e}"))?;
            }

            let pub_hex = hex::encode(keypair.public_key_bytes());
            repo.set_config(&format!("key.public.{name}"), &pub_hex)?;

            if name == "default" {
                repo.set_config("signing.key", "default")?;
            }

            println!("Generated keypair '{name}'");
            println!("  Private key: {}", priv_path.display());
            println!("  Public key:  {pub_hex}");
            if name != "default" {
                println!(
                    "Hint: run `suture config signing.key={name}` to use this key for signing"
                );
            }
        }
        KeyAction::List => {
            let entries = repo.list_config()?;
            let mut found = false;
            for (key, value) in &entries {
                if let Some(name) = key.strip_prefix("key.public.") {
                    println!("{name}  {value}");
                    found = true;
                }
            }
            if !found {
                println!("No signing keys found.");
                println!("Run `suture key generate` to create one.");
            }
        }
        KeyAction::Public { name } => {
            let key = format!("key.public.{name}");
            if let Some(pub_hex) = repo.get_config(&key)? { println!("{pub_hex}") } else {
                let all_keys: Vec<String> = repo
                    .list_config()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|(k, _)| k.strip_prefix("key.public.").map(std::borrow::ToOwned::to_owned))
                    .collect();
                if let Some(suggestion) = crate::fuzzy::suggest(name, &all_keys) {
                    eprintln!(
                        "No public key found for '{name}' (did you mean '{suggestion}'?)"
                    );
                } else {
                    eprintln!("No public key found for '{name}'");
                }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
