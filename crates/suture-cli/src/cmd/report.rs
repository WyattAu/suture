use crate::ref_utils::resolve_ref;
use std::path::Path;

use std::fmt::Write;
pub enum ReportType {
    Change {
        from: Option<String>,
        to: Option<String>,
        format: String,
        output: Option<String>,
    },
    Activity {
        days: u64,
        format: String,
    },
    Stats {
        at: String,
    },
}

pub async fn cmd_report(report_type: &ReportType) -> Result<(), Box<dyn std::error::Error>> {
    match report_type {
        ReportType::Change {
            from,
            to,
            format,
            output,
        } => cmd_report_change(from.as_deref(), to.as_deref(), format, output.as_deref()).await,
        ReportType::Activity { days, format } => cmd_report_activity(*days, format).await,
        ReportType::Stats { at } => cmd_report_stats(at.as_str()).await,
    }
}

async fn cmd_report_change(
    from: Option<&str>,
    to: Option<&str>,
    format: &str,
    output: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;
    let all_patches = repo.all_patches();

    let to_ref = to.unwrap_or("HEAD");
    let to_patch = resolve_ref(&repo, to_ref, &all_patches)?;

    let from_ref = from.map_or_else(|| {
        let tags = repo.list_tags().unwrap_or_default();
        if let Some((tag_name, tag_id)) = tags.last() {
            let tag_hex = tag_id.to_hex();
            if tag_hex == to_patch.id.to_hex() {
                "HEAD~10".to_owned()
            } else {
                tag_name.clone()
            }
        } else {
            "HEAD~10".to_owned()
        }
    }, std::borrow::ToOwned::to_owned);

    let from_patch = resolve_ref(&repo, &from_ref, &all_patches)?;

    let _from_hex = from_patch.id.to_hex();
    let _to_hex = to_patch.id.to_hex();

    let patches: Vec<_> = all_patches
        .iter()
        .filter(|p| p.timestamp > from_patch.timestamp && p.timestamp <= to_patch.timestamp)
        .cloned()
        .collect();

    let mut all_authors = std::collections::HashSet::new();
    let mut all_files_changed = std::collections::HashSet::new();
    let mut commit_entries: Vec<CommitEntry> = Vec::new();

    for patch in &patches {
        all_authors.insert(patch.author.clone());

        let (added, modified, deleted) = classify_files(&repo, patch);
        for f in &added {
            all_files_changed.insert(f.clone());
        }
        for f in &modified {
            all_files_changed.insert(f.clone());
        }
        for f in &deleted {
            all_files_changed.insert(f.clone());
        }

        commit_entries.push(CommitEntry {
            hash: patch.id.to_hex(),
            author: patch.author.clone(),
            timestamp: patch.timestamp,
            message: patch.message.clone(),
            added,
            modified,
            deleted,
        });
    }

    commit_entries.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

    let repo_name = std::env::current_dir()
        .unwrap_or_default()
        .file_name().map_or_else(|| "unknown".to_owned(), |n| n.to_string_lossy().to_string());

    let report = match format {
        "html" => generate_change_html(
            &repo_name,
            &from_ref,
            to_ref,
            &commit_entries,
            all_authors.len(),
            all_files_changed.len(),
        ),
        _ => generate_change_markdown(
            &repo_name,
            &from_ref,
            to_ref,
            &commit_entries,
            all_authors.len(),
            all_files_changed.len(),
        ),
    };

    match output {
        Some(path) => {
            std::fs::write(path, &report)?;
            println!("Report written to {path}");
        }
        None => print!("{report}"),
    }

    Ok(())
}

struct CommitEntry {
    hash: String,
    author: String,
    timestamp: u64,
    message: String,
    added: Vec<String>,
    modified: Vec<String>,
    deleted: Vec<String>,
}

fn generate_change_markdown(
    repo_name: &str,
    from_ref: &str,
    to_ref: &str,
    entries: &[CommitEntry],
    author_count: usize,
    file_count: usize,
) -> String {
    let mut out = String::new();
    let _ = write!(out, 
        "# Change Report: {repo_name} ({from_ref} → {to_ref})\n\n"
    );
    let _ = write!(out, 
        "## Summary\n\n- **Commits:** {}\n- **Files changed:** {}\n- **Authors:** {}\n\n",
        entries.len(),
        file_count,
        author_count
    );

    if !entries.is_empty() {
        out.push_str("## Commits\n\n");
        for entry in entries {
            let short_hash = entry.hash.chars().take(8).collect::<String>();
            let dt = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            let _ = write!(out, 
                "### {} — {}\n\n**Author:** {}  \n**Date:** {}\n\n",
                short_hash, entry.message, entry.author, dt
            );

            if !entry.added.is_empty() || !entry.modified.is_empty() || !entry.deleted.is_empty() {
                out.push_str("| Status | File |\n|--------|------|\n");
                for f in &entry.added {
                    let _ = writeln!(out, "| A | {f} |");
                }
                for f in &entry.modified {
                    let _ = writeln!(out, "| M | {f} |");
                }
                for f in &entry.deleted {
                    let _ = writeln!(out, "| D | {f} |");
                }
                out.push('\n');
            }
        }
    }

    out
}

fn generate_change_html(
    repo_name: &str,
    from_ref: &str,
    to_ref: &str,
    entries: &[CommitEntry],
    author_count: usize,
    file_count: usize,
) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">");
    out.push_str("<style>");
    out.push_str(
        "body{font-family:system-ui,sans-serif;max-width:900px;margin:2em auto;padding:0 1em}",
    );
    out.push_str("h1{color:#333}h2{color:#555;border-bottom:1px solid #eee;padding-bottom:.3em}");
    out.push_str("h3{color:#666}table{border-collapse:collapse;width:100%}th,td{border:1px solid #ddd;padding:6px 10px;text-align:left}");
    out.push_str("th{background:#f5f5f5}tr:nth-child(even){background:#fafafa}.added{color:#22863a}.modified{color:#b08800}.deleted{color:#cb2431}");
    out.push_str(".meta{color:#888;font-size:.9em}summary{margin:1em 0;padding:1em;background:#f9f9f9;border-radius:4px}");
    out.push_str("</style></head><body>\n");
    let _ = writeln!(out, 
        "<h1>Change Report: {repo_name} ({from_ref} → {to_ref})</h1>"
    );
    out.push_str("<div class=\"summary\">");
    let _ = write!(out, 
        "<strong>Commits:</strong> {} | <strong>Files changed:</strong> {} | <strong>Authors:</strong> {}",
        entries.len(),
        file_count,
        author_count
    );
    out.push_str("</div>\n");

    if !entries.is_empty() {
        out.push_str("<h2>Commits</h2>\n");
        for entry in entries {
            let short_hash = entry.hash.chars().take(8).collect::<String>();
            let dt = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            let _ = write!(out, 
                "<h3>{} — {}</h3>\n<p class=\"meta\">Author: {}<br>Date: {}</p>\n",
                short_hash, entry.message, entry.author, dt
            );

            if !entry.added.is_empty() || !entry.modified.is_empty() || !entry.deleted.is_empty() {
                out.push_str("<table><tr><th>Status</th><th>File</th></tr>\n");
                for f in &entry.added {
                    let _ = writeln!(out, 
                        "<tr><td class=\"added\">A</td><td>{f}</td></tr>"
                    );
                }
                for f in &entry.modified {
                    let _ = writeln!(out, 
                        "<tr><td class=\"modified\">M</td><td>{f}</td></tr>"
                    );
                }
                for f in &entry.deleted {
                    let _ = writeln!(out, 
                        "<tr><td class=\"deleted\">D</td><td>{f}</td></tr>"
                    );
                }
                out.push_str("</table>\n");
            }
        }
    }

    out.push_str("</body></html>");
    out
}

async fn cmd_report_activity(days: u64, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = now.saturating_sub(days * 86400);

    let patches = repo.log_all(None)?;
    let recent: Vec<_> = patches
        .into_iter()
        .filter(|p| p.timestamp >= cutoff)
        .collect();

    let mut author_stats: std::collections::HashMap<String, AuthorStats> =
        std::collections::HashMap::new();

    for patch in &recent {
        let entry = author_stats
            .entry(patch.author.clone())
            .or_insert_with(|| AuthorStats {
                commits: 0,
                files_changed: 0,
                most_recent: 0,
            });
        entry.commits += 1;
        entry.files_changed += patch.touch_set.addresses().len();
        if patch.timestamp > entry.most_recent {
            entry.most_recent = patch.timestamp;
        }
    }

    let mut sorted_authors: Vec<_> = author_stats.into_iter().collect();
    sorted_authors.sort_by_key(|b| std::cmp::Reverse(b.1.most_recent));

    if format == "markdown" {
        println!("# Activity Report (last {days} days)\n");
        println!("| Author | Commits | Files Changed | Most Recent |");
        println!("|--------|---------|---------------|-------------|");
        for (author, stats) in &sorted_authors {
            let dt = chrono::DateTime::from_timestamp(stats.most_recent as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M")
                .to_string();
            println!(
                "| {} | {} | {} | {} |",
                author, stats.commits, stats.files_changed, dt
            );
        }
        println!("\n**Total commits:** {}", recent.len());
    } else {
        println!("Activity Report (last {days} days)");
        println!("{}", "\u{2500}".repeat(60));
        for (author, stats) in &sorted_authors {
            let dt = chrono::DateTime::from_timestamp(stats.most_recent as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M")
                .to_string();
            println!(
                "  {:<20} {} commits, {} files changed, last: {}",
                author, stats.commits, stats.files_changed, dt
            );
        }
        println!("{}", "\u{2500}".repeat(60));
        println!(
            "Total: {} commits by {} authors",
            recent.len(),
            sorted_authors.len()
        );
    }

    Ok(())
}

struct AuthorStats {
    commits: usize,
    files_changed: usize,
    most_recent: u64,
}

async fn cmd_report_stats(at: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;
    let all_patches = repo.all_patches();
    let patch = resolve_ref(&repo, at, &all_patches)?;
    let tree = repo.snapshot(&patch.id)?;

    let mut type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut file_sizes: Vec<(String, usize)> = Vec::new();
    let mut total_size: usize = 0;

    for (path, hash) in tree.iter() {
        if path.starts_with(".suture/") {
            continue;
        }

        let category = classify_file_category(path);
        *type_counts.entry(category).or_insert(0) += 1;

        if let Ok(data) = repo.cas().get_blob(hash) {
            let size = data.len();
            total_size += size;
            file_sizes.push((path.clone(), size));
        }
    }

    file_sizes.sort_by_key(|b| std::cmp::Reverse(b.1));

    println!("File Statistics (at {at})");
    println!("{}", "\u{2500}".repeat(60));

    let mut categories: Vec<_> = type_counts.into_iter().collect();
    categories.sort_by_key(|b| std::cmp::Reverse(b.1));

    println!("\nFiles by type:");
    for (category, count) in &categories {
        println!("  {category:<12} {count}");
    }

    let total_files: usize = categories.iter().map(|(_, c)| *c).sum();
    println!("\nTotal files: {total_files}");
    println!("Total size:  {}", format_size(total_size));

    println!("\nLargest files:");
    for (path, size) in file_sizes.iter().take(10) {
        println!("  {:>8}  {}", format_size(*size), path);
    }

    Ok(())
}

fn classify_file_category(path: &str) -> String {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "rst" => "text".to_owned(),
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp" | "svg" => "image".to_owned(),
        "mp4" | "mov" | "avi" | "mkv" | "webm" => "video".to_owned(),
        "mp3" | "wav" | "flac" | "aac" | "ogg" => "audio".to_owned(),
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" => {
            "document".to_owned()
        }
        "json" | "yaml" | "yml" | "toml" | "xml" | "csv" => "data".to_owned(),
        "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" | "rb" | "sh" => {
            "code".to_owned()
        }
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => "archive".to_owned(),
        _ => "other".to_owned(),
    }
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn classify_files(
    repo: &suture_core::repository::Repository,
    patch: &suture_core::patch::types::Patch,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let parent_hex = patch
        .parent_ids
        .first()
        .map(suture_core::Hash::to_hex)
        .unwrap_or_default();
    let commit_hex = patch.id.to_hex();
    let from = if parent_hex.is_empty() {
        None
    } else {
        Some(parent_hex.as_str())
    };

    let entries = repo
        .diff(from, Some(commit_hex.as_str()))
        .unwrap_or_default();

    use suture_core::engine::diff::DiffType;
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    for entry in &entries {
        match &entry.diff_type {
            DiffType::Added => added.push(entry.path.clone()),
            DiffType::Modified => modified.push(entry.path.clone()),
            DiffType::Deleted => deleted.push(entry.path.clone()),
            DiffType::Renamed { old_path, new_path } => {
                deleted.push(old_path.clone());
                added.push(new_path.clone());
            }
        }
    }

    (added, modified, deleted)
}
