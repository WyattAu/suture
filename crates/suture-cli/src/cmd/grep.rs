use std::path::Path as StdPath;
use regex::Regex;

pub(crate) async fn cmd_grep(
    pattern: &str,
    paths: &[String],
    ignore_case: bool,
    files_only: bool,
    line_number: bool,
    fixed_string: bool,
    context: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
    let head_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());

    let re: Regex = if fixed_string {
        let escaped = regex::escape(pattern);
        if ignore_case {
            Regex::new(&format!("(?i){}", escaped))?
        } else {
            Regex::new(&escaped)?
        }
    } else if ignore_case {
        Regex::new(&format!("(?i){}", pattern))?
    } else {
        Regex::new(pattern)?
    };

    let mut tracked_paths: Vec<String> = head_tree.paths().into_iter().cloned().collect();
    tracked_paths.sort();

    let search_paths: Vec<String> = if paths.len() == 1 && paths[0] == "." {
        tracked_paths
    } else {
        tracked_paths
            .into_iter()
            .filter(|p| paths.iter().any(|search| p.starts_with(search) || p == search))
            .collect()
    };

    let mut match_count = 0;
    let mut file_match_count = 0;

    for path in &search_paths {
        let full_path = StdPath::new(path);
        if !full_path.exists() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(full_path) else {
            continue;
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut match_indices: Vec<usize> = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                match_indices.push(line_idx);
            }
        }

        if match_indices.is_empty() {
            continue;
        }

        file_match_count += 1;
        match_count += match_indices.len();

        if files_only {
            println!("{}", path);
            continue;
        }

        if let Some(ctx) = context {
            let mut printed_ranges: Vec<std::ops::RangeInclusive<usize>> = Vec::new();
            for &idx in &match_indices {
                let start = idx.saturating_sub(ctx);
                let end = (idx + ctx).min(lines.len().saturating_sub(1));
                if let Some(last) = printed_ranges.last_mut() && *last.end() + 1 >= start {
                    *last = *last.start()..=end;
                    continue;
                }
                printed_ranges.push(start..=end);
            }

            for range in &printed_ranges {
                for line_idx in range.clone() {
                    let line_num = line_idx + 1;
                    if match_indices.contains(&line_idx) {
                        if line_number {
                            println!("{}:{}:{}", path, line_num, lines[line_idx]);
                        } else {
                            println!("{}:{}", path, lines[line_idx]);
                        }
                    } else {
                        if line_number {
                            println!("{}:{}-{}", path, line_num, lines[line_idx]);
                        } else {
                            println!("{}-{}", path, lines[line_idx]);
                        }
                    }
                }
                if range.end() < &lines.len().saturating_sub(1) {
                    println!("{}--", path);
                }
            }
        } else {
            for &line_idx in &match_indices {
                let line_num = line_idx + 1;
                if line_number {
                    println!("{}:{}:{}", path, line_num, lines[line_idx]);
                } else {
                    println!("{}:{}", path, lines[line_idx]);
                }
            }
        }
    }

    if match_count == 0 {
        return Err(format!(
            "no matches found for pattern '{}' in {} files",
            pattern,
            search_paths.len()
        )
        .into());
    }

    eprintln!("\n{} matches in {} files", match_count, file_match_count);

    Ok(())
}
