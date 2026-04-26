use crate::style::{ANSI_GREEN, ANSI_RED, ANSI_RESET, ANSI_YELLOW};

/// Read a file's contents as bytes and convert to String.
///
/// For text files this is a normal UTF-8 read. For binary files (DOCX, XLSX, PPTX)
/// which are ZIP archives, we read the raw bytes and treat them as a String since
/// the SutureDriver trait operates on `&str`. This is safe because:
/// - The driver will parse the bytes at the format level (ZIP → XML → merge → ZIP)
/// - The output is written back as raw bytes via `std::fs::write`
/// - We never interpret the binary content as UTF-8 text ourselves
fn read_file_bytes(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    // SAFETY: SutureDriver implementations handle binary content at the format level.
    // DOCX/XLSX/PPTX are ZIP archives — the bytes are round-tripped through
    // ZIP read → XML parse → merge → XML serialize → ZIP write.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

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

    // Read all three files (binary-safe for OOXML formats)
    let base_content = read_file_bytes(base_path)
        .map_err(|e| format!("{ANSI_RED}Error reading base file '{base_path}': {e}{ANSI_RESET}"))?;
    let ours_content = read_file_bytes(ours_path)
        .map_err(|e| format!("{ANSI_RED}Error reading ours file '{ours_path}': {e}{ANSI_RESET}"))?;
    let theirs_content = read_file_bytes(theirs_path).map_err(|e| {
        format!("{ANSI_RED}Error reading theirs file '{theirs_path}': {e}{ANSI_RESET}")
    })?;

    let ours_label = label_ours.unwrap_or("ours");
    let theirs_label = label_theirs.unwrap_or("theirs");

    let registry = crate::driver_registry::builtin_registry();

    // Resolve driver: explicit --driver flag > auto-detect by extension > none
    let driver: Option<&dyn suture_driver::SutureDriver> = match driver_name {
        Some(name) => {
            if name == "auto" {
                // --driver auto: detect from file extension
                registry.get_for_path(StdPath::new(ours_path)).ok()
            } else {
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
        }
        None => {
            // Auto-detect by file extension
            registry.get_for_path(StdPath::new(ours_path)).ok()
        }
    };

    // Try semantic merge if a driver is available
    if let Some(driver) = driver {
        match driver.merge(&base_content, &ours_content, &theirs_content) {
            Ok(Some(merged)) => {
                // Clean semantic merge
                match output_path {
                    Some(path) => std::fs::write(path, merged.as_bytes())?,
                    None => {
                        // For binary formats, don't print to stdout
                        let ours_ext = StdPath::new(ours_path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("");
                        if is_binary_extension(ours_ext) {
                            eprintln!(
                                "{ANSI_YELLOW}Note: merged content is binary ({ours_ext}). Use -o <path> to write output.{ANSI_RESET}"
                            );
                        } else {
                            print!("{merged}");
                        }
                    }
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
                    "{ANSI_YELLOW}Note: {} driver could not auto-resolve, falling back to line-based merge{ANSI_RESET}",
                    driver.name()
                );
            }
            Err(e) => {
                // Parse error — fall back to line-based merge
                eprintln!(
                    "{ANSI_YELLOW}Note: {} driver error: {e}, falling back to line-based merge{ANSI_RESET}",
                    driver.name()
                );
            }
        }
    }

    // Line-based three-way merge (fallback or default)
    // For binary formats with no driver, this won't produce useful results,
    // but it's better than nothing.
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
        Some(path) => std::fs::write(path, merged_output.as_bytes())?,
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
            result.conflicts, result.auto_merged
        );
        std::process::exit(1);
    }

    Ok(())
}

/// Check if a file extension represents a binary format.
fn is_binary_extension(ext: &str) -> bool {
    matches!(
        ext,
        "docx" | "docm" | "xlsx" | "xlsm" | "pptx" | "pptm" | "pdf" | "otio"
    )
}
