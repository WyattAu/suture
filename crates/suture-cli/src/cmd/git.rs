use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

struct GitCommit {
    tree: String,
    parents: Vec<String>,
    author: String,
    message: String,
}

impl Clone for GitCommit {
    fn clone(&self) -> Self {
        Self {
            tree: self.tree.clone(),
            parents: self.parents.clone(),
            author: self.author.clone(),
            message: self.message.clone(),
        }
    }
}

pub(crate) enum GitAction {
    Import { path: Option<String> },
    Log { path: Option<String> },
    Status { path: Option<String> },
    Driver { action: DriverAction },
}

pub(crate) enum DriverAction {
    Install,
    Uninstall,
    List,
}

pub(crate) async fn cmd_git(action: GitAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        GitAction::Import { path } => git_import(path),
        GitAction::Log { path } => git_log(path),
        GitAction::Status { path } => git_status(path),
        GitAction::Driver { action } => match action {
            DriverAction::Install => cmd_driver_install(),
            DriverAction::Uninstall => cmd_driver_uninstall(),
            DriverAction::List => cmd_driver_list(),
        },
    }
}

fn find_git_dir(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut current = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if current.is_file() {
        current.pop();
    }
    loop {
        let marker = current.join(".git");
        if marker.exists() {
            if marker.is_dir() {
                return Ok(marker);
            }
            if marker.is_file() {
                let content = std::fs::read_to_string(&marker)?;
                if let Some(dir) = content.strip_prefix("gitdir: ") {
                    let dir = dir.trim();
                    return Ok(if Path::new(dir).is_relative() {
                        current.join(dir)
                    } else {
                        PathBuf::from(dir)
                    });
                }
            }
        }
        if !current.pop() {
            return Err(format!("not a git repository: {}", path.display()).into());
        }
    }
}

fn read_git_object(git_dir: &Path, sha: &str) -> Result<(String, Vec<u8>), Box<dyn std::error::Error>> {
    if sha.len() < 4 {
        return Err("SHA too short".into());
    }
    let obj_path = git_dir.join("objects").join(&sha[..2]).join(&sha[2..]);
    let compressed = std::fs::read(&obj_path).map_err(|e| {
        format!(
            "cannot read git object {}: {} (packed objects are not supported)",
            sha, e
        )
    })?;
    let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    let null_pos = decompressed
        .iter()
        .position(|&b| b == 0)
        .ok_or("invalid git object format")?;
    let header = String::from_utf8_lossy(&decompressed[..null_pos]);
    let content = decompressed[null_pos + 1..].to_vec();
    let obj_type = header.split(' ').next().unwrap_or("").to_string();
    Ok((obj_type, content))
}

fn parse_commit(data: &[u8]) -> Result<GitCommit, Box<dyn std::error::Error>> {
    let text = String::from_utf8_lossy(data);
    let mut tree = String::new();
    let mut parents = Vec::new();
    let mut author = String::new();
    let mut in_message = false;
    let mut message_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        if in_message {
            message_lines.push(line);
        } else if line.is_empty() {
            in_message = true;
        } else if let Some(rest) = line.strip_prefix("tree ") {
            tree = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("parent ") {
            parents.push(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("author ") {
            author = rest.to_string();
        }
    }

    let message = message_lines.join("\n").trim().to_string();
    Ok(GitCommit {
        tree,
        parents,
        author,
        message,
    })
}

fn parse_tree_entries(data: &[u8]) -> Vec<(String, String, String)> {
    let mut entries = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let space = match data[pos..].iter().position(|&b| b == b' ') {
            Some(i) => pos + i,
            None => break,
        };
        let null = match data[space + 1..].iter().position(|&b| b == 0) {
            Some(i) => space + 1 + i,
            None => break,
        };
        if null + 21 > data.len() {
            break;
        }
        let mode = String::from_utf8_lossy(&data[pos..space]).to_string();
        let name = String::from_utf8_lossy(&data[space + 1..null]).to_string();
        const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
        let sha: String = {
            let mut s = String::with_capacity(40);
            for b in &data[null + 1..null + 21] {
                s.push(HEX_CHARS[(b >> 4) as usize] as char);
                s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
            }
            s
        };
        entries.push((mode, name, sha));
        pos = null + 21;
    }
    entries
}

fn flatten_tree(
    git_dir: &Path,
    tree_sha: &str,
    prefix: &str,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut files = HashMap::new();
    let (obj_type, data) = read_git_object(git_dir, tree_sha)?;
    if obj_type != "tree" {
        return Err(format!("expected tree object, got {}", obj_type).into());
    }
    for (mode, name, sha) in parse_tree_entries(&data) {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        match mode.as_str() {
            "40000" => {
                let sub = flatten_tree(git_dir, &sha, &path)?;
                files.extend(sub);
            }
            "100644" | "100755" => {
                files.insert(path, sha);
            }
            _ => {}
        }
    }
    Ok(files)
}

fn read_head_sha(git_dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let head_path = git_dir.join("HEAD");
    let content = std::fs::read_to_string(&head_path)?.trim().to_string();
    if let Some(ref_path) = content.strip_prefix("ref: ") {
        let full_path = git_dir.join(ref_path);
        if full_path.exists() {
            return Ok(std::fs::read_to_string(&full_path)?.trim().to_string());
        }
        let packed = git_dir.join("packed-refs");
        if packed.exists() {
            for line in std::fs::read_to_string(&packed)?.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.starts_with('^') {
                    continue;
                }
                if let Some((sha, r)) = line.split_once(' ') && r == ref_path {
                    return Ok(sha.to_string());
                }
            }
        }
        return Err(format!("branch ref not found: {}", ref_path).into());
    }
    if content.len() == 40 && content.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(content);
    }
    Err("invalid HEAD".into())
}

fn walk_commits(git_dir: &Path) -> Result<Vec<(String, GitCommit)>, Box<dyn std::error::Error>> {
    let head_sha = match read_head_sha(git_dir) {
        Ok(sha) => sha,
        Err(_) => return Ok(Vec::new()),
    };
    let mut commits = Vec::new();
    let mut current = head_sha;
    let mut seen = std::collections::HashSet::new();
    while !current.is_empty() && seen.insert(current.clone()) {
        let commit = match read_commit(git_dir, &current) {
            Ok(c) => c,
            Err(_) => break,
        };
        let parent = commit.parents.first().cloned().unwrap_or_default();
        commits.push((current.clone(), commit));
        current = parent;
    }
    commits.reverse();
    Ok(commits)
}

fn read_commit(git_dir: &Path, sha: &str) -> Result<GitCommit, Box<dyn std::error::Error>> {
    let (obj_type, data) = read_git_object(git_dir, sha)?;
    if obj_type != "commit" {
        return Err(format!("expected commit, got {}", obj_type).into());
    }
    parse_commit(&data)
}

fn read_reflog(git_dir: &Path) -> Vec<(String, String)> {
    let log_path = git_dir.join("logs").join("HEAD");
    if !log_path.exists() {
        return Vec::new();
    }
    let Ok(content) = std::fs::read_to_string(&log_path) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (header, message) = if let Some(pos) = line.find('\t') {
            (&line[..pos], line[pos + 1..].to_string())
        } else if let Some(pos) = line.find("  ") {
            (&line[..pos], line[pos + 2..].to_string())
        } else {
            continue;
        };
        let mut parts = header.splitn(3, ' ');
        parts.next();
        let new_sha = match parts.next() {
            Some(sha) if sha.len() == 40 => sha.to_string(),
            _ => continue,
        };
        if seen.insert(new_sha.clone()) {
            entries.push((new_sha, message));
        }
    }
    entries
}

fn read_branches(git_dir: &Path) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut branches = Vec::new();
    let refs_dir = git_dir.join("refs").join("heads");
    if refs_dir.exists() {
        collect_branches(&refs_dir, &refs_dir, &mut branches)?;
    }
    let packed = git_dir.join("packed-refs");
    if packed.exists() {
        for line in std::fs::read_to_string(&packed)?.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with('^') {
                continue;
            }
            if let Some((sha, ref_name)) = line.split_once(' ')
                && let Some(name) = ref_name.strip_prefix("refs/heads/")
            {
                branches.push((name.to_string(), sha.to_string()));
            }
        }
    }
    Ok(branches)
}

fn collect_branches(
    base: &Path,
    dir: &Path,
    out: &mut Vec<(String, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_branches(base, &path, out)?;
        } else if path.is_file() {
            let name = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let sha = std::fs::read_to_string(&path)?.trim().to_string();
            out.push((name, sha));
        }
    }
    Ok(())
}

fn truncate_msg(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

const SUTURE_DRIVER_SCRIPT_PATH: &str = ".suture/git-merge-driver.sh";

fn gitattributes_patterns() -> &'static str {
    "*.docx merge=suture
*.docm merge=suture
*.xlsx merge=suture
*.xlsm merge=suture
*.pptx merge=suture
*.pptm merge=suture
*.json merge=suture
*.jsonl merge=suture
*.yaml merge=suture
*.yml merge=suture
*.toml merge=suture
*.csv merge=suture
*.tsv merge=suture
*.xml merge=suture
*.xsl merge=suture
*.svg merge=suture
*.md merge=suture
*.markdown merge=suture
*.otio merge=suture
*.sql merge=suture"
}

fn merge_driver_script() -> &'static str {
    r#"#!/bin/sh
# Suture merge driver — called by Git with: $LOCAL $BASE $REMOTE
# $LOCAL = ours (current branch version)
# $BASE = common ancestor
# $REMOTE = theirs (incoming branch version)
# Exit 0 with merged content on stdout
BASE_FILE="$3"
# Write the merged result using suture merge-file
suture merge-file --driver auto "$BASE_FILE" "$2" "$4" -o -
exit $?
"#
}

fn ensure_git_repo() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()?;
    if !output.status.success() {
        return Err("not a Git repository".into());
    }
    Ok(())
}

fn cmd_driver_install() -> Result<(), Box<dyn std::error::Error>> {
    ensure_git_repo()?;

    std::fs::create_dir_all(".suture")?;

    std::fs::write(SUTURE_DRIVER_SCRIPT_PATH, merge_driver_script())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(SUTURE_DRIVER_SCRIPT_PATH, perms)?;
    }

    let patterns = gitattributes_patterns();
    let gitattributes_path = std::path::Path::new(".gitattributes");
    if gitattributes_path.exists() {
        let existing = std::fs::read_to_string(gitattributes_path)?;
        let filtered: String = existing
            .lines()
            .filter(|line| !line.contains("merge=suture"))
            .collect::<Vec<_>>()
            .join("\n");
        let needs_newline = !filtered.is_empty();
        let new_content = if needs_newline {
            format!("{filtered}\n{patterns}")
        } else {
            patterns.to_string()
        };
        std::fs::write(gitattributes_path, new_content)?;
    } else {
        std::fs::write(gitattributes_path, patterns)?;
    }

    let script_path = std::path::Path::new(SUTURE_DRIVER_SCRIPT_PATH);
    let absolute_script = if script_path.is_absolute() {
        script_path.display().to_string()
    } else {
        std::env::current_dir()?
            .join(script_path)
            .display()
            .to_string()
    };

    std::process::Command::new("git")
        .args([
            "config",
            "merge.suture.name",
            "Suture Semantic Merge Driver",
        ])
        .output()?;
    std::process::Command::new("git")
        .args(["config", "merge.suture.driver", &absolute_script])
        .output()?;
    std::process::Command::new("git")
        .args(["config", "merge.suture.recursive", "binary"])
        .output()?;

    println!("Suture merge driver installed successfully.");
    println!();
    println!("  Merge driver script:  {SUTURE_DRIVER_SCRIPT_PATH}");
    println!("  Git config section:   merge.suture");
    println!("  Gitattributes:        updated with 20 file patterns");
    println!();
    println!("Next steps:");
    println!("  git add .gitattributes .suture/git-merge-driver.sh");
    println!("  git commit -m \"Configure suture merge driver\"");

    Ok(())
}

fn cmd_driver_uninstall() -> Result<(), Box<dyn std::error::Error>> {
    ensure_git_repo()?;

    let gitattributes_path = std::path::Path::new(".gitattributes");
    if gitattributes_path.exists() {
        let existing = std::fs::read_to_string(gitattributes_path)?;
        let filtered: String = existing
            .lines()
            .filter(|line| !line.contains("merge=suture"))
            .collect::<Vec<_>>()
            .join("\n");
        if filtered.trim().is_empty() {
            std::fs::remove_file(gitattributes_path)?;
        } else {
            std::fs::write(gitattributes_path, filtered)?;
        }
    }

    std::process::Command::new("git")
        .args(["config", "--remove-section", "merge.suture"])
        .output()?;

    let script_path = std::path::Path::new(SUTURE_DRIVER_SCRIPT_PATH);
    if script_path.exists() {
        std::fs::remove_file(script_path)?;
    }

    println!("Suture merge driver uninstalled successfully.");

    Ok(())
}

fn cmd_driver_list() -> Result<(), Box<dyn std::error::Error>> {
    ensure_git_repo()?;

    let gitattributes_path = std::path::Path::new(".gitattributes");
    if gitattributes_path.exists() {
        let content = std::fs::read_to_string(gitattributes_path)?;
        let patterns: Vec<&str> = content
            .lines()
            .filter(|line| line.contains("merge=suture"))
            .collect();
        if patterns.is_empty() {
            println!("  .gitattributes:    no suture patterns found");
        } else {
            println!("  .gitattributes:    {} patterns configured", patterns.len());
            for p in &patterns {
                println!("    {}", p.trim());
            }
        }
    } else {
        println!("  .gitattributes:    not found");
    }

    let name = std::process::Command::new("git")
        .args(["config", "--get", "merge.suture.name"])
        .output()?;
    let driver = std::process::Command::new("git")
        .args(["config", "--get", "merge.suture.driver"])
        .output()?;
    let recursive = std::process::Command::new("git")
        .args(["config", "--get", "merge.suture.recursive"])
        .output()?;

    if name.status.success() {
        println!(
            "  merge.suture.name:      {}",
            String::from_utf8_lossy(&name.stdout).trim()
        );
        println!(
            "  merge.suture.driver:    {}",
            String::from_utf8_lossy(&driver.stdout).trim()
        );
        println!(
            "  merge.suture.recursive: {}",
            String::from_utf8_lossy(&recursive.stdout).trim()
        );
    } else {
        println!("  git config:        merge.suture section not found");
    }

    let script_path = std::path::Path::new(SUTURE_DRIVER_SCRIPT_PATH);
    if script_path.exists() {
        println!("  driver script:     {} (exists)", SUTURE_DRIVER_SCRIPT_PATH);
    } else {
        println!("  driver script:     {} (missing)", SUTURE_DRIVER_SCRIPT_PATH);
    }

    Ok(())
}

fn write_blob_to_disk(git_dir: &Path, path: &str, blob_sha: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (_, data) = read_git_object(git_dir, blob_sha)?;
    let full = std::path::Path::new(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(full, &data)?;
    Ok(())
}

fn git_import(path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let git_path = path.as_deref().unwrap_or(".");
    let git_dir = find_git_dir(Path::new(git_path))?;
    let commits = walk_commits(&git_dir)?;
    if commits.is_empty() {
        println!("No commits found in Git repository.");
        return Ok(());
    }

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let status = repo.status()?;
    let is_empty = status.patch_count <= 1;

    let latest_git_msg = &commits.last().unwrap().1.message;
    if !is_empty
        && let Ok((_, head_id)) = repo.head()
        && let Some(patch) = repo.dag().get_patch(&head_id)
        && patch.message == *latest_git_msg
    {
        println!(
            "Already up to date (latest commit: \"{}\")",
            latest_git_msg
        );
        return Ok(());
    }

    if !is_empty {
        let branch_name = "git-import/main";
        if !repo.list_branches().iter().any(|(n, _)| n == branch_name) {
            repo.create_branch(branch_name, None)?;
        }
        repo.checkout(branch_name)?;
    }

    println!("Importing {} commits from Git...", commits.len());

    let mut prev_tree: HashMap<String, String> = HashMap::new();
    let total = commits.len();
    let mut imported = 0usize;

    for (_sha, commit) in commits.iter() {
        let tree = match flatten_tree(&git_dir, &commit.tree, "") {
            Ok(t) => t,
            Err(e) => {
                eprintln!("warning: skipping commit (tree read error): {}", e);
                prev_tree = HashMap::new();
                continue;
            }
        };

        let mut to_add: Vec<(String, String)> = Vec::new();
        let mut to_modify: Vec<(String, String)> = Vec::new();
        let mut to_delete: Vec<String> = Vec::new();

        for (p, s) in &tree {
            match prev_tree.get(p) {
                Some(prev) if prev == s => {}
                Some(_) => to_modify.push((p.clone(), s.clone())),
                None => to_add.push((p.clone(), s.clone())),
            }
        }
        for p in prev_tree.keys() {
            if !tree.contains_key(p) {
                to_delete.push(p.clone());
            }
        }

        if to_add.is_empty() && to_modify.is_empty() && to_delete.is_empty() {
            prev_tree = tree;
            continue;
        }

        for (p, blob_sha) in &to_add {
            if let Err(e) = write_blob_to_disk(&git_dir, p, blob_sha) {
                eprintln!("warning: cannot write {}: {}", p, e);
            }
        }
        for (p, blob_sha) in &to_modify {
            if let Err(e) = write_blob_to_disk(&git_dir, p, blob_sha) {
                eprintln!("warning: cannot write {}: {}", p, e);
            }
        }
        for p in &to_delete {
            let full = std::path::Path::new(p);
            if full.exists() {
                let _ = std::fs::remove_file(full);
            }
        }

        for (p, _) in &to_add {
            if let Err(e) = repo.add(p) {
                eprintln!("warning: cannot stage {}: {}", p, e);
            }
        }
        for (p, _) in &to_modify {
            if let Err(e) = repo.add(p) {
                eprintln!("warning: cannot stage {}: {}", p, e);
            }
        }
        for p in &to_delete {
            if let Err(e) = repo.add(p) {
                eprintln!("warning: cannot stage {}: {}", p, e);
            }
        }

        let msg = if commit.message.is_empty() {
            "(no message)".to_string()
        } else {
            commit.message.clone()
        };

        match repo.commit(&msg) {
            Ok(id) => {
                imported += 1;
                if imported.is_multiple_of(100) || imported == total {
                    let short = id.to_hex().chars().take(8).collect::<String>();
                    println!(
                        "  [{}/{}] {} {}",
                        imported,
                        total,
                        short,
                        truncate_msg(&msg, 50)
                    );
                }
            }
            Err(e) if e.to_string().contains("nothing to commit") => {
                continue;
            }
            Err(e) => return Err(e.into()),
        }

        prev_tree = tree;
    }

    println!("Imported {} commits.", imported);
    Ok(())
}

fn git_log(path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let git_path = path.as_deref().unwrap_or(".");
    let git_dir = find_git_dir(Path::new(git_path))?;
    let reflog_entries = read_reflog(&git_dir);
    if reflog_entries.is_empty() {
        println!("No commits found in Git repository.");
        return Ok(());
    }

    let branches = read_branches(&git_dir)?;
    let mut sha_to_branch: HashMap<String, String> = HashMap::new();
    for (name, sha) in &branches {
        sha_to_branch.insert(sha.clone(), name.clone());
    }

    let mut prev_tree: HashMap<String, String> = HashMap::new();
    let mut commit_infos: Vec<(String, String, usize, usize, usize)> =
        Vec::new();

    for (sha, _reflog_msg) in &reflog_entries {
        let commit = match read_commit(&git_dir, sha) {
            Ok(c) => c,
            Err(_) => {
                commit_infos.push((sha.clone(), "(cannot read)".to_string(), 0, 0, 0));
                continue;
            }
        };
        let tree = match flatten_tree(&git_dir, &commit.tree, "") {
            Ok(t) => t,
            Err(_) => {
                commit_infos.push((sha.clone(), commit.message.clone(), 0, 0, 0));
                continue;
            }
        };

        let mut added = 0usize;
        let mut modified = 0usize;
        let mut deleted = 0usize;

        for (p, s) in &tree {
            match prev_tree.get(p) {
                None => added += 1,
                Some(prev) if prev != s => modified += 1,
                _ => {}
            }
        }
        for p in prev_tree.keys() {
            if !tree.contains_key(p) {
                deleted += 1;
            }
        }

        commit_infos.push((sha.clone(), commit.message.clone(), added, modified, deleted));
        prev_tree = tree;
    }

    let total = commit_infos.len();
    for (pos, (sha, message, added, modified, deleted)) in
        commit_infos.iter().enumerate().rev()
    {
        let short = &sha[..8];
        let branch = sha_to_branch
            .get(sha)
            .map(|s| s.as_str())
            .unwrap_or("-");
        let is_head = pos == total - 1;
        let marker = if is_head { " * " } else { "   " };

        let change_str = if *added + *modified + *deleted == 0 {
            "(no changes)".to_string()
        } else {
            let mut parts = Vec::new();
            if *added > 0 {
                parts.push(format!("{} added", added));
            }
            if *modified > 0 {
                parts.push(format!("{} modified", modified));
            }
            if *deleted > 0 {
                parts.push(format!("{} deleted", deleted));
            }
            format!("({})", parts.join(", "))
        };

        println!(
            "{}{}  {:20} {} {}",
            marker,
            short,
            branch,
            truncate_msg(message, 50),
            change_str
        );
    }

    println!("\n{} commits", total);
    Ok(())
}

fn git_status(path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let git_path = path.as_deref().unwrap_or(".");
    let git_dir = find_git_dir(Path::new(git_path))?;
    let commits = walk_commits(&git_dir)?;
    let branches = read_branches(&git_dir)?;

    let file_count = if let Some((_, commit)) = commits.last() {
        flatten_tree(&git_dir, &commit.tree, "")
            .map(|t| t.len())
            .unwrap_or(0)
    } else {
        0
    };

    println!("Git repository: {}", git_path);
    println!("  Commits: {}", commits.len());
    println!("  Branches: {}", branches.len());
    println!("  Files in latest tree: {}", file_count);

    let suture_path = std::path::Path::new(".suture");
    if !suture_path.exists() {
        println!("  Suture repo: not found (run `suture init` first)");
    } else {
        match suture_core::repository::Repository::open(std::path::Path::new(".")) {
            Ok(repo) => {
                let s = repo.status()?;
                if s.patch_count <= 1 {
                    println!("  Suture repo: empty (will import onto main)");
                } else {
                    println!(
                        "  Suture repo: has history (will import to git-import/main)"
                    );
                }
            }
            Err(_) => {
                println!("  Suture repo: corrupted or invalid");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_log_line() {
        let line = "0000000000000000000000000000000000000000 abc123def4567890123456789012345678901234 John Doe <john@example.com> 1700000000 +0000\tInitial commit";
        let entries = parse_reflog_line(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].0,
            "abc123def4567890123456789012345678901234"
        );
        assert_eq!(entries[0].1, "Initial commit");
    }

    #[test]
    fn test_parse_git_log_line_double_space() {
        let line = "0000000000000000000000000000000000000000 abc123def4567890123456789012345678901234 John Doe <john@example.com> 1700000000 +0000  Initial commit";
        let entries = parse_reflog_line(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1, "Initial commit");
    }

    #[test]
    fn test_parse_git_log_line_empty() {
        let entries = parse_reflog_line("");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_commit_object() {
        let data = b"tree abc123def4567890123456789012345678901234\nparent def4567890123456789012345678901234567890\nauthor Alice <alice@example.com> 1700000000 +0000\ncommitter Alice <alice@example.com> 1700000000 +0000\n\nAdd feature X\n\nDetailed description here.";
        let commit = parse_commit(data).unwrap();
        assert_eq!(
            commit.tree,
            "abc123def4567890123456789012345678901234"
        );
        assert_eq!(commit.parents.len(), 1);
        assert_eq!(
            commit.parents[0],
            "def4567890123456789012345678901234567890"
        );
        assert!(commit.author.contains("Alice"));
        assert_eq!(commit.message, "Add feature X\n\nDetailed description here.");
    }

    #[test]
    fn test_parse_commit_object_root() {
        let data = b"tree abc123def4567890123456789012345678901234\nauthor Bob <bob@example.com> 1700000000 +0000\ncommitter Bob <bob@example.com> 1700000000 +0000\n\nInitial commit";
        let commit = parse_commit(data).unwrap();
        assert_eq!(commit.parents.len(), 0);
        assert_eq!(commit.message, "Initial commit");
    }

    #[test]
    fn test_parse_commit_object_merge() {
        let data = b"tree abc123def4567890123456789012345678901234\nparent def4567890123456789012345678901234567890\nparent 1111111111111111111111111111111111111111\nauthor Alice <alice@example.com> 1700000000 +0000\ncommitter Alice <alice@example.com> 1700000000 +0000\n\nMerge feature branch";
        let commit = parse_commit(data).unwrap();
        assert_eq!(commit.parents.len(), 2);
        assert_eq!(commit.message, "Merge feature branch");
    }

    #[test]
    fn test_parse_tree_object() {
        let mut data = Vec::new();
        data.extend_from_slice(b"100644");
        data.push(b' ');
        data.extend_from_slice(b"readme.txt");
        data.push(0);
        data.extend_from_slice(&[0xaa; 20]);
        data.extend_from_slice(b"40000");
        data.push(b' ');
        data.extend_from_slice(b"src");
        data.push(0);
        data.extend_from_slice(&[0xbb; 20]);
        data.extend_from_slice(b"100755");
        data.push(b' ');
        data.extend_from_slice(b"build.sh");
        data.push(0);
        data.extend_from_slice(&[0xcc; 20]);

        let entries = parse_tree_entries(&data);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].0, "100644");
        assert_eq!(entries[0].1, "readme.txt");
        assert_eq!(entries[0].2, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(entries[1].0, "40000");
        assert_eq!(entries[1].1, "src");
        assert_eq!(entries[1].2, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert_eq!(entries[2].0, "100755");
        assert_eq!(entries[2].1, "build.sh");
        assert_eq!(entries[2].2, "cccccccccccccccccccccccccccccccccccccccc");
    }

    #[test]
    fn test_parse_tree_object_empty() {
        let entries = parse_tree_entries(&[]);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_zlib_decompress() {
        let original = b"blob 5\0hello";
        let mut compressed = Vec::new();
        {
            let mut encoder = flate2::write::ZlibEncoder::new(
                &mut compressed,
                flate2::Compression::default(),
            );
            std::io::Write::write_all(&mut encoder, original).unwrap();
            encoder.finish().unwrap();
        }
        let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_zlib_decompress_empty() {
        let original = b"";
        let mut compressed = Vec::new();
        {
            let mut encoder = flate2::write::ZlibEncoder::new(
                &mut compressed,
                flate2::Compression::default(),
            );
            std::io::Write::write_all(&mut encoder, original).unwrap();
            encoder.finish().unwrap();
        }
        let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_import_empty_repo() {
        let dir = std::env::temp_dir().join("suture-test-git-empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git").join("objects")).unwrap();
        std::fs::create_dir_all(dir.join(".git").join("refs").join("heads")).unwrap();
        std::fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();

        let git_dir = find_git_dir(&dir).unwrap();
        let commits = walk_commits(&git_dir).unwrap();
        assert!(commits.is_empty(), "empty git repo should have no commits");

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn parse_reflog_line(line: &str) -> Vec<(String, String)> {
        let mut entries = Vec::new();
        let line = line.trim();
        if line.is_empty() {
            return entries;
        }
        let (header, message) = if let Some(pos) = line.find('\t') {
            (&line[..pos], line[pos + 1..].to_string())
        } else if let Some(pos) = line.find("  ") {
            (&line[..pos], line[pos + 2..].to_string())
        } else {
            return entries;
        };
        let mut parts = header.splitn(3, ' ');
        parts.next();
        let new_sha = match parts.next() {
            Some(sha) if sha.len() == 40 => sha.to_string(),
            _ => return entries,
        };
        entries.push((new_sha, message));
        entries
    }

    #[test]
    fn test_gitattributes_content() {
        let patterns = gitattributes_patterns();
        let lines: Vec<&str> = patterns.lines().collect();
        assert_eq!(lines.len(), 20);
        assert!(lines.iter().all(|l| l.contains("merge=suture")));
        assert!(lines.iter().any(|l| l.starts_with("*.json")));
        assert!(lines.iter().any(|l| l.starts_with("*.yaml")));
        assert!(lines.iter().any(|l| l.starts_with("*.yml")));
        assert!(lines.iter().any(|l| l.starts_with("*.toml")));
        assert!(lines.iter().any(|l| l.starts_with("*.csv")));
        assert!(lines.iter().any(|l| l.starts_with("*.xml")));
        assert!(lines.iter().any(|l| l.starts_with("*.md")));
        assert!(lines.iter().any(|l| l.starts_with("*.docx")));
        assert!(lines.iter().any(|l| l.starts_with("*.xlsx")));
        assert!(lines.iter().any(|l| l.starts_with("*.pptx")));
        assert!(lines.iter().any(|l| l.starts_with("*.sql")));
        assert!(lines.iter().any(|l| l.starts_with("*.otio")));
        assert!(lines.iter().any(|l| l.starts_with("*.svg")));
    }

    #[test]
    fn test_merge_driver_script() {
        let script = merge_driver_script();
        assert!(script.starts_with("#!/bin/sh"));
        assert!(script.contains("suture merge-file --driver auto"));
        assert!(script.contains("$BASE_FILE"));
        assert!(script.contains("$2"));
        assert!(script.contains("$4"));
        assert!(script.contains("exit $?"));
    }

    #[test]
    fn test_install_uninstall_flow() {
        let _cwd = crate::cwd_guard();
        let dir = std::env::temp_dir().join("suture-test-driver-flow");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::create_dir_all(dir.join(".git/objects")).unwrap();
        std::fs::create_dir_all(dir.join(".git/refs/heads")).unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        cmd_driver_install().unwrap();

        let gitattributes = std::fs::read_to_string(".gitattributes").unwrap();
        assert!(gitattributes.contains("*.json merge=suture"));
        assert!(gitattributes.contains("*.sql merge=suture"));

        let script = std::path::Path::new(SUTURE_DRIVER_SCRIPT_PATH);
        assert!(script.exists());
        let script_content = std::fs::read_to_string(script).unwrap();
        assert!(script_content.contains("suture merge-file"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(script).unwrap().permissions();
            assert_eq!(perms.mode() & 0o111, 0o111);
        }

        cmd_driver_install().unwrap();

        let gitattributes2 = std::fs::read_to_string(".gitattributes").unwrap();
        let suture_count = gitattributes2
            .lines()
            .filter(|l| l.contains("merge=suture"))
            .count();
        assert_eq!(suture_count, 20, "re-install should not duplicate patterns");

        cmd_driver_uninstall().unwrap();

        assert!(!script.exists());
        let gitattributes3 = std::fs::read_to_string(".gitattributes").unwrap_or_default();
        assert!(!gitattributes3.contains("merge=suture"));

        std::env::set_current_dir(&prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
