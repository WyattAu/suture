use crate::remote_proto::{
    BlobRef, BranchProto, PushRequest, PushResponse, check_handshake, derive_repo_id,
    hex_to_hash_proto, patch_to_proto, sign_push_request,
};
use crate::style::run_hook_if_exists;
use base64::Engine;

pub(crate) async fn cmd_push(
    remote: &str,
    force: bool,
    branch: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let url = repo.get_remote_url(remote)?;

    check_handshake(&url).await?;

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
            return Err(format!("branch '{}' not found", branch_name).into());
        }
        return Err("no branches to push".into());
    }

    let push_state_key = format!("remote.{}.last_pushed", remote);
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
        if !patch.payload.is_empty() {
            let hash_hex = String::from_utf8_lossy(&patch.payload).to_string();
            let Ok(hash) = suture_common::Hash::from_hex(&hash_hex) else {
                continue;
            };
            if !seen_hashes.contains(&hash_hex) {
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

    let push_body = sign_push_request(&repo, push_body)?;

    let (branch_display, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_string(), branch_display);
    pre_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    pre_extra.insert("SUTURE_PUSH_REMOTE".to_string(), remote.to_string());
    pre_extra.insert("SUTURE_PUSH_PATCHES".to_string(), patches.len().to_string());
    run_hook_if_exists(repo.root(), "pre-push", pre_extra)?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/push", url))
        .json(&push_body)
        .send()
        .await?;

    if resp.status().is_success() {
        let result: PushResponse = resp.json().await?;
        if result.success {
            let (_, head_id) = repo.head()?;
            repo.set_config(&push_state_key, &head_id.to_hex())?;
            println!("Push successful ({} patch(es))", patches.len());

            let (branch_display, head_id) = repo.head()?;
            let mut post_extra = std::collections::HashMap::new();
            post_extra.insert("SUTURE_BRANCH".to_string(), branch_display);
            post_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
            post_extra.insert("SUTURE_PUSH_REMOTE".to_string(), remote.to_string());
            post_extra.insert("SUTURE_PUSH_PATCHES".to_string(), patches.len().to_string());
            run_hook_if_exists(repo.root(), "post-push", post_extra)?;
        } else {
            eprintln!("Push failed: {:?}", result.error);
        }
    } else {
        let text = resp.text().await?;
        eprintln!("Push failed: {}", text);
    }

    Ok(())
}
