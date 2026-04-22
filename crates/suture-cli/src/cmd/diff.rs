use crate::display::format_line_diff;
use crate::style::{ANSI_BOLD_CYAN, ANSI_RESET};

const BINARY_EXTENSIONS: &[&str] = &[
    ".docx", ".xlsx", ".pptx",
    ".pdf",
    ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".tiff", ".tif", ".ico", ".avif",
    ".svg",
];

fn is_binary_format(path: &str) -> bool {
    let lower = path.to_lowercase();
    BINARY_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

fn blob_to_string_preserve_bytes(blob: &[u8]) -> String {
    unsafe { String::from_utf8_unchecked(blob.to_vec()) }
}

fn blob_to_string(blob: &[u8], path: &str) -> String {
    if is_binary_format(path) {
        blob_to_string_preserve_bytes(blob)
    } else {
        String::from_utf8_lossy(blob).into_owned()
    }
}

pub(crate) async fn cmd_diff(
    from: Option<&str>,
    to: Option<&str>,
    cached: bool,
    integrity: bool,
    name_only: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use suture_core::engine::diff::DiffType;
    use suture_core::engine::merge::diff_lines;

    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let entries = if cached {
        repo.diff_staged()?
    } else {
        repo.diff(from, to)?
    };

    if entries.is_empty() {
        println!("No differences.");
        return Ok(());
    }

    if name_only {
        for entry in &entries {
            match &entry.diff_type {
                DiffType::Renamed { old_path, new_path } => {
                    println!("{} -> {}", old_path, new_path);
                }
                _ => {
                    println!("{}", entry.path);
                }
            }
        }
        return Ok(());
    }

    // Integrity analysis mode: show mathematical properties and risk indicators
    if integrity {
        let report = build_integrity_report(&entries, &repo, from, to, cached)?;
        let formatted = suture_core::integrity::format_integrity_report(&report);
        println!("{formatted}");
        return Ok(());
    }

    use std::path::Path as StdPath;

    let registry = crate::driver_registry::builtin_registry();

    for entry in &entries {
        let file_type = suture_core::file_type::detect_file_type(StdPath::new(&entry.path));

        match &entry.diff_type {
            DiffType::Renamed { old_path, new_path } => {
                println!(
                    "{ANSI_BOLD_CYAN}renamed {} → {}{ANSI_RESET}",
                    old_path, new_path
                );
            }
            DiffType::Added => {
                if let Some(new_hash) = &entry.new_hash {
                    let new_blob = repo
                        .cas()
                        .get_blob(new_hash)
                        .ok()
                        .or_else(|| std::fs::read(repo.root().join(&entry.path)).ok());
                    let Some(new_blob) = new_blob else {
                        println!(
                            "{ANSI_BOLD_CYAN}added {} (binary){ANSI_RESET}",
                            entry.path
                        );
                        continue;
                    };
                    let new_str = blob_to_string(&new_blob, &entry.path);

                    if let Ok(driver) = registry.get_for_path(StdPath::new(&entry.path))
                        && let Ok(semantic) = driver.format_diff(None, &new_str)
                        && !semantic.is_empty()
                        && semantic != "no changes"
                    {
                        let formatted = if file_type == suture_core::file_type::FileType::Image {
                            suture_core::diff::semantic_formatter::SemanticDiffFormatter::format_image_diff(
                                &entry.path,
                                None,
                                Some(new_blob.len()),
                                &semantic,
                            )
                        } else {
                            suture_core::diff::semantic_formatter::SemanticDiffFormatter::format(
                                &entry.path,
                                file_type,
                                &semantic,
                            )
                        };
                        println!("\n{ANSI_BOLD_CYAN}{formatted}{ANSI_RESET}");
                        continue;
                    }

                    let new_lines: Vec<&str> = new_str.lines().collect();
                    let changes = diff_lines(&[], &new_lines);
                    format_line_diff(&entry.path, &changes);
                } else {
                    println!("{ANSI_BOLD_CYAN}added {}{ANSI_RESET}", entry.path);
                }
            }
            DiffType::Deleted => {
                if let Some(old_hash) = &entry.old_hash {
                    let Ok(old_blob) = repo.cas().get_blob(old_hash) else {
                        println!(
                            "{ANSI_BOLD_CYAN}deleted {} (binary){ANSI_RESET}",
                            entry.path
                        );
                        continue;
                    };
                    let old_str = blob_to_string(&old_blob, &entry.path);
                    let old_lines: Vec<&str> = old_str.lines().collect();
                    let changes = diff_lines(&old_lines, &[]);
                    format_line_diff(&entry.path, &changes);
                } else {
                    println!("{ANSI_BOLD_CYAN}deleted {}{ANSI_RESET}", entry.path);
                }
            }
            DiffType::Modified => {
                if let (Some(old_hash), Some(new_hash)) = (&entry.old_hash, &entry.new_hash) {
                    let old_blob = repo.cas().get_blob(old_hash).ok();
                    let new_blob = repo
                        .cas()
                        .get_blob(new_hash)
                        .ok()
                        .or_else(|| std::fs::read(repo.root().join(&entry.path)).ok());
                    match (old_blob, new_blob) {
                        (Some(old_blob), Some(new_blob)) => {
                            let old_str = blob_to_string(&old_blob, &entry.path);
                            let new_str = blob_to_string(&new_blob, &entry.path);

                            if let Ok(driver) = registry.get_for_path(StdPath::new(&entry.path))
                                && let Ok(semantic) =
                                    driver.format_diff(Some(&old_str), &new_str)
                                && !semantic.is_empty()
                                && semantic != "no changes"
                            {
                                let formatted = if file_type
                                    == suture_core::file_type::FileType::Image
                                {
                                    suture_core::diff::semantic_formatter::SemanticDiffFormatter::format_image_diff(
                                        &entry.path,
                                        Some(old_blob.len()),
                                        Some(new_blob.len()),
                                        &semantic,
                                    )
                                } else {
                                    suture_core::diff::semantic_formatter::SemanticDiffFormatter::format(
                                        &entry.path,
                                        file_type,
                                        &semantic,
                                    )
                                };
                                println!("\n{ANSI_BOLD_CYAN}{formatted}{ANSI_RESET}");
                                continue;
                            }

                            let old_lines: Vec<&str> = old_str.lines().collect();
                            let new_lines: Vec<&str> = new_str.lines().collect();
                            let changes = diff_lines(&old_lines, &new_lines);
                            format_line_diff(&entry.path, &changes);
                        }
                        _ => {
                            println!(
                                "{ANSI_BOLD_CYAN}modified {} (binary){ANSI_RESET}",
                                entry.path
                            );
                        }
                    }
                } else {
                    println!("{ANSI_BOLD_CYAN}modified {}{ANSI_RESET}", entry.path);
                }
            }
        }
    }

    Ok(())
}

/// Build an integrity report from diff entries by reading actual file contents.
fn build_integrity_report(
    entries: &[suture_core::engine::diff::DiffEntry],
    repo: &suture_core::repository::Repository,
    _from: Option<&str>,
    _to: Option<&str>,
    _cached: bool,
) -> Result<suture_core::integrity::DiffIntegrityReport, Box<dyn std::error::Error>> {
    use std::collections::HashMap;
    use suture_core::engine::diff::DiffType;
    use suture_core::integrity::{analyze_file, FileIntegrityReport};

    let mut files = Vec::new();
    let mut old_files: HashMap<String, FileIntegrityReport> = HashMap::new();

    for entry in entries {
        match &entry.diff_type {
            DiffType::Added => {
                let content = get_file_content(repo, &entry.path, entry.new_hash.as_ref());
                let report = analyze_file(&entry.path, &content);
                files.push(report);
            }
            DiffType::Deleted => {
                let content = get_file_content(repo, &entry.path, entry.old_hash.as_ref());
                let report = analyze_file(&entry.path, &content);
                files.push(report);
            }
            DiffType::Modified => {
                // Analyze old version
                let old_content =
                    get_file_content(repo, &entry.path, entry.old_hash.as_ref());
                let old_report = analyze_file(&entry.path, &old_content);
                old_files.insert(entry.path.clone(), old_report);

                // Analyze new version
                let new_content =
                    get_file_content(repo, &entry.path, entry.new_hash.as_ref());
                let mut new_report = analyze_file(&entry.path, &new_content);

                // Check for sudden entropy increase
                if let Some(old) = old_files.get(&entry.path)
                    && new_report.shannon_entropy > old.shannon_entropy + 2.0
                {
                    new_report
                        .risk_indicators
                        .push(suture_core::integrity::RiskIndicator::SuddenEntropyIncrease);
                }

                files.push(new_report);
            }
            DiffType::Renamed { old_path, .. } => {
                // Treat as deleted + added for integrity purposes
                let old_content =
                    get_file_content(repo, old_path, entry.old_hash.as_ref());
                let old_report = analyze_file(old_path, &old_content);
                old_files.insert(entry.path.clone(), old_report);

                let new_content =
                    get_file_content(repo, &entry.path, entry.new_hash.as_ref());
                let new_report = analyze_file(&entry.path, &new_content);
                files.push(new_report);
            }
        }
    }

    // Check for xz-style attack pattern: build script + test infrastructure modified together
    let has_build_script = files.iter().any(|f| {
        f.risk_indicators
            .contains(&suture_core::integrity::RiskIndicator::BuildScriptModified)
    });
    let has_test_infra = files.iter().any(|f| {
        f.risk_indicators
            .contains(&suture_core::integrity::RiskIndicator::TestInfrastructureModified)
    });

    // Check for lockfile modified without corresponding source
    let has_lockfile = files.iter().any(|f| {
        f.risk_indicators
            .contains(&suture_core::integrity::RiskIndicator::LockfileModifiedWithoutSource)
    });
    let has_manifest = files.iter().any(|f| {
        let p = f.path.to_lowercase();
        p.ends_with("cargo.toml")
            || p.ends_with("package.json")
            || p.ends_with("pyproject.toml")
            || p.ends_with("go.mod")
    });
    if has_lockfile && !has_manifest {
        // Flag on the lockfile entry
        for f in &mut files {
            if f.risk_indicators.contains(
                &suture_core::integrity::RiskIndicator::LockfileModifiedWithoutSource,
            ) && !has_manifest
            {
                // Already flagged, that's sufficient
            }
        }
    }

    let mut warnings = Vec::new();
    if has_build_script && has_test_infra {
        let build_paths: Vec<String> = files
            .iter()
            .filter(|f| {
                f.risk_indicators
                    .contains(&suture_core::integrity::RiskIndicator::BuildScriptModified)
            })
            .map(|f| f.path.clone())
            .collect();
        let test_paths: Vec<String> = files
            .iter()
            .filter(|f| {
                f.risk_indicators
                    .contains(&suture_core::integrity::RiskIndicator::TestInfrastructureModified)
            })
            .take(3)
            .map(|f| f.path.clone())
            .collect();
        warnings.push(format!(
            "Build script modified alongside test infrastructure. \
             This pattern was used in the XZ Utils backdoor (CVE-2024-3094). \
             Review: {}",
            build_paths
                .iter()
                .chain(test_paths.iter())
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if has_lockfile && !has_manifest {
        warnings.push(
            "Lockfile modified without corresponding manifest change. \
             Verify no new dependencies were injected."
                .to_string(),
        );
    }

    let high_risk_count = files
        .iter()
        .filter(|f| f.risk_score >= suture_core::integrity::RiskScore::High)
        .count();
    let critical_risk_count = files
        .iter()
        .filter(|f| f.risk_score >= suture_core::integrity::RiskScore::Critical)
        .count();
    let new_binary_files = files
        .iter()
        .filter(|f| f.is_binary)
        .count();
    let build_system_changes = files
        .iter()
        .filter(|f| {
            f.risk_indicators
                .contains(&suture_core::integrity::RiskIndicator::BuildScriptModified)
        })
        .count();

    let total_indicators: usize = files.iter().map(|f| f.risk_indicators.len()).sum();
    let overall_risk = match total_indicators {
        0 => suture_core::integrity::RiskScore::None,
        1 => suture_core::integrity::RiskScore::Low,
        2..=3 => suture_core::integrity::RiskScore::Medium,
        4..=5 => suture_core::integrity::RiskScore::High,
        _ => suture_core::integrity::RiskScore::Critical,
    };
    // Upgrade if any individual file is critical
    let overall_risk = std::cmp::max(overall_risk, {
        files
            .iter()
            .map(|f| f.risk_score)
            .max()
            .unwrap_or(suture_core::integrity::RiskScore::None)
    });

    let summary = suture_core::integrity::IntegritySummary {
        total_files_changed: files.len(),
        high_risk_count,
        critical_risk_count,
        new_binary_files,
        build_system_changes,
        overall_risk,
        warnings,
    };

    Ok(suture_core::integrity::DiffIntegrityReport {
        files,
        old_files,
        summary,
    })
}

/// Get file content from CAS or filesystem.
fn get_file_content(
    repo: &suture_core::repository::Repository,
    path: &str,
    hash: Option<&suture_common::Hash>,
) -> Vec<u8> {
    if let Some(h) = hash
        && let Ok(blob) = repo.cas().get_blob(h)
    {
        return blob;
    }
    // Fall back to filesystem
    std::fs::read(repo.root().join(path)).unwrap_or_default()
}
