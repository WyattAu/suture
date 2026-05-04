use std::path::Path;

pub enum BatchAction {
    Stage {
        pattern: String,
    },
    Commit {
        pattern: String,
        message: String,
    },
    ExportClients {
        output: String,
        clients: Vec<String>,
    },
}

pub async fn cmd_batch(action: &BatchAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        BatchAction::Stage { pattern } => cmd_batch_stage(pattern).await,
        BatchAction::Commit { pattern, message } => cmd_batch_commit(pattern, message).await,
        BatchAction::ExportClients { clients, output } => {
            cmd_batch_export_clients(clients, output).await
        }
    }
}

async fn cmd_batch_stage(pattern: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;
    let matched = glob_match_files(".", pattern)?;

    if matched.is_empty() {
        println!("No files matched pattern '{pattern}'");
        return Ok(());
    }

    let mut staged = 0usize;
    for path in &matched {
        if let Err(e) = repo.add(path) {
            let err_msg = format!("failed to stage '{path}': {e}");
            return Err(err_msg.into());
        }
        staged += 1;
    }

    println!("Staged {staged} file(s) matching '{pattern}'");
    Ok(())
}

async fn cmd_batch_commit(pattern: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(Path::new("."))?;
    let matched = glob_match_files(".", pattern)?;

    if matched.is_empty() {
        println!("No files matched pattern '{pattern}'");
        return Ok(());
    }

    let mut staged = 0usize;
    for path in &matched {
        repo.add(path)?;
        staged += 1;
    }

    let patch_id = repo.commit(message)?;
    println!(
        "Staged {} file(s) and committed: {}",
        staged,
        patch_id.to_hex().chars().take(8).collect::<String>()
    );
    Ok(())
}

async fn cmd_batch_export_clients(
    clients: &[String],
    output: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if clients.is_empty() {
        return Err("no clients specified".into());
    }

    println!("\nBatch export summary:");
    println!("{}", "\u{2500}".repeat(40));

    let mut ok = 0usize;
    let mut fail = 0usize;
    for client in clients {
        let client_output = format!("{output}/{client}");
        match crate::cmd::export::cmd_export(&client_output, None, false, None, false, None).await {
            Ok(()) => {
                println!("  {client:<20} OK");
                ok += 1;
            }
            Err(e) => {
                println!("  {client:<20} FAILED: {e}");
                fail += 1;
            }
        }
    }

    println!("{}", "\u{2500}".repeat(40));
    println!("{ok} succeeded, {fail} failed");

    if fail > 0 {
        let err_msg = format!("{fail} client export(s) failed");
        return Err(err_msg.into());
    }

    Ok(())
}

fn glob_match_files(
    base_dir: &str,
    pattern: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let base = Path::new(base_dir);
    let mut matched = Vec::new();

    if pattern.contains('*') || pattern.contains('?') {
        collect_matching_files(base, base, pattern, &mut matched)?;
    } else {
        let target = base.join(pattern);
        if target.exists() && target.is_file() {
            matched.push(pattern.to_owned());
        }
    }

    matched.sort();
    Ok(matched)
}

fn collect_matching_files(
    root: &Path,
    current: &Path,
    pattern: &str,
    result: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entries: Vec<_> = std::fs::read_dir(current)?.filter_map(std::result::Result::ok).collect();
    entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    for entry in &entries {
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name == ".suture" {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if path.is_dir() {
            collect_matching_files(root, &path, pattern, result)?;
        } else if path.is_file() && glob_simple_match(&rel, pattern) {
            result.push(rel);
        }
    }

    Ok(())
}

fn glob_simple_match(path: &str, pattern: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();

    if pattern_parts.len() != path_parts.len() {
        return false;
    }

    for (pp, pat) in path_parts.iter().zip(pattern_parts.iter()) {
        if !match_segment(pp, pat) {
            return false;
        }
    }

    true
}

fn match_segment(text: &str, pattern: &str) -> bool {
    if !pattern.contains('*') && !pattern.contains('?') {
        return text == pattern;
    }

    let pat_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    let p_len = pat_chars.len();
    let t_len = text_chars.len();

    let mut dp = vec![vec![false; t_len + 1]; p_len + 1];
    dp[0][0] = true;

    for i in 1..=p_len {
        if pat_chars[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }

    for i in 1..=p_len {
        for j in 1..=t_len {
            if pat_chars[i - 1] == '*' {
                dp[i][j] = dp[i - 1][j] || dp[i][j - 1];
            } else if pat_chars[i - 1] == '?' || pat_chars[i - 1] == text_chars[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            }
        }
    }

    dp[p_len][t_len]
}
