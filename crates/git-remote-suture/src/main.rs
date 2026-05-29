use base64::Engine;
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
    let tree_url = format!("{}/repos/{}/tree/main", base_url, repo_id);
    let client = reqwest::blocking::Client::new();

    let tree_resp = match client.get(&tree_url).send() {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            eprintln!("fetch tree failed: HTTP {}", r.status());
            writeln!(stdout).unwrap();
            stdout.flush().unwrap();
            return;
        }
        Err(e) => {
            eprintln!("fetch tree failed: {}", e);
            writeln!(stdout).unwrap();
            stdout.flush().unwrap();
            return;
        }
    };

    let tree_body: serde_json::Value = match tree_resp.json() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("parse tree failed: {}", e);
            writeln!(stdout).unwrap();
            stdout.flush().unwrap();
            return;
        }
    };

    let branches = fetch_branches(base_url, repo_id).unwrap_or_default();

    let files = tree_body
        .get("files")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default();

    for (name, target_hex) in &branches {
        writeln!(stdout, "{} refs/heads/{}", target_hex, name).unwrap();
    }

    // fast-import header
    for (name, _target_hex) in &branches {
        writeln!(stdout, "commit refs/heads/{}", name).unwrap();
        writeln!(stdout, "committer Suture Hub <hub@suture.dev> now +0000").unwrap();
        writeln!(stdout, "data <<EOFMARKER").unwrap();
        writeln!(stdout, "Imported from suture hub").unwrap();
        writeln!(stdout, "EOFMARKER").unwrap();

        for file_entry in &files {
            let path = file_entry
                .get("path")
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let content_b64 = file_entry
                .get("content_b64")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            if path.is_empty() {
                continue;
            }
            let content = base64::engine::general_purpose::STANDARD
                .decode(content_b64)
                .unwrap_or_default();
            writeln!(stdout, "M 644 inline {}", path).unwrap();
            writeln!(stdout, "data {}", content.len()).unwrap();
            stdout.write_all(&content).unwrap();
            writeln!(stdout).unwrap();
        }
    }

    writeln!(stdout, "done").unwrap();
    stdout.flush().unwrap();
}

fn handle_push(base_url: &str, repo_id: &str, cmd: &str, stdout: &mut dyn Write) {
    let parts: Vec<&str> = cmd.strip_prefix("push ").unwrap_or("").split(':').collect();
    if parts.len() != 2 {
        writeln!(stdout, "error invalid push refspec").unwrap();
        writeln!(stdout).unwrap();
        stdout.flush().unwrap();
        return;
    }

    let _local_ref = parts[0];
    let remote_ref = parts[1];
    let branch_name = remote_ref.trim_start_matches("refs/heads/");

    let client = reqwest::blocking::Client::new();
    let create_branch_url = format!("{}/repos/{}/branches", base_url, repo_id);

    let body = serde_json::json!({
        "repo_id": repo_id,
        "branch_name": branch_name,
    });

    match client.post(&create_branch_url).json(&body).send() {
        Ok(resp) if resp.status().is_success() => {
            writeln!(stdout, "ok {}", remote_ref).unwrap();
        }
        Ok(resp) => {
            let text = resp.text().unwrap_or_default();
            writeln!(stdout, "error {}", text).unwrap();
        }
        Err(e) => {
            writeln!(stdout, "error {}", e).unwrap();
        }
    }

    writeln!(stdout).unwrap();
    stdout.flush().unwrap();
}
