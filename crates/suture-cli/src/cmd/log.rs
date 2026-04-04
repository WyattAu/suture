use crate::ref_utils::parse_time_filter;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn cmd_log(
    branch: Option<&str>,
    graph: bool,
    first_parent: bool,
    oneline: bool,
    author: Option<&str>,
    grep: Option<&str>,
    all: bool,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let since_ts = since.map(parse_time_filter).transpose()?;
    let until_ts = until.map(parse_time_filter).transpose()?;

    let show_graph = graph && !all;

    if !show_graph {
        let mut patches = if all {
            let branches = repo.list_branches();
            let mut seen = std::collections::HashSet::new();
            let mut all_patches = Vec::new();
            for (_, tip_id) in &branches {
                let chain = repo.dag().patch_chain(tip_id);
                for pid in &chain {
                    if seen.insert(*pid)
                        && let Some(patch) = repo.dag().get_patch(pid)
                    {
                        all_patches.push(patch.clone());
                    }
                }
            }
            all_patches.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            all_patches
        } else if first_parent {
            use suture_common::Hash;
            let _branch_name = branch.unwrap_or("HEAD");
            let (_head_branch, head_id) = repo
                .head()
                .unwrap_or_else(|_| ("main".to_string(), Hash::ZERO));
            let mut chain = Vec::new();
            let mut current = head_id;
            while current != Hash::ZERO {
                chain.push(current);
                if let Some(patch) = repo.dag().get_patch(&current) {
                    current = patch.parent_ids.first().copied().unwrap_or(Hash::ZERO);
                } else {
                    break;
                }
            }
            let mut patches = Vec::new();
            for pid in &chain {
                if let Some(patch) = repo.dag().get_patch(pid) {
                    patches.push(patch.clone());
                }
            }
            patches
        } else {
            repo.log_all(branch)?
        };

        if let Some(since) = since_ts {
            patches.retain(|p| p.timestamp >= since);
        }
        if let Some(until) = until_ts {
            patches.retain(|p| p.timestamp <= until);
        }
        if let Some(author_filter) = author {
            patches.retain(|p| p.author.contains(author_filter));
        }
        if let Some(grep_filter) = grep {
            let grep_lower = grep_filter.to_lowercase();
            patches.retain(|p| p.message.to_lowercase().contains(&grep_lower));
        }

        if patches.is_empty() {
            println!("No commits.");
            return Ok(());
        }

        if oneline {
            for patch in &patches {
                let short_hash = patch.id.to_hex().chars().take(8).collect::<String>();
                println!("{} {}", short_hash, patch.message);
            }
            return Ok(());
        }

        for (i, patch) in patches.iter().enumerate() {
            if i == 0 {
                println!("* {} {}", patch.id.to_hex(), patch.message);
            } else {
                println!("  {} {}", patch.id.to_hex(), patch.message);
            }
        }

        return Ok(());
    }

    let branches = repo.list_branches();
    if branches.is_empty() {
        println!("No commits.");
        return Ok(());
    }

    let all_patches = repo.all_patches();
    let mut commit_groups: Vec<(Vec<suture_core::patch::types::PatchId>, String, u64)> = Vec::new();
    let mut seen_messages: std::collections::HashMap<(String, u64), usize> =
        std::collections::HashMap::new();

    for patch in &all_patches {
        let key = (patch.message.clone(), patch.timestamp);
        if let Some(&idx) = seen_messages.get(&key) {
            commit_groups[idx].0.push(patch.id);
        } else {
            seen_messages.insert(key, commit_groups.len());
            commit_groups.push((vec![patch.id], patch.message.clone(), patch.timestamp));
        }
    }

    commit_groups.sort_by(|a, b| b.2.cmp(&a.2));

    let branch_tips: std::collections::HashSet<suture_core::patch::types::PatchId> =
        branches.iter().map(|(_, id)| *id).collect();

    let tip_list: Vec<_> = branches.iter().collect();
    let mut col_assign: std::collections::HashMap<suture_core::patch::types::PatchId, usize> =
        std::collections::HashMap::new();
    for (i, (_, id)) in tip_list.iter().enumerate() {
        col_assign.insert(*id, i);
    }

    let mut next_col = tip_list.len();

    let num_cols = tip_list.len() + 5;
    for (patch_ids, message, _ts) in &commit_groups {
        let mut row = vec![' '; num_cols];
        let mut used_cols: std::collections::HashSet<usize> = std::collections::HashSet::new();

        for pid in patch_ids {
            if let Some(&col) = col_assign.get(pid) {
                row[col] = '*';
                used_cols.insert(col);
            } else {
                row[next_col % num_cols] = '*';
                used_cols.insert(next_col % num_cols);
                col_assign.insert(*pid, next_col % num_cols);
                next_col += 1;
            }
        }

        let is_tip = patch_ids.iter().any(|pid| branch_tips.contains(pid));
        if !is_tip {
            for &col in &used_cols {
                row[col] = '|';
            }
        }

        let row_str: String = row.iter().collect();
        let short_hash = if let Some(pid) = patch_ids.first() {
            pid.to_hex().chars().take(8).collect()
        } else {
            "????????".to_string()
        };

        let labels: Vec<String> = branches
            .iter()
            .filter(|(_, id)| patch_ids.contains(id))
            .map(|(name, _)| name.clone())
            .collect();
        let label_str = if labels.is_empty() {
            String::new()
        } else {
            format!(" ({})", labels.join(", "))
        };

        println!("{} {} {}{}", row_str, short_hash, message, label_str);
    }

    Ok(())
}
