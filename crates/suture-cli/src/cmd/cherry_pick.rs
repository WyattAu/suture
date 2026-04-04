use crate::ref_utils::resolve_ref;
use crate::style::run_hook_if_exists;

pub(crate) async fn cmd_cherry_pick(commit: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit, &patches)?;
    let patch_id = target.id;

    // Run pre-cherry-pick hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_BRANCH".to_string(), branch);
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    extra.insert("SUTURE_CHERRY_PICK_TARGET".to_string(), patch_id.to_hex());
    run_hook_if_exists(repo.root(), "pre-cherry-pick", extra)?;

    let new_id = repo.cherry_pick(&patch_id)?;
    println!("Cherry-picked {} as {}", commit, new_id);
    Ok(())
}
