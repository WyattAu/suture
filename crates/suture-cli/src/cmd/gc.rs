pub(crate) async fn cmd_gc(
    dry_run: bool,
    aggressive: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let branches = repo.dag().list_branches();
    let all_ids: std::collections::HashSet<suture_common::Hash> =
        repo.dag().patch_ids().into_iter().collect();

    let mut reachable: std::collections::HashSet<suture_common::Hash> =
        std::collections::HashSet::new();
    for (_name, tip_id) in &branches {
        reachable.insert(*tip_id);
        for anc in repo.dag().ancestors(tip_id).iter() {
            reachable.insert(*anc);
        }
    }

    let unreachable_count = all_ids.iter().filter(|id| !reachable.contains(id)).count();

    let mut referenced_blobs: std::collections::HashSet<suture_common::Hash> =
        std::collections::HashSet::new();
    for id in &reachable {
        if let Some(patch) = repo.dag().get_patch(id) {
            for addr in patch.touch_set.addresses() {
                let _ = addr;
            }
            if !patch.payload.is_empty()
                && let Ok(hex_str) = std::str::from_utf8(&patch.payload)
                && let Ok(hash) = suture_common::Hash::from_hex(hex_str)
            {
                referenced_blobs.insert(hash);
            }
        }
    }

    let all_blobs = repo.cas().list_blobs().unwrap_or_default();
    let orphan_count = all_blobs
        .iter()
        .filter(|b| !referenced_blobs.contains(b))
        .count();

    let total_size = repo.cas().total_size().unwrap_or(0);
    let estimated_bytes = if orphan_count > 0 && !all_blobs.is_empty() {
        total_size / all_blobs.len() as u64 * orphan_count as u64
    } else {
        0
    };

    if dry_run {
        println!("Dry run: would prune:");
        println!("  {} unreachable patch(es)", unreachable_count);
        println!("  {} orphaned blob(s)", orphan_count);
        println!("  ~{} bytes estimated", estimated_bytes);

        if aggressive {
            let entries = repo.reflog_entries().unwrap_or_default();
            let now_ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let cutoff = now_ts - (90 * 24 * 3600);
            let old_entries: Vec<_> = entries.iter().filter(|e| e.timestamp < cutoff).collect();
            println!("  {} reflog entries older than 90 days", old_entries.len());
        }
        return Ok(());
    }

    let result = repo.gc()?;

    if aggressive {
        let entries = repo.reflog_entries().unwrap_or_default();
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let cutoff = now_ts - (90 * 24 * 3600);
        let old_count = entries.iter().filter(|e| e.timestamp < cutoff).count();
        if old_count > 0 {
            let _ = repo.meta().reflog_clear();
        }

        let _ = repo.cas().repack(10);
    }

    println!(
        "Cleaned {} unreachable patches, {} orphan blobs, freed ~{} bytes",
        result.patches_removed, result.blobs_removed, estimated_bytes
    );

    if result.patches_removed > 0 || result.blobs_removed > 0 {
        println!("  Hint: reopen the repository to fully update the in-memory DAG");
    }
    Ok(())
}
