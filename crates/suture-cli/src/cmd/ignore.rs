use std::path::Path;

pub(crate) async fn cmd_ignore(args: &IgnoreArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args {
        IgnoreArgs::List => cmd_ignore_list(),
        IgnoreArgs::Check { path } => cmd_ignore_check(path),
    }
}

fn cmd_ignore_list() -> Result<(), Box<dyn std::error::Error>> {
    let ignore_path = std::path::Path::new(".sutureignore");
    if !ignore_path.exists() {
        println!("No .sutureignore file found.");
        return Ok(());
    }
    let contents = std::fs::read_to_string(ignore_path)?;
    let lines: Vec<&str> = contents
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.is_empty() {
        println!("No ignore patterns defined.");
    } else {
        println!("Ignore patterns ({}):", lines.len());
        for line in &lines {
            println!("  {}", line);
        }
    }
    Ok(())
}

fn cmd_ignore_check(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let patterns = load_ignore_patterns(Path::new("."));
    let ignored = is_ignored(path, &patterns);
    if ignored {
        println!("{} is ignored", path);
    } else {
        println!("{} is NOT ignored", path);
    }
    Ok(())
}

fn load_ignore_patterns(root: &Path) -> Vec<String> {
    let ignore_file = root.join(".sutureignore");
    if !ignore_file.exists() {
        return Vec::new();
    }

    std::fs::read_to_string(&ignore_file)
        .unwrap_or_default()
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn is_ignored(rel_path: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if let Some(suffix) = pattern.strip_prefix('*') {
            if rel_path.ends_with(suffix) {
                return true;
            }
        } else if pattern.ends_with('/') {
            if rel_path.starts_with(pattern) {
                return true;
            }
        } else {
            if rel_path == pattern || rel_path.starts_with(&format!("{}/", pattern)) {
                return true;
            }
        }
    }
    false
}

pub(crate) enum IgnoreArgs {
    List,
    Check { path: String },
}
