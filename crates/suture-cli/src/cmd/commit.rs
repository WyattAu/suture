use crate::style::run_hook_if_exists;
use ed25519_dalek::Signer;

pub(crate) async fn cmd_commit(message: &str, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    if all {
        let count = repo.add_all()?;
        if count > 0 {
            println!("Staged {} file(s)", count);
        }
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

    let patch_id = repo.commit(message)?;
    println!("Committed: {}", patch_id);

    if let Ok(Some(key_name)) = repo.get_config("signing.key") {
        let key_path = std::path::Path::new(".suture")
            .join("keys")
            .join(format!("{key_name}.ed25519"));
        if let Ok(key_bytes) = std::fs::read(&key_path)
            && key_bytes.len() == 32
        {
            let signing_key = ed25519_dalek::SigningKey::from_bytes(
                key_bytes.as_slice().try_into().unwrap(),
            );
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
                let _ = repo.meta().store_signature(
                    &patch.id.to_hex(),
                    &sig.to_bytes(),
                );
                let _ = repo.meta().store_public_key(
                    &patch.author,
                    &signing_key.verifying_key().to_bytes(),
                );
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
