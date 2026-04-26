use crate::cmd::user_error;
use crate::style::run_hook_if_exists;
use ed25519_dalek::Signer;

pub(crate) async fn cmd_commit(message: &str, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    if message.trim().is_empty() {
        return Err("commit message cannot be empty".into());
    }

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;
    if all {
        let count = repo
            .add_all()
            .map_err(|e| user_error("failed to stage files", e))?;
        if count > 0 {
            println!("Staged {} file(s)", count);
        }
    }

    let status = repo
        .status()
        .map_err(|e| user_error("failed to check repository status", e))?;
    if status.staged_files.is_empty() {
        return Err("nothing to commit (use 'suture add' to stage files)".into());
    }

    // Run pre-commit hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let author = repo
        .get_config("user.name")
        .unwrap_or(None)
        .unwrap_or_default();
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_AUTHOR".to_string(), author);
    extra.insert("SUTURE_BRANCH".to_string(), branch.clone());
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    run_hook_if_exists(repo.root(), "pre-commit", extra)?;

    let patch_id = repo
        .commit(message)
        .map_err(|e| user_error("failed to create commit", e))?;
    println!("Committed: {}", patch_id);

    {
        let author = repo
            .get_config("user.name")
            .unwrap_or(None)
            .unwrap_or_default();
        let audit_dir = repo.root().join(".suture").join("audit").join("chain.log");
        let audit = suture_core::audit::AuditLog::open(&audit_dir)
            .map_err(|e| user_error("failed to open audit log", e))?;
        let patch = repo.dag().get_patch(&patch_id);
        let touch_set: Vec<String> = patch
            .as_ref()
            .map(|p| p.touch_set.addresses())
            .unwrap_or_default();
        let parent_ids: Vec<String> = patch
            .as_ref()
            .map(|p| p.parent_ids.iter().map(|id| id.to_hex()).collect())
            .unwrap_or_default();
        let details = serde_json::json!({
            "patch_id": patch_id.to_hex(),
            "files": touch_set,
            "message": message,
            "parents": parent_ids,
        })
        .to_string();
        let _ = audit.append(&author, "commit", &details);
    }

    if let Ok(Some(key_name)) = repo.get_config("signing.key") {
        // Validate key_name to prevent path traversal
        if key_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            let key_path = std::path::Path::new(".suture")
                .join("keys")
                .join(format!("{key_name}.ed25519"));
            if let Ok(key_bytes) = std::fs::read(&key_path)
                && key_bytes.len() == 32
            {
                let signing_key =
                    ed25519_dalek::SigningKey::from_bytes(key_bytes.as_slice().try_into().unwrap());
                let patch = repo.dag().get_patch(&patch_id);
                if let Some(patch) = patch {
                    let canonical = suture_core::signing::canonical_patch_bytes(
                        &patch.operation_type.to_string(),
                        &patch.touch_set.addresses(),
                        &patch.target_path,
                        &patch.payload,
                        &patch.parent_ids,
                        &patch.author,
                        &patch.message,
                        patch.timestamp,
                    );
                    let sig = signing_key.sign(&canonical);
                    let _ = repo
                        .meta()
                        .store_signature(&patch.id.to_hex(), &sig.to_bytes());
                    let _ = repo
                        .meta()
                        .store_public_key(&patch.author, &signing_key.verifying_key().to_bytes());
                }
            }
        }
    }

    // Run post-commit hook
    let (branch, head_id) = repo.head()?;
    let author = repo
        .get_config("user.name")
        .unwrap_or(None)
        .unwrap_or_default();
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_AUTHOR".to_string(), author);
    extra.insert("SUTURE_BRANCH".to_string(), branch);
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    extra.insert("SUTURE_COMMIT".to_string(), patch_id.to_hex());
    run_hook_if_exists(repo.root(), "post-commit", extra)?;

    Ok(())
}
