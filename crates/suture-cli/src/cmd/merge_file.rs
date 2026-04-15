use crate::style::{ANSI_GREEN, ANSI_RED, ANSI_RESET};

pub(crate) async fn cmd_merge_file(
    base_path: &str,
    ours_path: &str,
    theirs_path: &str,
    label_ours: Option<&str>,
    label_theirs: Option<&str>,
    driver_name: Option<&str>,
    output_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path as StdPath;

    let base_content = std::fs::read_to_string(base_path)?;
    let ours_content = std::fs::read_to_string(ours_path)?;
    let theirs_content = std::fs::read_to_string(theirs_path)?;

    let ours_label = label_ours.unwrap_or("ours");
    let theirs_label = label_theirs.unwrap_or("theirs");

    let registry = crate::driver_registry::builtin_registry();

    // Resolve driver: explicit --driver flag > auto-detect by extension > none
    let driver: Option<&dyn suture_driver::SutureDriver> = match driver_name {
        Some(name) => {
            // Explicit driver: try as extension name (e.g., "json" -> ".json")
            let ext = if name.starts_with('.') {
                name.to_string()
            } else {
                format!(".{name}")
            };
            match registry.get(&ext) {
                Ok(d) => Some(d),
                Err(e) => {
                    eprintln!("{ANSI_RED}Error: {e}{ANSI_RESET}");
                    std::process::exit(1);
                }
            }
        }
        None => {
            // Auto-detect by file extension
            registry
                .get_for_path(StdPath::new(ours_path))
                .ok()
        }
    };

    // Try semantic merge if a driver is available
    if let Some(driver) = driver {
        match driver.merge(&base_content, &ours_content, &theirs_content) {
            Ok(Some(merged)) => {
                // Clean semantic merge
                match output_path {
                    Some(path) => std::fs::write(path, &merged)?,
                    None => print!("{merged}"),
                }
                eprintln!(
                    "{ANSI_GREEN}Merged via {} driver (semantic merge){ANSI_RESET}",
                    driver.name()
                );
                return Ok(());
            }
            Ok(None) => {
                // Driver declined — fall back to line-based merge
                eprintln!(
                    "Note: {} driver could not auto-resolve, falling back to line-based merge",
                    driver.name()
                );
            }
            Err(e) => {
                // Parse error — fall back to line-based merge
                eprintln!(
                    "Note: {} driver error: {e}, falling back to line-based merge",
                    driver.name()
                );
            }
        }
    }

    // Line-based three-way merge (fallback or default)
    let base_lines: Vec<&str> = base_content.lines().collect();
    let ours_lines: Vec<&str> = ours_content.lines().collect();
    let theirs_lines: Vec<&str> = theirs_content.lines().collect();

    let result = suture_core::engine::merge::three_way_merge_lines(
        &base_lines,
        &ours_lines,
        &theirs_lines,
        ours_label,
        theirs_label,
    );

    let merged_output: String = result.lines.join("\n");
    match output_path {
        Some(path) => std::fs::write(path, &merged_output)?,
        None => print!("{merged_output}"),
    }

    if result.is_clean {
        eprintln!(
            "{ANSI_GREEN}Merge clean ({} regions auto-merged){ANSI_RESET}",
            result.auto_merged
        );
    } else {
        eprintln!(
            "{ANSI_RED}Merge conflicts: {} conflict(s), {} auto-merged{ANSI_RESET}",
            result.conflicts,
            result.auto_merged
        );
        std::process::exit(1);
    }

    Ok(())
}
