use crate::style::{ANSI_GREEN, ANSI_RED, ANSI_RESET};

pub(crate) async fn cmd_merge_file(
    base_path: &str,
    ours_path: &str,
    theirs_path: &str,
    label_ours: Option<&str>,
    label_theirs: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let base_content = std::fs::read_to_string(base_path)?;
    let ours_content = std::fs::read_to_string(ours_path)?;
    let theirs_content = std::fs::read_to_string(theirs_path)?;

    let ours_label = label_ours.unwrap_or("ours");
    let theirs_label = label_theirs.unwrap_or("theirs");

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

    for line in &result.lines {
        println!("{line}");
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
