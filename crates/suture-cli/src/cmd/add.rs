use std::path::Path;
use tokio::io::AsyncBufReadExt;

use crate::cmd::lfs::{compute_sha256, create_lfs_pointer, should_track_as_lfs, store_lfs_object};

/// Expand paths: if a path is a directory, recursively collect all files in it.
fn expand_paths(paths: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for path_str in paths {
        let path = Path::new(path_str);
        if path.is_dir() {
            expand_dir_recursive(path, &mut result);
        } else if path.is_file()
            && let Some(s) = path.to_str()
        {
            result.push(s.to_owned());
        }
    }
    result
}

/// Recursively collect all files under `dir`, skipping .suture directories.
fn expand_dir_recursive(dir: &Path, result: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let entry_path = entry.path();
        let file_name = entry.file_name();

        // Skip .suture directory
        if file_name == ".suture" {
            continue;
        }

        if entry_path.is_dir() {
            expand_dir_recursive(&entry_path, result);
        } else if entry_path.is_file()
            && let Some(s) = entry_path.to_str()
        {
            result.push(s.to_owned());
        }
    }
}

fn maybe_convert_to_lfs_pointer(path: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let full_path = Path::new(path);
    let metadata = std::fs::metadata(full_path)?;
    let file_size = metadata.len();

    let repo_root = Path::new(".");
    if let Some(_limit) = should_track_as_lfs(repo_root, path, file_size) {
        let data = std::fs::read(full_path)?;
        let hash = compute_sha256(&data);
        let file_name = full_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path).to_owned();
        store_lfs_object(repo_root, &hash, &data)?;
        let pointer = create_lfs_pointer(&hash, file_size, &file_name);
        std::fs::write(full_path, pointer)?;
        return Ok(true);
    }
    Ok(false)
}

pub async fn cmd_add(
    paths: &[String],
    all: bool,
    patch: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;

    if all {
        let count = repo.add_all()?;
        println!("Staged {count} files");
        return Ok(());
    }

    let file_paths = expand_paths(paths);

    if file_paths.is_empty() {
        return Err("no files to stage (use --all to stage everything)".into());
    }

    if patch {
        cmd_add_patch(&repo, &file_paths).await
    } else {
        for path in &file_paths {
            if maybe_convert_to_lfs_pointer(path)? {
                // repo.add() reads the pointer from disk and stores it
                repo.add(path)?;
                println!(
                    "Added {} (LFS pointer, {} bytes)",
                    path,
                    std::fs::metadata(path)?.len()
                );
            } else {
                repo.add(path)?;
                println!("Added {path}");
            }
        }
        Ok(())
    }
}

async fn cmd_add_patch(
    repo: &suture_core::repository::Repository,
    paths: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let head_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::FileTree::empty());
    let mut stage_all = false;

    for path in paths {
        if stage_all {
            repo.add(path)?;
            println!("Added {path}");
            continue;
        }

        if head_tree.contains(path) {
            let Some(&head_hash) = head_tree.get(path) else {
                continue;
            };
            let head_bytes = repo.cas().get_blob(&head_hash).unwrap_or_default();
            let head_content = String::from_utf8_lossy(&head_bytes);

            let disk_content = std::fs::read_to_string(path).unwrap_or_default();

            if head_content == disk_content {
                continue;
            }

            let head_lines: Vec<&str> = head_content.lines().collect();
            let disk_lines: Vec<&str> = disk_content.lines().collect();

            println!("\n--- diff for {path} ---");
            print_line_diff(&head_lines, &disk_lines);
        } else {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            let lines: Vec<&str> = content.lines().take(10).collect();
            println!("\n--- new file: {} (first {} lines) ---", path, lines.len());
            for line in &lines {
                println!("+ {line}");
            }
            if content.lines().count() > 10 {
                println!("+ ... ({} more lines)", content.lines().count() - 10);
            }
        }

        loop {
            print!("Stage changes to {path}? [y/n/q/a(ll)] ");
            use std::io::Write;
            std::io::stdout().flush()?;

            let mut input = String::new();
            tokio::io::BufReader::new(tokio::io::stdin())
                .read_line(&mut input)
                .await?;
            let answer = input.trim().to_lowercase();

            match answer.as_str() {
                "y" | "yes" => {
                    repo.add(path)?;
                    println!("Added {path}");
                    break;
                }
                "n" | "no" => {
                    break;
                }
                "q" | "quit" => {
                    println!("Quit.");
                    return Ok(());
                }
                "a" | "all" => {
                    repo.add(path)?;
                    println!("Added {path}");
                    stage_all = true;
                    break;
                }
                _ => {
                    eprintln!("Please answer y, n, q, or a.");
                }
            }
        }
    }

    Ok(())
}

fn print_line_diff(head_lines: &[&str], disk_lines: &[&str]) {
    let additions: Vec<&str> = disk_lines
        .iter()
        .filter(|l| !head_lines.contains(l))
        .copied()
        .collect();
    let removals: Vec<&str> = head_lines
        .iter()
        .filter(|l| !disk_lines.contains(l))
        .copied()
        .collect();

    for line in &removals {
        println!("- {line}");
    }
    for line in &additions {
        println!("+ {line}");
    }
}
