pub async fn cmd_fsck(full: bool) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.fsck()?;

    println!("Repository integrity check complete.");
    println!("  {} check(s) passed", result.checks_passed);

    if full {
        let mut full_checks = 0usize;
        let mut full_issues = Vec::new();

        let all_patches = repo.all_patches();
        let all_ids: std::collections::HashSet<suture_common::Hash> =
            repo.dag().patch_ids().into_iter().collect();

        for patch in &all_patches {
            if !patch.payload.is_empty()
                && let Ok(hex_str) = std::str::from_utf8(&patch.payload)
                && let Ok(expected_hash) = suture_common::Hash::from_hex(hex_str)
                && repo.cas().has_blob(&expected_hash)
            {
                if let Ok(blob_data) = repo.cas().get_blob(&expected_hash) {
                    use suture_core::cas::hasher;
                    let actual_hash = hasher::hash_bytes(&blob_data);
                    if actual_hash != expected_hash {
                        full_issues.push(format!(
                            "blob {} has integrity mismatch for patch {}",
                            expected_hash.to_hex(),
                            patch.id.to_hex()
                        ));
                    }
                }
                full_checks += 1;
            }
        }

        for patch in &all_patches {
            for parent_id in &patch.parent_ids {
                if parent_id != &suture_common::Hash::ZERO && !all_ids.contains(parent_id) {
                    full_issues.push(format!(
                        "patch {} references missing parent {}",
                        patch.id.to_hex(),
                        parent_id.to_hex()
                    ));
                }
            }
        }

        let branches = repo.list_branches();
        for (name, target_id) in &branches {
            if !all_ids.contains(target_id) {
                full_issues.push(format!(
                    "branch '{}' points to non-existent patch {}",
                    name,
                    target_id.to_hex()
                ));
            }
        }

        if full_checks > 0 {
            println!("  Full: {full_checks} blob integrity check(s) passed");
        }
        if !full_issues.is_empty() {
            println!("\nFull check issues:");
            for issue in &full_issues {
                println!("  ISSUE: {issue}");
            }
        } else if full_checks > 0 {
            println!("  Full: all integrity checks passed");
        }
    }

    if !result.warnings.is_empty() {
        println!("\nWarnings:");
        for w in &result.warnings {
            println!("  WARNING: {w}");
        }
    }
    if !result.errors.is_empty() {
        println!("\nErrors:");
        for e in &result.errors {
            println!("  ERROR: {e}");
        }
    }
    Ok(())
}
