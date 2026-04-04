use crate::display::format_line_diff;
use crate::style::{ANSI_BOLD_CYAN, ANSI_RESET};

pub(crate) async fn cmd_diff(
    from: Option<&str>,
    to: Option<&str>,
    cached: bool,
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

    use std::path::Path as StdPath;
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

    for entry in &entries {
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
                        println!("{ANSI_BOLD_CYAN}added {} (binary){ANSI_RESET}", entry.path);
                        continue;
                    };
                    let new_str = String::from_utf8_lossy(&new_blob);

                    if let Ok(driver) = registry.get_for_path(StdPath::new(&entry.path))
                        && let Ok(semantic) = driver.format_diff(None, &new_str)
                        && !semantic.is_empty()
                        && semantic != "no changes"
                    {
                        println!(
                            "\n{ANSI_BOLD_CYAN}--- Semantic diff for {} ---{ANSI_RESET}",
                            entry.path
                        );
                        println!("{semantic}");
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
                    let old_str = String::from_utf8_lossy(&old_blob);
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
                            let old_str = String::from_utf8_lossy(&old_blob);
                            let new_str = String::from_utf8_lossy(&new_blob);

                            if let Ok(driver) = registry.get_for_path(StdPath::new(&entry.path))
                                && let Ok(semantic) = driver.format_diff(Some(&old_str), &new_str)
                                && !semantic.is_empty()
                                && semantic != "no changes"
                            {
                                println!(
                                    "\n{ANSI_BOLD_CYAN}--- Semantic diff for {} ---{ANSI_RESET}",
                                    entry.path
                                );
                                println!("{semantic}");
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
