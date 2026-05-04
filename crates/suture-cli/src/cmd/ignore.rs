use std::path::Path;

pub struct IgnoreRule {
    pattern: String,
    is_negation: bool,
    dir_only: bool,
    line_number: usize,
}

pub async fn cmd_ignore(args: &IgnoreArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args {
        IgnoreArgs::List => { cmd_ignore_list(); }
        IgnoreArgs::Check { path } => { cmd_ignore_check(path); }
    }
    Ok(())
}

fn cmd_ignore_list() {
    let ignore_path = std::path::Path::new(".sutureignore");
    if !ignore_path.exists() {
        println!("No .sutureignore file found.");
        return;
    }
    let rules = load_ignore_rules(ignore_path);
    if rules.is_empty() {
        println!("No ignore patterns defined.");
    } else {
        println!("Ignore patterns ({}):", rules.len());
        for rule in &rules {
            let prefix = if rule.is_negation { "! " } else { "" };
            let suffix = if rule.dir_only { "/" } else { "" };
            println!(
                "  {:>4}: {}{}{}",
                rule.line_number, prefix, rule.pattern, suffix
            );
        }
    }
}

fn cmd_ignore_check(path: &str) {
    let ignore_path = std::path::Path::new(".sutureignore");
    if !ignore_path.exists() {
        println!("{path} is NOT ignored (no .sutureignore file)");
        return;
    }
    let rules = load_ignore_rules(ignore_path);
    let result = check_ignored(path, &rules);
    match result {
        Some(rule) => {
            let display_pattern = if rule.is_negation {
                format!("!{}", rule.pattern)
            } else {
                rule.pattern.clone()
            };
            if rule.is_negation {
                println!(
                    "{} is NOT ignored (negated by: {} line {} of .sutureignore)",
                    path, display_pattern, rule.line_number
                );
            } else {
                println!(
                    "  Ignored by: {} (line {} of .sutureignore)",
                    display_pattern, rule.line_number
                );
            }
        }
        None => {
            println!("{path} is NOT ignored");
        }
    }
}

fn load_ignore_rules(ignore_path: &Path) -> Vec<IgnoreRule> {
    let contents = match std::fs::read_to_string(ignore_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            eprintln!("warning: failed to read .sutureignore: {e}");
            String::new()
        }
    };
    let mut rules = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (is_negation, raw) = trimmed
            .strip_prefix('!')
            .map_or((false, trimmed), |rest| (true, rest.trim()));
        let (pattern, dir_only) = if raw.ends_with('/') {
            (raw.trim_end_matches('/'), true)
        } else {
            (raw, false)
        };
        if pattern.is_empty() {
            continue;
        }
        rules.push(IgnoreRule {
            pattern: pattern.to_owned(),
            is_negation,
            dir_only,
            line_number: line_idx + 1,
        });
    }
    rules
}

fn pattern_matches(pattern: &str, rel_path: &str) -> bool {
    pattern.strip_prefix('*').map_or_else(
        || {
            pattern.strip_suffix('*').map_or_else(
                || if pattern.contains('*') {
                    simple_glob_match(pattern, rel_path)
                } else {
                    rel_path == pattern || rel_path.starts_with(&format!("{pattern}/"))
                },
                |prefix| rel_path.starts_with(prefix),
            )
        },
        |suffix| rel_path.ends_with(suffix),
    )
}

fn simple_glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut dp = vec![vec![false; t.len() + 1]; p.len() + 1];
    dp[0][0] = true;
    for i in 1..=p.len() {
        if p[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=p.len() {
        for j in 1..=t.len() {
            if p[i - 1] == '*' {
                dp[i][j] = dp[i - 1][j] || dp[i][j - 1];
            } else if p[i - 1] == '?' || p[i - 1] == t[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            }
        }
    }
    dp[p.len()][t.len()]
}

fn check_ignored<'a>(rel_path: &str, rules: &'a [IgnoreRule]) -> Option<&'a IgnoreRule> {
    let is_dir = rel_path.ends_with('/');
    let path_for_match = rel_path.trim_end_matches('/');
    let mut last_negation: Option<&IgnoreRule> = None;

    for rule in rules {
        if rule.dir_only && !is_dir {
            continue;
        }
        if pattern_matches(&rule.pattern, path_for_match) {
            if rule.is_negation {
                last_negation = Some(rule);
            } else {
                return Some(rule);
            }
        }
    }

    last_negation
}

pub enum IgnoreArgs {
    List,
    Check { path: String },
}
