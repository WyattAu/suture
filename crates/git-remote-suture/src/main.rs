use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: git-remote-suture <remote> <url>");
        std::process::exit(1);
    }

    let _remote_name = &args[1];
    let url = &args[2];

    let (base_url, repo_id) = parse_suture_url(url);

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match line {
            "capabilities" => {
                writeln!(stdout, "fetch").unwrap();
                writeln!(stdout, "push").unwrap();
                writeln!(stdout, "import").unwrap();
                writeln!(stdout, "export").unwrap();
                writeln!(stdout, "refspec refs/heads/*:refs/remotes/origin/*").unwrap();
                writeln!(stdout).unwrap();
                stdout.flush().unwrap();
            }
            "list" => {
                let branches = fetch_branches(&base_url, &repo_id);
                match branches {
                    Ok(brs) => {
                        for (name, target_hex) in &brs {
                            writeln!(stdout, "{} refs/heads/{}", target_hex, name).unwrap();
                        }
                        if brs.is_empty() {
                            writeln!(stdout).unwrap();
                        }
                    }
                    Err(e) => {
                        writeln!(stdout, "error {}", e).unwrap();
                    }
                }
                writeln!(stdout).unwrap();
                stdout.flush().unwrap();
            }
            cmd if cmd.starts_with("fetch ") => {
                handle_fetch(&base_url, &repo_id, &mut stdout);
            }
            cmd if cmd.starts_with("push ") => {
                handle_push(&base_url, &repo_id, cmd, &mut stdout);
            }
            "" => continue,
            _ => {
                eprintln!("unknown command: {}", line);
            }
        }
    }
}

fn parse_suture_url(url: &str) -> (String, String) {
    let stripped = url.strip_prefix("suture://").unwrap_or(url);
    let parts: Vec<&str> = stripped.splitn(2, '/').collect();
    let host = parts.first().unwrap_or(&"localhost:8080");
    let repo_id = parts.get(1).unwrap_or(&"default").to_string();
    (format!("http://{}", host), repo_id)
}

fn fetch_branches(base_url: &str, repo_id: &str) -> Result<Vec<(String, String)>, String> {
    let url = format!("{}/repos/{}/branches", base_url, repo_id);
    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    let branches = body
        .get("branches")
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default();

    let mut result = Vec::new();
    for branch in branches {
        let name = branch
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let target = branch
            .get("target_id")
            .and_then(|t| t.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !name.is_empty() {
            result.push((name, target));
        }
    }
    Ok(result)
}

fn handle_fetch(base_url: &str, repo_id: &str, stdout: &mut dyn Write) {
    let url = format!("{}/pull/compressed", base_url);
    let client = reqwest::blocking::Client::new();

    let pull_body = serde_json::json!({
        "repo_id": repo_id,
        "known_branches": [],
        "max_depth": null
    });

    let resp = match client.post(&url).json(&pull_body).send() {
        Ok(r) => r,
        Err(e) => {
            writeln!(stdout, "error {}", e).unwrap();
            writeln!(stdout).unwrap();
            stdout.flush().unwrap();
            return;
        }
    };

    let body: serde_json::Value = match resp.json() {
        Ok(b) => b,
        Err(e) => {
            writeln!(stdout, "error {}", e).unwrap();
            writeln!(stdout).unwrap();
            stdout.flush().unwrap();
            return;
        }
    };

    let branches = body
        .get("branches")
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default();

    for branch in &branches {
        let name = branch
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("main");
        let target = branch
            .get("target_id")
            .and_then(|t| t.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        writeln!(stdout, "{} refs/heads/{}", target, name).unwrap();
    }

    writeln!(stdout).unwrap();
    stdout.flush().unwrap();
}

fn handle_push(_base_url: &str, _repo_id: &str, _cmd: &str, stdout: &mut dyn Write) {
    writeln!(stdout, "error push not yet supported via git remote helper").unwrap();
    writeln!(stdout).unwrap();
    stdout.flush().unwrap();
}
