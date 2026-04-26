#[derive(Debug, Clone)]
struct HunkLine {
    kind: HunkLineKind,
    content: String,
}

#[derive(Debug, Clone, PartialEq)]
enum HunkLineKind {
    Context,
    Add,
    Remove,
}

#[derive(Debug, Clone)]
struct FileDiff {
    old_path: String,
    new_path: String,
    hunks: Vec<Hunk>,
}

#[derive(Debug, Clone)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    #[allow(dead_code)]
    new_start: usize,
    new_count: usize,
    lines: Vec<HunkLine>,
}

fn strip_path_prefix(p: &str) -> &str {
    p.strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p)
}

fn parse_unified_diff(text: &str) -> Result<Vec<FileDiff>, Box<dyn std::error::Error>> {
    let mut diffs: Vec<FileDiff> = Vec::new();
    let mut old_path = String::new();
    let mut new_path = String::new();
    let mut in_file = false;
    let mut hunk_lines: Vec<HunkLine> = Vec::new();
    let mut hunk_header: Option<(usize, usize, usize, usize)> = None;
    let mut current_hunks: Vec<Hunk> = Vec::new();

    let flush_hunk = |hunks: &mut Vec<Hunk>,
                      lines: &mut Vec<HunkLine>,
                      header: &mut Option<(usize, usize, usize, usize)>| {
        if let Some((os, oc, ns, nc)) = header.take() {
            hunks.push(Hunk {
                old_start: os,
                old_count: oc,
                new_start: ns,
                new_count: nc,
                lines: std::mem::take(lines),
            });
        } else {
            lines.clear();
        }
    };

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("--- ") {
            if in_file {
                flush_hunk(&mut current_hunks, &mut hunk_lines, &mut hunk_header);
                if !current_hunks.is_empty() {
                    diffs.push(FileDiff {
                        old_path: old_path.clone(),
                        new_path: new_path.clone(),
                        hunks: std::mem::take(&mut current_hunks),
                    });
                }
            }
            old_path = strip_path_prefix(rest.trim_end()).to_string();
            new_path = String::new();
            in_file = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            new_path = strip_path_prefix(rest.trim_end()).to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("@@ ") {
            flush_hunk(&mut current_hunks, &mut hunk_lines, &mut hunk_header);
            let parts: Vec<&str> = rest.split_whitespace().collect();
            let parse_range = |s: &str| -> Option<(usize, usize)> {
                let s = s.trim_start_matches('-').trim_start_matches('+');
                let mut iter = s.splitn(2, ',');
                let start: usize = iter.next()?.parse().ok()?;
                let count: usize = iter.next().and_then(|n| n.parse().ok()).unwrap_or(1);
                Some((start, count))
            };
            let old_range = parts.first().and_then(|s| parse_range(s));
            let new_range = parts.get(1).and_then(|s| parse_range(s));
            if let (Some((os, oc)), Some((ns, nc))) = (old_range, new_range) {
                hunk_header = Some((os, oc, ns, nc));
            }
            continue;
        }
        if hunk_header.is_some() {
            let ch = line.chars().next();
            let kind = match ch {
                Some('+') => HunkLineKind::Add,
                Some('-') => HunkLineKind::Remove,
                _ => HunkLineKind::Context,
            };
            let content = match ch {
                Some('+') | Some('-') => line[1..].to_string(),
                _ => line.to_string(),
            };
            hunk_lines.push(HunkLine { kind, content });
        }
    }

    flush_hunk(&mut current_hunks, &mut hunk_lines, &mut hunk_header);
    if in_file && !current_hunks.is_empty() {
        diffs.push(FileDiff {
            old_path,
            new_path,
            hunks: current_hunks,
        });
    }

    Ok(diffs)
}

fn apply_hunk(
    file_lines: &mut Vec<String>,
    hunk: &Hunk,
    reverse: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let pos = hunk.old_start.saturating_sub(1);
    if pos > file_lines.len() {
        let msg = format!(
            "hunk at line {} is out of range (file has {} lines)",
            hunk.old_start,
            file_lines.len()
        );
        return Err(msg.into());
    }

    let mut new_lines: Vec<String> = Vec::new();
    for line in &hunk.lines {
        let effective_kind = if reverse {
            match line.kind {
                HunkLineKind::Add => HunkLineKind::Remove,
                HunkLineKind::Remove => HunkLineKind::Add,
                HunkLineKind::Context => HunkLineKind::Context,
            }
        } else {
            line.kind.clone()
        };

        match effective_kind {
            HunkLineKind::Context => new_lines.push(line.content.clone()),
            HunkLineKind::Add => new_lines.push(line.content.clone()),
            HunkLineKind::Remove => {}
        }
    }

    let old_lines_count = hunk
        .lines
        .iter()
        .filter(|l| l.kind != HunkLineKind::Add)
        .count();
    let effective_old_count = if reverse {
        hunk.new_count
    } else {
        hunk.old_count
    };
    let remove_count = if old_lines_count > 0 {
        old_lines_count
    } else {
        effective_old_count
    };

    let end = std::cmp::min(pos + remove_count, file_lines.len());
    file_lines.drain(pos..end);
    for (i, line) in new_lines.into_iter().enumerate() {
        file_lines.insert(pos + i, line);
    }

    Ok(())
}

fn stat_for_diffs(diffs: &[FileDiff]) -> String {
    let mut output = String::new();
    for diff in diffs {
        let total_add: usize = diff
            .hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| l.kind == HunkLineKind::Add)
            .count();
        let total_remove: usize = diff
            .hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| l.kind == HunkLineKind::Remove)
            .count();
        output.push_str(&format!(
            " {} | {} insertion(s), {} deletion(s)\n",
            diff.new_path, total_add, total_remove
        ));
    }
    output
}

pub(crate) async fn cmd_apply(
    patch_file: &str,
    reverse: bool,
    stat: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(patch_file)
        .map_err(|e| format!("cannot read patch file '{}': {}", patch_file, e))?;

    let mut diffs = parse_unified_diff(&content)?;

    if reverse {
        for diff in &mut diffs {
            std::mem::swap(&mut diff.old_path, &mut diff.new_path);
        }
    }

    if stat {
        let summary = stat_for_diffs(&diffs);
        if summary.is_empty() {
            println!("No changes found in patch.");
        } else {
            print!("{summary}");
        }
        println!("{} file(s) affected", diffs.len());
        return Ok(());
    }

    let mut files_applied = 0usize;

    for diff in &diffs {
        let target_path = if reverse {
            &diff.old_path
        } else {
            &diff.new_path
        };
        if target_path.is_empty() || target_path == "/dev/null" {
            continue;
        }

        let file_content = if std::path::Path::new(target_path).exists() {
            std::fs::read_to_string(target_path)?
        } else {
            String::new()
        };

        let mut file_lines: Vec<String> = file_content.lines().map(|l| l.to_string()).collect();

        for hunk in &diff.hunks {
            apply_hunk(&mut file_lines, hunk, reverse)?;
        }

        std::fs::write(target_path, file_lines.join("\n") + "\n")?;
        files_applied += 1;
    }

    println!("Applied patch to {files_applied} file(s)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_apply_stat() {
        let dir = tempfile::tempdir().unwrap();
        let patch_path = dir.path().join("test.patch");

        let patch_content = "\
--- a/hello.txt
+++ b/hello.txt
@@ -1,3 +1,4 @@
 line one
 line two
+inserted line
 line three
";
        std::fs::write(&patch_path, patch_content).unwrap();

        let result = cmd_apply(patch_path.to_str().unwrap(), false, true).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_apply_patch() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "line one\nline two\nline three\n").unwrap();

        let patch_path = dir.path().join("test.patch");
        let patch_content = "\
--- a/hello.txt
+++ b/hello.txt
@@ -1,3 +1,4 @@
 line one
 line two
+inserted line
 line three
";
        std::fs::write(&patch_path, patch_content).unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = cmd_apply("test.patch", false, false).await;
        std::env::set_current_dir(&prev).unwrap();
        assert!(result.is_ok());

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("inserted line"));
    }

    #[test]
    fn test_parse_unified_diff() {
        let diff_text = "\
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@
 line one
+added line
 line two
 line three
";
        let diffs = parse_unified_diff(diff_text).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].old_path, "file.txt");
        assert_eq!(diffs[0].new_path, "file.txt");
        assert_eq!(diffs[0].hunks.len(), 1);
        let hunk = &diffs[0].hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_count, 3);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_count, 4);
        assert_eq!(hunk.lines.len(), 4);
    }
}
