use crate::RemoteAction;
use crate::cmd::user_error;

pub async fn cmd_remote(action: &crate::RemoteAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;
    match action {
        RemoteAction::Add { name, url } => {
            let remotes = repo.list_remotes().unwrap_or_default();
            if remotes.iter().any(|(n, _)| n == name) {
                return Err(format!(
                    "remote '{name}' already exists (use 'suture remote rename' to rename it)"
                )
                .into());
            }
            repo.add_remote(name, url)
                .map_err(|e| user_error(&format!("failed to add remote '{name}'"), e))?;
            println!("Remote '{name}' added -> {url}");
        }
        RemoteAction::List => {
            let remotes = repo
                .list_remotes()
                .map_err(|e| user_error("failed to list remotes", e))?;
            if remotes.is_empty() {
                println!("No remotes configured.");
            } else {
                for (name, url) in &remotes {
                    println!("{name}\t{url}");
                }
            }
        }
        RemoteAction::Remove { name } => {
            let remotes = repo.list_remotes().unwrap_or_default();
            if !remotes.iter().any(|(n, _)| n == name) {
                return Err(format!(
                    "remote '{name}' not found (use 'suture remote list' to see available remotes)"
                )
                .into());
            }
            repo.remove_remote(name)
                .map_err(|e| user_error(&format!("failed to remove remote '{name}'"), e))?;
            println!("Remote '{name}' removed");
        }
        RemoteAction::Rename { old_name, new_name } => {
            let remotes = repo.list_remotes().unwrap_or_default();
            if !remotes.iter().any(|(n, _)| n == old_name) {
                return Err(format!("remote '{old_name}' not found (use 'suture remote list' to see available remotes)").into());
            }
            if remotes.iter().any(|(n, _)| n == new_name) {
                return Err(format!(
                    "remote '{new_name}' already exists (choose a different name)"
                )
                .into());
            }
            repo.rename_remote(old_name, new_name)
                .map_err(|e| user_error(&format!("failed to rename remote '{old_name}'"), e))?;
            println!("Renamed remote '{old_name}' → '{new_name}'");
        }
        RemoteAction::Login { name } => {
            let remotes = repo.list_remotes().unwrap_or_default();
            if !remotes.iter().any(|(n, _)| n == name) {
                return Err(format!(
                    "remote '{name}' not found (use 'suture remote list' to see available remotes)"
                )
                .into());
            }
            let remote_url = repo
                .get_remote_url(name)
                .map_err(|e| user_error(&format!("failed to get URL for remote '{name}'"), e))?;

            eprintln!("Authenticating with {remote_url}...");

            let client = reqwest::Client::new();
            let response = client
                .post(format!("{remote_url}/auth/token"))
                .send()
                .await
                .map_err(|e| user_error(&format!("network error connecting to '{name}'"), e))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!("login failed (HTTP {status}): {body}").into());
            }

            let body: serde_json::Value = response.json().await?;
            let token = body["token"]
                .as_str()
                .ok_or("invalid response from server")?;

            repo.set_config(&format!("remote.{name}.token"), token)?;

            eprintln!("Authentication successful. Token stored in config.");
        }
        RemoteAction::Mirror {
            url,
            repo: upstream_repo,
            name: local_name,
        } => {
            let local_repo_name = local_name.as_deref().unwrap_or(upstream_repo);

            #[derive(serde::Serialize)]
            struct MirrorSetupReq {
                repo_name: String,
                upstream_url: String,
                upstream_repo: String,
            }
            #[derive(serde::Deserialize)]
            struct MirrorSetupResp {
                success: bool,
                error: Option<String>,
                mirror_id: Option<i64>,
            }
            #[derive(serde::Serialize)]
            struct MirrorSyncReq {
                mirror_id: i64,
            }
            #[derive(serde::Deserialize)]
            struct MirrorSyncResp {
                success: bool,
                error: Option<String>,
                patches_synced: u64,
                branches_synced: u64,
            }

            let client = reqwest::Client::new();

            let setup_body = MirrorSetupReq {
                repo_name: local_repo_name.to_owned(),
                upstream_url: url.clone(),
                upstream_repo: upstream_repo.clone(),
            };

            let hub_url = repo
                .get_remote_url("origin")
                .unwrap_or_else(|_| url.clone());
            let setup_resp = client
                .post(format!("{hub_url}/mirror/setup"))
                .json(&setup_body)
                .send()
                .await?;

            let setup_result: MirrorSetupResp = setup_resp.json().await?;
            if !setup_result.success {
                return Err(setup_result
                    .error
                    .unwrap_or_else(|| "mirror setup failed".to_owned())
                    .into());
            }

            let mirror_id = setup_result.mirror_id.ok_or("no mirror id returned")?;
            println!("Mirror registered (id: {mirror_id}), syncing...");

            let sync_resp = client
                .post(format!("{hub_url}/mirror/sync"))
                .json(&MirrorSyncReq { mirror_id })
                .send()
                .await?;

            let sync_result: MirrorSyncResp = sync_resp.json().await?;
            if !sync_result.success {
                return Err(sync_result
                    .error
                    .unwrap_or_else(|| "mirror sync failed".to_owned())
                    .into());
            }

            println!(
                "Mirror sync complete: {} patch(es), {} branch(es)",
                sync_result.patches_synced, sync_result.branches_synced
            );
        }
    }
    Ok(())
}
