use tokio::io::AsyncBufReadExt;

pub(crate) async fn cmd_add(
    paths: &[String],
    all: bool,
    patch: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if all {
        let count = repo.add_all()?;
        println!("Staged {} files", count);
        return Ok(());
    }

    let file_paths: Vec<String> = if paths.is_empty() {
        return Err("no paths specified (use --all to stage everything)".into());
    } else {
        paths.to_vec()
    };

    if patch {
        cmd_add_patch(&repo, &file_paths).await
    } else {
        for path in &file_paths {
            repo.add(path)?;
            println!("Added {}", path);
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
            println!("Added {}", path);
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

            println!("\n--- diff for {} ---", path);
            print_line_diff(&head_lines, &disk_lines);
        } else {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            let lines: Vec<&str> = content.lines().take(10).collect();
            println!("\n--- new file: {} (first {} lines) ---", path, lines.len());
            for line in &lines {
                println!("+ {}", line);
            }
            if content.lines().count() > 10 {
                println!("+ ... ({} more lines)", content.lines().count() - 10);
            }
        }

        loop {
            print!("Stage changes to {}? [y/n/q/a(ll)] ", path);
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
                    println!("Added {}", path);
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
                    println!("Added {}", path);
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
        println!("- {}", line);
    }
    for line in &additions {
        println!("+ {}", line);
    }
}
