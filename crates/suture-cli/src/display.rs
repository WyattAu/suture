use crate::style::{ANSI_BOLD_CYAN, ANSI_GREEN, ANSI_RED, ANSI_RESET};

pub fn walk_repo_files(dir: &std::path::Path) -> Vec<String> {
    let mut files = Vec::new();
    walk_repo_files_inner(dir, dir, &mut files);
    files.sort(); // Deterministic display order
    files
}

fn walk_repo_files_inner(
    root: &std::path::Path,
    current: &std::path::Path,
    files: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        let name = entry.file_name();
        if name == ".suture" {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            walk_repo_files_inner(root, &path, files);
        } else if path.is_file() {
            files.push(rel);
        }
    }
}

pub fn format_line_diff(path: &str, changes: &[suture_core::engine::merge::LineChange]) {
    use suture_core::engine::merge::LineChange;

    let has_changes = changes
        .iter()
        .any(|c| !matches!(c, LineChange::Unchanged(_)));
    if !has_changes {
        return;
    }

    println!("{ANSI_BOLD_CYAN}diff --git a/{path} b/{path}{ANSI_RESET}");
    println!("{ANSI_BOLD_CYAN}--- a/{path}{ANSI_RESET}");
    println!("{ANSI_BOLD_CYAN}+++ b/{path}{ANSI_RESET}");

    let mut old_line = 1usize;
    let mut new_line = 1usize;
    let mut i = 0;

    while i < changes.len() {
        match &changes[i] {
            LineChange::Unchanged(lines) => {
                old_line += lines.len();
                new_line += lines.len();
                i += 1;
            }
            LineChange::Deleted(_) | LineChange::Inserted(_) => {
                let hunk_old_start = old_line;
                let hunk_new_start = new_line;
                let mut hunk_old_count = 0usize;
                let mut hunk_new_count = 0usize;
                let mut hunk_lines: Vec<(char, String)> = Vec::new();

                while i < changes.len() {
                    match &changes[i] {
                        LineChange::Deleted(lines) => {
                            for line in lines {
                                hunk_lines.push(('-', line.clone()));
                                hunk_old_count += 1;
                                old_line += 1;
                            }
                            i += 1;
                        }
                        LineChange::Inserted(lines) => {
                            for line in lines {
                                hunk_lines.push(('+', line.clone()));
                                hunk_new_count += 1;
                                new_line += 1;
                            }
                            i += 1;
                        }
                        LineChange::Unchanged(_) => break,
                    }
                }

                println!(
                    "{ANSI_BOLD_CYAN}@@ -{hunk_old_start},{hunk_old_count} +{hunk_new_start},{hunk_new_count} @@{ANSI_RESET}"
                );
                for (prefix, line) in &hunk_lines {
                    if *prefix == '-' {
                        println!("{ANSI_RED}-{line}{ANSI_RESET}");
                    } else {
                        println!("{ANSI_GREEN}+{line}{ANSI_RESET}");
                    }
                }
            }
        }
    }
}

pub fn format_timestamp(ts: u64) -> String {
    let days = ts / 86400;
    let hours = (ts % 86400) / 3600;
    let minutes = (ts % 3600) / 60;
    let remaining_secs = ts % 60;
    format!("{days}d {hours:02}:{minutes:02}:{remaining_secs:02} (unix: {ts})")
}
