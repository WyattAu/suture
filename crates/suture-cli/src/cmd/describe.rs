use crate::ref_utils::resolve_ref;

pub(crate) async fn cmd_describe(
    commit_ref: &str,
    _all: bool,
    _tags: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit_ref, &patches)?;

    let tags = repo.list_tags()?;

    if let Some(tag_name) = tags.iter().find_map(|(name, id)| {
        if *id == target.id {
            Some(name.clone())
        } else {
            None
        }
    }) {
        println!("{tag_name}");
        return Ok(());
    }

    let commit_ancestors = repo.dag().ancestors(&target.id);
    let mut all_reachable: std::collections::HashSet<suture_common::Hash> =
        commit_ancestors.into_iter().collect();
    all_reachable.insert(target.id);

    let mut best_tag: Option<(String, usize)> = None;

    for (tag_name, tag_id) in &tags {
        if !all_reachable.contains(tag_id) {
            continue;
        }

        let mut count = 0usize;
        let mut current_id = target.id;
        while current_id != *tag_id {
            let patch = patches
                .iter()
                .find(|p| p.id == current_id)
                .ok_or_else(|| "patch not found while walking ancestors".to_string())?;
            current_id = *patch
                .parent_ids
                .first()
                .ok_or_else(|| "reached root without finding tag".to_string())?;
            count += 1;
        }

        if best_tag.is_none() || count < best_tag.as_ref().unwrap().1 {
            best_tag = Some((tag_name.clone(), count));
        }
    }

    if let Some((tag_name, count)) = best_tag {
        let short_hash = target.id.to_hex().chars().take(8).collect::<String>();
        println!("{}-{}-g{}", tag_name, count, short_hash);
    } else {
        let short_hash = target.id.to_hex().chars().take(8).collect::<String>();
        println!("{short_hash}");
    }

    Ok(())
}
