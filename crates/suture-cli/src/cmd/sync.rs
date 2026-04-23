use std::path::Path;

use crate::remote_proto::do_pull;

pub(crate) async fn cmd_sync(
    remote: &str,
    no_push: bool,
    pull_only: bool,
    message: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(Path::new("."))?;
    let has_remote = has_configured_remote(&repo, remote);

    let mut pulled = false;
    let mut pull_count = 0;

    if has_remote {
        eprintln!("Pulling from {}...", remote);
        match do_pull(&mut repo, remote).await {
            Ok(count) => {
                if count > 0 {
                    pulled = true;
                    pull_count = count;
                }
            }
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                if msg.contains("conflict") || msg.contains("merge") {
                    return Err(format!("merge conflict during pull: {e}").into());
                }
                return Err(format!("pull failed: {e}").into());
            }
        }
    }

    if pull_only {
        if pull_count > 0 {
            println!("Pulled {pull_count} patches from {remote}");
        } else {
            println!("Already up to date.");
        }
        return Ok(());
    }

    let changed_files = detect_changed_files(&repo)?;
    if changed_files.is_empty() && !pulled {
        println!("Everything up to date.");
        return Ok(());
    }

    let committed_files = if !changed_files.is_empty() {
        let count = repo.add_all()?;
        if count == 0 {
            println!("Everything up to date.");
            return Ok(());
        }

        let msg = match message {
            Some(m) => m.to_string(),
            None => generate_sync_message(&changed_files),
        };

        let patch_id = repo.commit(&msg)?;
        println!(
            "Committed {} change{}:",
            changed_files.len(),
            if changed_files.len() == 1 { "" } else { "s" }
        );
        for path in &changed_files {
            let icon = file_type_icon(path);
            let filename = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            println!("  {icon} {filename} (modified)");
        }
        println!("  ({})", &patch_id.to_hex()[..12]);

        Some(changed_files)
    } else {
        None
    };

    if has_remote && !no_push {
        let (branch, _) = repo.head().unwrap_or(("main".to_string(), suture_common::Hash::ZERO));
        match cmd_push_inner(&mut repo, remote).await {
            Ok(()) => {
                println!("Pushed to {remote}/{branch}");
            }
            Err(e) => {
                eprintln!("Push failed: {e}");
                eprintln!("Changes are committed locally.");
            }
        }
    }

    if pulled {
        println!("Pulled {pull_count} patches from {remote}");
    }

    if let Some(files) = committed_files {
        if !has_remote {
            println!("\nNo remote configured. Changes committed locally only.");
            println!("Run `suture remote add <name> <url>` to enable push/pull.");
        }
        let _ = files;
    }

    Ok(())
}

fn has_configured_remote(
    repo: &suture_core::repository::Repository,
    name: &str,
) -> bool {
    let remotes = repo.list_remotes().unwrap_or_default();
    remotes.iter().any(|(n, _)| n == name)
}

fn detect_changed_files(
    repo: &suture_core::repository::Repository,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut changed = Vec::new();

    let head_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());

    let repo_dir = Path::new(".");
    let disk_files = crate::display::walk_repo_files(repo_dir);

    for rel_path in &disk_files {
        let full_path = repo_dir.join(rel_path);
        if let Ok(data) = std::fs::read(&full_path) {
            let current_hash = suture_common::Hash::from_data(&data);
            if let Some(head_hash) = head_tree.get(rel_path) {
                if &current_hash != head_hash {
                    changed.push(rel_path.clone());
                }
            } else {
                changed.push(rel_path.clone());
            }
        }
    }

    for (path, _) in head_tree.iter() {
        if !disk_files.iter().any(|f| f == path) {
            changed.push(path.clone());
        }
    }

    Ok(changed)
}

fn generate_sync_message(changed_files: &[String]) -> String {
    let file_count = changed_files.len();
    if file_count == 0 {
        return "Sync: no changes".to_string();
    }

    if file_count == 1 {
        let filename = Path::new(&changed_files[0])
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&changed_files[0]);
        return format!("Sync: update {filename}");
    }

    let doc_count = changed_files
        .iter()
        .filter(|f| {
            let ext = Path::new(f)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            matches!(ext.as_str(), "docx" | "pdf" | "md" | "html" | "htm")
        })
        .count();
    let xls_count = changed_files
        .iter()
        .filter(|f| {
            let ext = Path::new(f)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            matches!(ext.as_str(), "xlsx" | "csv")
        })
        .count();
    let pptx_count = changed_files
        .iter()
        .filter(|f| {
            let ext = Path::new(f)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            ext == "pptx"
        })
        .count();
    let other_count = file_count - doc_count - xls_count - pptx_count;

    let mut parts = Vec::new();
    if doc_count > 0 {
        parts.push(format!(
            "{} document{}",
            doc_count,
            plural(doc_count)
        ));
    }
    if xls_count > 0 {
        parts.push(format!(
            "{} spreadsheet{}",
            xls_count,
            plural(xls_count)
        ));
    }
    if pptx_count > 0 {
        parts.push(format!(
            "{} presentation{}",
            pptx_count,
            plural(pptx_count)
        ));
    }
    if other_count > 0 {
        parts.push(format!("{} file{}", other_count, plural(other_count)));
    }

    format!("Sync: update {}", parts.join(", "))
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn file_type_icon(path: &str) -> &'static str {
    suture_core::file_type::detect_file_type(Path::new(path)).icon()
}

async fn cmd_push_inner(
    repo: &mut suture_core::repository::Repository,
    remote: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::remote_proto::{
        BranchProto, PushRequest, PushResponse, check_handshake, derive_repo_id,
        hex_to_hash_proto, patch_to_proto, sign_push_request,
    };
    use base64::Engine;

    let url = repo.get_remote_url(remote)?;
    check_handshake(&url).await?;

    let (branch_name, _) = repo.head()?;
    let branches = repo.list_branches();
    let branches_to_push: Vec<(String, suture_common::Hash)> = branches
        .into_iter()
        .filter(|(n, _)| *n == branch_name)
        .collect();

    if branches_to_push.is_empty() {
        return Err(format!("branch '{}' not found", branch_name).into());
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
                blobs.push(crate::remote_proto::BlobRef {
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
            blobs.push(crate::remote_proto::BlobRef {
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
        force: false,
    };

    let push_body = sign_push_request(repo, push_body)?;

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
            Ok(())
        } else {
            Err(format!("server rejected push: {:?}", result.error).into())
        }
    } else {
        let text = resp.text().await?;
        Err(format!("push failed: {text}").into())
    }
}
