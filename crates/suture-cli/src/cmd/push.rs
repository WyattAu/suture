use crate::cmd::user_error;
use crate::remote_proto::{
    BlobRef, BranchProto, PushRequest, PushResponse, check_handshake, derive_repo_id,
    hex_to_hash_proto, patch_to_proto, sign_push_request,
};
use crate::style::run_hook_if_exists;
use base64::Engine;

pub async fn cmd_push(
    remote: &str,
    force: bool,
    branch: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;

    let remotes = repo.list_remotes().unwrap_or_default();
    if !remotes.iter().any(|r| r.0 == remote) {
        return Err(format!(
            "remote '{remote}' not found (use 'suture remote add {remote} <url>' to add it)"
        )
        .into());
    }

    let url = repo
        .get_remote_url(remote)
        .map_err(|e| user_error(&format!("failed to get URL for remote '{remote}'"), e))?;

    check_handshake(&url)
        .await
        .map_err(|e| user_error(&format!("failed to connect to remote '{remote}'"), e))?;

    let branches = repo.list_branches();
    let branches_to_push: Vec<(String, suture_common::Hash)> = if let Some(branch_name) = branch {
        branches
            .into_iter()
            .filter(|(n, _)| n == branch_name)
            .collect()
    } else {
        branches
    };

    if branches_to_push.is_empty() {
        if let Some(branch_name) = branch {
            return Err(format!(
                "branch '{branch_name}' not found locally (use 'suture branch' to list branches)"
            )
            .into());
        }
        return Err("no branches to push".into());
    }

    let push_state_key = format!("remote.{remote}.last_pushed");
    let patches = if let Some(last_pushed_hex) = repo.get_config(&push_state_key)? {
        let last_pushed = suture_common::Hash::from_hex(&last_pushed_hex)?;
        repo.patches_since(&last_pushed)
    } else {
        repo.all_patches()
    };

    let b64 = base64::engine::general_purpose::STANDARD;

    let mut blobs = Vec::new();
    let mut seen_hashes = std::collections::HashSet::new();
    for patch in &patches {
        let file_changes = patch.file_changes();
        let is_batch = patch.operation_type == suture_core::patch::types::OperationType::Batch;

        if is_batch {
            let changes = file_changes.as_deref().unwrap_or(&[]);
            for change in changes {
                if change.payload.is_empty() {
                    continue;
                }
                let hash_hex = String::from_utf8_lossy(&change.payload).to_string();
                if seen_hashes.contains(&hash_hex) {
                    continue;
                }
                let Ok(hash) = suture_common::Hash::from_hex(&hash_hex) else {
                    continue;
                };
                seen_hashes.insert(hash_hex.clone());
                let Ok(blob_data) = repo.cas().get_blob(&hash) else {
                    continue;
                };
                blobs.push(BlobRef {
                    hash: hex_to_hash_proto(&hash_hex),
                    data: b64.encode(&blob_data),
                });
            }
        } else if !patch.payload.is_empty() {
            let hash_hex = String::from_utf8_lossy(&patch.payload).to_string();
            if seen_hashes.contains(&hash_hex) {
                continue;
            }
            let Ok(hash) = suture_common::Hash::from_hex(&hash_hex) else {
                continue;
            };
            seen_hashes.insert(hash_hex.clone());
            let Ok(blob_data) = repo.cas().get_blob(&hash) else {
                continue;
            };
            blobs.push(BlobRef {
                hash: hex_to_hash_proto(&hash_hex),
                data: b64.encode(&blob_data),
            });
        }
    }

    let known_branches = repo
        .list_branches()
        .iter()
        .map(|(name, target_id)| BranchProto {
            name: name.clone(),
            target_id: hex_to_hash_proto(&target_id.to_hex()),
        })
        .collect();

    let push_body = PushRequest {
        repo_id: derive_repo_id(&url, remote),
        patches: patches.iter().map(patch_to_proto).collect(),
        branches: branches_to_push
            .iter()
            .map(|(name, target_id)| BranchProto {
                name: name.clone(),
                target_id: hex_to_hash_proto(&target_id.to_hex()),
            })
            .collect(),
        blobs,
        signature: None,
        known_branches,
        force,
    };

    let push_body = sign_push_request(&repo, push_body)
        .map_err(|e| user_error("failed to sign push request", e))?;

    eprintln!(
        "Pushing {} patch(es), {} blob(s) to {}...",
        patches.len(),
        push_body.blobs.len(),
        remote
    );

    let (branch_display, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_owned(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_owned(), branch_display);
    pre_extra.insert("SUTURE_HEAD".to_owned(), head_id.to_hex());
    pre_extra.insert("SUTURE_PUSH_REMOTE".to_owned(), remote.to_owned());
    pre_extra.insert("SUTURE_PUSH_PATCHES".to_owned(), patches.len().to_string());
    run_hook_if_exists(repo.root(), "pre-push", pre_extra)?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{url}/push"))
        .json(&push_body)
        .send()
        .await
        .map_err(|e| user_error(&format!("network error pushing to '{remote}'"), e))?;

    if resp.status().is_success() {
        let result: PushResponse = resp
            .json()
            .await
            .map_err(|e| user_error("failed to parse push response", e))?;
        if result.success {
            let (_, head_id) = repo
                .head()
                .map_err(|e| user_error("failed to get HEAD after push", e))?;
            repo.set_config(&push_state_key, &head_id.to_hex())
                .map_err(|e| user_error("failed to save push state", e))?;
            println!("Push successful ({} patch(es))", patches.len());

            let (branch_display, head_id) = repo.head()?;
            let mut post_extra = std::collections::HashMap::new();
            post_extra.insert("SUTURE_BRANCH".to_owned(), branch_display);
            post_extra.insert("SUTURE_HEAD".to_owned(), head_id.to_hex());
            post_extra.insert("SUTURE_PUSH_REMOTE".to_owned(), remote.to_owned());
            post_extra.insert("SUTURE_PUSH_PATCHES".to_owned(), patches.len().to_string());
            run_hook_if_exists(repo.root(), "post-push", post_extra)?;
        } else {
            return Err(format!("push rejected: {}", result.error.unwrap_or_default()).into());
        }
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("push failed (HTTP {status}): {text}").into());
    }

    Ok(())
}
