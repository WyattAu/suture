use crate::ref_utils::resolve_ref;

pub async fn cmd_rollback(commit: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit, &patches)?;
    let patch_id = target.id;

    if target.parent_ids.is_empty() {
        return Err("cannot rollback root commit".into());
    }

    let short_hash = &patch_id.to_hex()[..8];
    let original_msg = target.message.clone();
    let rollback_msg = format!("Rollback {short_hash}: {original_msg}");

    let revert_id = repo.revert(&patch_id, Some(&rollback_msg))?;

    println!("Rolled back {}: {}", short_hash, &revert_id.to_hex()[..8]);
    Ok(())
}
