use crate::RemoteAction;

pub(crate) async fn cmd_remote(
    action: &crate::RemoteAction,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    match action {
        RemoteAction::Add { name, url } => {
            repo.add_remote(name, url)?;
            println!("Remote '{}' added -> {}", name, url);
        }
        RemoteAction::List => {
            let remotes = repo.list_remotes()?;
            if remotes.is_empty() {
                println!("No remotes configured.");
            } else {
                for (name, url) in &remotes {
                    println!("{}\t{}", name, url);
                }
            }
        }
        RemoteAction::Remove { name } => {
            repo.remove_remote(name)?;
            println!("Remote '{}' removed", name);
        }
        RemoteAction::Rename { old_name, new_name } => {
            repo.rename_remote(old_name, new_name)?;
            println!("Renamed remote '{}' → '{}'", old_name, new_name);
        }
        RemoteAction::Login { name } => {
            let remote_url = repo.get_remote_url(name)?;

            eprintln!("Authenticating with {}...", remote_url);

            let client = reqwest::Client::new();
            let response = client
                .post(format!("{}/auth/token", remote_url))
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!("login failed (HTTP {}): {}", status, body).into());
            }

            let body: serde_json::Value = response.json().await?;
            let token = body["token"]
                .as_str()
                .ok_or("invalid response from server")?;

            repo.set_config(&format!("remote.{}.token", name), token)?;

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
                repo_name: local_repo_name.to_string(),
                upstream_url: url.clone(),
                upstream_repo: upstream_repo.clone(),
            };

            let hub_url = repo
                .get_remote_url("origin")
                .unwrap_or_else(|_| url.clone());
            let setup_resp = client
                .post(format!("{}/mirror/setup", hub_url))
                .json(&setup_body)
                .send()
                .await?;

            let setup_result: MirrorSetupResp = setup_resp.json().await?;
            if !setup_result.success {
                return Err(setup_result
                    .error
                    .unwrap_or_else(|| "mirror setup failed".to_string())
                    .into());
            }

            let mirror_id = setup_result.mirror_id.ok_or("no mirror id returned")?;
            println!("Mirror registered (id: {mirror_id}), syncing...");

            let sync_resp = client
                .post(format!("{}/mirror/sync", hub_url))
                .json(&MirrorSyncReq { mirror_id })
                .send()
                .await?;

            let sync_result: MirrorSyncResp = sync_resp.json().await?;
            if !sync_result.success {
                return Err(sync_result
                    .error
                    .unwrap_or_else(|| "mirror sync failed".to_string())
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
