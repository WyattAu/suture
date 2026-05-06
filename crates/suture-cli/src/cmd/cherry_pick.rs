use crate::ref_utils::resolve_ref;
use crate::style::run_hook_if_exists;

pub async fn cmd_cherry_pick(
    commit: &str,
    no_commit: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit, &patches)?;
    let patch_id = target.id;

    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_owned(), suture_common::Hash::ZERO));
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_BRANCH".to_owned(), branch);
    extra.insert("SUTURE_HEAD".to_owned(), head_id.to_hex());
    extra.insert("SUTURE_CHERRY_PICK_TARGET".to_owned(), patch_id.to_hex());
    run_hook_if_exists(repo.root(), "pre-cherry-pick", extra)?;

    if no_commit {
        let patch = repo
            .dag()
            .get_patch(&patch_id)
            .ok_or_else(|| format!("patch not found: {patch_id}"))?;

        let old_head = repo
            .head()
            .map(|(_, id)| id)
            .unwrap_or(suture_common::Hash::ZERO);
        let old_tree = repo
            .snapshot(&old_head)
            .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());

        let (_, _) = repo.head()?;
        let touch_files = patch.touch_set.addresses();
        for file_path in &touch_files {
            if repo.root().join(file_path).exists() {
                repo.add(file_path)?;
            }
        }

        let _ = old_tree;
        println!("Cherry-picked changes from {commit} staged (no commit created)");
        Ok(())
    } else {
        let new_id = repo.cherry_pick(&patch_id)?;
        println!("Cherry-picked {commit} as {new_id}");
        Ok(())
    }
}
