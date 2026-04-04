use crate::style::run_hook_if_exists;

pub(crate) async fn cmd_merge(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path as StdPath;
    use suture_core::repository::ConflictInfo;

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    // Run pre-merge hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_string(), branch.clone());
    pre_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    pre_extra.insert("SUTURE_MERGE_SOURCE".to_string(), source.to_string());
    run_hook_if_exists(repo.root(), "pre-merge", pre_extra)?;

    let result = repo.execute_merge(source)?;

    if result.is_clean {
        if let Some(id) = result.merge_patch_id {
            println!("Merge successful: {}", id);
        }
        if result.patches_applied > 0 {
            println!(
                "Applied {} patch(es) from '{}'",
                result.patches_applied, source
            );
        }
        // Run post-merge hook (only on clean merge)
        let (branch, head_id) = repo.head()?;
        let mut post_extra = std::collections::HashMap::new();
        post_extra.insert("SUTURE_BRANCH".to_string(), branch);
        post_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
        post_extra.insert("SUTURE_MERGE_SOURCE".to_string(), source.to_string());
        run_hook_if_exists(repo.root(), "post-merge", post_extra)?;

        return Ok(());
    }

    let conflicts = result.unresolved_conflicts;
    let mut remaining: Vec<ConflictInfo> = Vec::new();
    let mut resolved_count = 0usize;

    {
        use suture_driver::DriverRegistry;
        use suture_driver_csv::CsvDriver;
        use suture_driver_docx::DocxDriver;
        use suture_driver_json::JsonDriver;
        use suture_driver_pptx::PptxDriver;
        use suture_driver_toml::TomlDriver;
        use suture_driver_xlsx::XlsxDriver;
        use suture_driver_xml::XmlDriver;
        use suture_driver_yaml::YamlDriver;

        let mut registry = DriverRegistry::new();
        registry.register(Box::new(JsonDriver));
        registry.register(Box::new(TomlDriver));
        registry.register(Box::new(CsvDriver));
        registry.register(Box::new(YamlDriver));
        registry.register(Box::new(XmlDriver));
        registry.register(Box::new(DocxDriver));
        registry.register(Box::new(XlsxDriver));
        registry.register(Box::new(PptxDriver));

        for conflict in &conflicts {
            let path = StdPath::new(&conflict.path);
            let Ok(driver) = registry.get_for_path(path) else {
                remaining.push(conflict.clone());
                continue;
            };

            let base_content = conflict
                .base_content_hash
                .and_then(|h| repo.cas().get_blob(&h).ok())
                .map(|b| String::from_utf8_lossy(&b).to_string());
            let ours_content = conflict
                .our_content_hash
                .and_then(|h| repo.cas().get_blob(&h).ok())
                .map(|b| String::from_utf8_lossy(&b).to_string());
            let theirs_content = conflict
                .their_content_hash
                .and_then(|h| repo.cas().get_blob(&h).ok())
                .map(|b| String::from_utf8_lossy(&b).to_string());

            let base_str = base_content.as_deref().unwrap_or("");
            let Some(ours_str) = ours_content.as_deref() else {
                remaining.push(conflict.clone());
                continue;
            };
            let Some(theirs_str) = theirs_content.as_deref() else {
                remaining.push(conflict.clone());
                continue;
            };

            let Ok(merged) = driver.merge(base_str, ours_str, theirs_str) else {
                remaining.push(conflict.clone());
                continue;
            };
            let Some(content) = merged else {
                remaining.push(conflict.clone());
                continue;
            };

            if let Err(e) = std::fs::write(&conflict.path, &content) {
                eprintln!(
                    "Warning: could not write resolved file '{}': {e}",
                    conflict.path
                );
                remaining.push(conflict.clone());
                continue;
            }

            if let Err(e) = repo.add(&conflict.path) {
                eprintln!(
                    "Warning: could not stage resolved file '{}': {e}",
                    conflict.path
                );
                remaining.push(conflict.clone());
                continue;
            }

            resolved_count += 1;
        }
    }

    if resolved_count > 0 {
        println!("Resolved {resolved_count} conflict(s) via semantic drivers");
    }

    if remaining.is_empty() {
        println!("All conflicts resolved via semantic drivers.");
        println!("Run `suture commit` to finalize the merge.");
    } else {
        println!("Merge has {} conflict(s):", remaining.len());
        for conflict in &remaining {
            println!(
                "  CONFLICT in '{}': edit the file, then commit to resolve",
                conflict.path
            );
        }
        println!("Hint: resolve conflicts, then run `suture commit`");
    }

    Ok(())
}
