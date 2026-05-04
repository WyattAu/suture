use crate::ClassificationAction;

use std::fmt::Write;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassificationChange {
    Added(String),
    Removed(String),
    Upgraded { from: String, to: String },
    Downgraded { from: String, to: String },
}

pub struct ClassificationResult {
    pub path: String,
    pub change: ClassificationChange,
    pub old_count: usize,
    pub new_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ClassificationEvent {
    pub seq: usize,
    pub timestamp: String,
    pub commit: String,
    pub author: String,
    pub file: String,
    pub event_type: String,
    pub from: String,
    pub to: String,
}

fn classification_level(marking: &str) -> u8 {
    match marking.to_uppercase().as_str() {
        "UNCLASSIFIED" | "OFFICIAL" => 1,
        "CUI" | "RESTRICTED" | "PROTECTED" | "OFFICIAL-SENSITIVE" | "FOR OFFICIAL USE ONLY" => 2,
        "CONFIDENTIAL" | "COMMERCIAL IN CONFIDENCE" | "PRIVILEGED AND CONFIDENTIAL" => 3,
        "SECRET" => 4,
        "TOP SECRET" => 5,
        "TOP SECRET//SCI" => 6,
        _ => 0,
    }
}

fn classification_patterns() -> Vec<(&'static str, &'static str)> {
    vec![
        ("TOP SECRET//SCI", r"(?i)TOP\s+SECRET\s*//\s*SCI"),
        ("TOP SECRET", r"(?i)\bTOP\s+SECRET\b"),
        ("SECRET", r"(?i)\bSECRET\b"),
        ("CONFIDENTIAL", r"(?i)\bCONFIDENTIAL\b"),
        ("OFFICIAL-SENSITIVE", r"(?i)\bOFFICIAL[\s-]SENSITIVE\b"),
        ("OFFICIAL", r"(?i)\bOFFICIAL\b"),
        ("RESTRICTED", r"(?i)\bRESTRICTED\b"),
        ("PROTECTED", r"(?i)\bPROTECTED\b"),
        ("CUI", r"(?i)\bCUI\b"),
        ("UNCLASSIFIED", r"(?i)\bUNCLASSIFIED\b"),
        (
            "FOR OFFICIAL USE ONLY",
            r"(?i)\bFOR\s+OFFICIAL\s+USE\s+ONLY\b",
        ),
        (
            "COMMERCIAL IN CONFIDENCE",
            r"(?i)\bCOMMERCIAL\s+IN\s+CONFIDENCE\b",
        ),
        (
            "PRIVILEGED AND CONFIDENTIAL",
            r"(?i)\bPRIVILEGED\s+AND\s+CONFIDENTIAL\b",
        ),
    ]
}

fn find_classifications(text: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    for (name, pattern) in classification_patterns() {
        let Ok(re) = regex::Regex::new(pattern) else { continue };
        let count = re.find_iter(text).count();
        if count > 0 {
            found.push((name.to_owned(), count));
        }
    }
    found
}

fn highest_classification(text: &str) -> Option<(String, usize, usize)> {
    let matches = find_classifications(text);
    if matches.is_empty() {
        return None;
    }
    let total_count: usize = matches.iter().map(|(_, c)| *c).sum();
    let best = matches
        .into_iter()
        .max_by(|a, b| classification_level(&a.0).cmp(&classification_level(&b.0)))
        .unwrap();
    Some((best.0, best.1, total_count))
}

fn detect_docx_classifications(data: &[u8]) -> Option<(String, usize, usize)> {
    let mut text = String::new();
    if let Ok(s) = std::str::from_utf8(data) {
        text.push_str(s);
    } else {
        let raw = String::from_utf8_lossy(data);
        for (name, pattern) in classification_patterns() {
            if let Ok(re) = regex::Regex::new(pattern) {
                let count = re.find_iter(&raw).count();
                if count > 0 {
                    return Some((name.to_owned(), count, count));
                }
            }
        }
        return None;
    }
    highest_classification(&text)
}

fn detect_classification(content: &[u8], path: &str) -> Option<(String, usize, usize)> {
    let lower = path.to_lowercase();
    if lower.ends_with(".docx") || lower.ends_with(".xlsx") || lower.ends_with(".pptx") {
        return detect_docx_classifications(content);
    }
    if lower.ends_with(".pdf")
        || lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".bmp")
        || lower.ends_with(".webp")
        || lower.ends_with(".tiff")
        || lower.ends_with(".ico")
    {
        return None;
    }
    let text = String::from_utf8_lossy(content);
    highest_classification(&text)
}

pub fn analyze_classification_changes(
    entries: &[suture_core::engine::diff::DiffEntry],
    repo: &suture_core::repository::Repository,
) -> Vec<ClassificationResult> {
    let mut results = Vec::new();

    for entry in entries {
        let old_content = get_content(repo, entry.old_hash.as_ref(), &entry.path);
        let new_content = get_content(repo, entry.new_hash.as_ref(), &entry.path);

        let old_cls = detect_classification(&old_content, &entry.path);
        let new_cls = detect_classification(&new_content, &entry.path);

        let (old_marking, old_count) = match &old_cls {
            Some((m, _, total)) => (Some(m.clone()), *total),
            None => (None, 0),
        };
        let (new_marking, new_count) = match &new_cls {
            Some((m, _, total)) => (Some(m.clone()), *total),
            None => (None, 0),
        };

        let change = match (&old_marking, &new_marking) {
            (None, None) => continue,
            (None, Some(new)) => ClassificationChange::Added(new.clone()),
            (Some(old), None) => ClassificationChange::Removed(old.clone()),
            (Some(old), Some(new)) if old == new => continue,
            (Some(old), Some(new)) => {
                let old_level = classification_level(old);
                let new_level = classification_level(new);
                if new_level > old_level {
                    ClassificationChange::Upgraded {
                        from: old.clone(),
                        to: new.clone(),
                    }
                } else {
                    ClassificationChange::Downgraded {
                        from: old.clone(),
                        to: new.clone(),
                    }
                }
            }
        };

        results.push(ClassificationResult {
            path: entry.path.clone(),
            change,
            old_count,
            new_count,
        });
    }

    results
}

pub fn format_classification_report(results: &[ClassificationResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let mut out = String::from("=== Classification Analysis ===\n\n");

    for r in results {
        out.push_str(&r.path);
        out.push_str(":\n");
        match &r.change {
            ClassificationChange::Added(marking) => {
                let _ = write!(out, 
                    "  OLD: not found\n  NEW: \"{}\" (found in {} location{})\n",
                    marking,
                    r.new_count,
                    if r.new_count == 1 { "" } else { "s" }
                );
                out.push_str("  ! ADDED: Classification marking added\n\n");
            }
            ClassificationChange::Removed(marking) => {
                let _ = write!(out, 
                    "  OLD: \"{}\" (found in {} location{})\n  NEW: not found\n",
                    marking,
                    r.old_count,
                    if r.old_count == 1 { "" } else { "s" }
                );
                out.push_str("  ! REMOVED: Classification marking removed\n\n");
            }
            ClassificationChange::Upgraded { from, to } => {
                let _ = write!(out, 
                    "  OLD: \"{}\" (found in {} location{})\n  NEW: \"{}\" (found in {} location{})\n",
                    from,
                    r.old_count,
                    if r.old_count == 1 { "" } else { "s" },
                    to,
                    r.new_count,
                    if r.new_count == 1 { "" } else { "s" }
                );
                let _ = write!(out, "  ! UPGRADE: {from} -> {to}\n\n");
            }
            ClassificationChange::Downgraded { from, to } => {
                let _ = write!(out, 
                    "  OLD: \"{}\" (found in {} location{})\n  NEW: \"{}\" (found in {} location{})\n",
                    from,
                    r.old_count,
                    if r.old_count == 1 { "" } else { "s" },
                    to,
                    r.new_count,
                    if r.new_count == 1 { "" } else { "s" }
                );
                let _ = write!(out, "  ! DOWNGRADE: {from} -> {to}\n\n");
            }
        }
    }

    out
}

fn get_content(
    repo: &suture_core::repository::Repository,
    hash: Option<&suture_common::Hash>,
    path: &str,
) -> Vec<u8> {
    if let Some(h) = hash
        && let Ok(blob) = repo.cas().get_blob(h)
    {
        return blob;
    }
    let full_path = crate::util::safe_path(repo.root(), std::path::Path::new(path))
        .unwrap_or_default();
    std::fs::read(full_path).unwrap_or_default()
}

fn change_to_event_type(change: &ClassificationChange) -> (&'static str, String, String) {
    match change {
        ClassificationChange::Added(m) => ("ADDED", String::new(), m.clone()),
        ClassificationChange::Removed(m) => ("REMOVED", m.clone(), String::new()),
        ClassificationChange::Upgraded { from, to } => ("UPGRADED", from.clone(), to.clone()),
        ClassificationChange::Downgraded { from, to } => ("DOWNGRADED", from.clone(), to.clone()),
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

pub async fn cmd_classification(
    action: &ClassificationAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ClassificationAction::Scan {
            since,
            format,
            filter,
        } => cmd_scan(since.as_deref(), format, filter.as_deref()).await,
        ClassificationAction::Report { output } => cmd_report(output.as_deref()).await,
    }
}

async fn cmd_scan(
    since: Option<&str>,
    format: &str,
    filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let branches = repo.list_branches();
    let mut seen = std::collections::HashSet::new();
    let mut all_patches = Vec::new();

    for (_, tip_id) in &branches {
        let chain = repo.dag().patch_chain(tip_id);
        for pid in &chain {
            if seen.insert(*pid)
                && let Some(patch) = repo.dag().get_patch(pid)
            {
                all_patches.push(patch.clone());
            }
        }
    }

    if let Some(since_ref) = since {
        let since_id = repo.resolve_ref(since_ref)?;
        all_patches.retain(|p| repo.dag().patch_chain(&since_id).contains(&p.id));
    }

    all_patches.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));

    let mut events: Vec<ClassificationEvent> = Vec::new();
    let mut commits_with_events = std::collections::HashSet::new();
    let mut files_with_events = std::collections::HashSet::new();
    let mut seq = 0usize;

    for patch in &all_patches {
        let parent_hex = patch
            .parent_ids
            .first()
            .map(suture_core::Hash::to_hex)
            .unwrap_or_default();
        let commit_hex = patch.id.to_hex();
        let from_ref = if parent_hex.is_empty() {
            None
        } else {
            Some(parent_hex.as_str())
        };

        let entries = repo
            .diff(from_ref, Some(commit_hex.as_str()))
            .unwrap_or_default();
        let results = analyze_classification_changes(&entries, &repo);

        for r in &results {
            let (event_type, from, to) = change_to_event_type(&r.change);
            if let Some(f) = filter
                && !event_type.eq_ignore_ascii_case(f)
            {
                continue;
            }
            seq += 1;
            let dt = chrono::DateTime::from_timestamp(patch.timestamp as i64, 0)
                .unwrap_or_default()
                .to_rfc3339();
            events.push(ClassificationEvent {
                seq,
                timestamp: dt,
                commit: commit_hex.clone(),
                author: patch.author.clone(),
                file: r.path.clone(),
                event_type: event_type.to_owned(),
                from,
                to,
            });
            commits_with_events.insert(commit_hex.clone());
            files_with_events.insert(r.path.clone());
        }
    }

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&events)?);
        }
        "csv" => {
            println!("seq,timestamp,commit,author,file,event_type,from,to");
            for e in &events {
                println!(
                    "{},{},{},{},{},{},{},{}",
                    e.seq,
                    csv_escape(&e.timestamp),
                    csv_escape(&e.commit),
                    csv_escape(&e.author),
                    csv_escape(&e.file),
                    csv_escape(&e.event_type),
                    csv_escape(&e.from),
                    csv_escape(&e.to),
                );
            }
        }
        _ => {
            if events.is_empty() {
                println!("No classification events found.");
            } else {
                println!(
                    "{:<5} {:<28} {:<20} {:<10} {:<12} {:<12} {:<12}",
                    "SEQ", "TIMESTAMP", "FILE", "EVENT", "FROM", "TO", "AUTHOR"
                );
                for e in &events {
                    let short_author = if e.author.len() > 10 {
                        format!("{}...", &e.author[..10])
                    } else {
                        e.author.clone()
                    };
                    println!(
                        "{:<5} {:<28} {:<20} {:<10} {:<12} {:<12} {:<12}",
                        e.seq,
                        e.timestamp,
                        if e.file.len() > 18 {
                            format!("{}...", &e.file[..18])
                        } else {
                            e.file.clone()
                        },
                        e.event_type,
                        e.from,
                        e.to,
                        short_author,
                    );
                }
            }
        }
    }

    println!(
        "\nFound {} classification events across {} commits, {} files",
        events.len(),
        commits_with_events.len(),
        files_with_events.len()
    );

    Ok(())
}

async fn cmd_report(output: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let branches = repo.list_branches();
    let mut seen = std::collections::HashSet::new();
    let mut all_patches = Vec::new();

    for (_, tip_id) in &branches {
        let chain = repo.dag().patch_chain(tip_id);
        for pid in &chain {
            if seen.insert(*pid)
                && let Some(patch) = repo.dag().get_patch(pid)
            {
                all_patches.push(patch.clone());
            }
        }
    }

    all_patches.sort_by_key(|a| a.timestamp);
    let total_commits = all_patches.len();

    let mut events: Vec<ClassificationEvent> = Vec::new();
    let mut current_state: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut file_change_count: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut chain_of_custody: std::collections::HashMap<String, Vec<(String, String, String)>> =
        std::collections::HashMap::new();
    let mut seq = 0usize;

    for patch in &all_patches {
        let parent_hex = patch
            .parent_ids
            .first()
            .map(suture_core::Hash::to_hex)
            .unwrap_or_default();
        let commit_hex = patch.id.to_hex();
        let from_ref = if parent_hex.is_empty() {
            None
        } else {
            Some(parent_hex.as_str())
        };

        let entries = repo
            .diff(from_ref, Some(commit_hex.as_str()))
            .unwrap_or_default();
        let results = analyze_classification_changes(&entries, &repo);

        for r in &results {
            let (event_type, from, to) = change_to_event_type(&r.change);
            seq += 1;
            let dt = chrono::DateTime::from_timestamp(patch.timestamp as i64, 0)
                .unwrap_or_default()
                .to_rfc3339();
            let short_hash = commit_hex.chars().take(12).collect::<String>();

            events.push(ClassificationEvent {
                seq,
                timestamp: dt,
                commit: commit_hex.clone(),
                author: patch.author.clone(),
                file: r.path.clone(),
                event_type: event_type.to_owned(),
                from: from.clone(),
                to: to.clone(),
            });

            *file_change_count.entry(r.path.clone()).or_insert(0) += 1;

            match &r.change {
                ClassificationChange::Removed(_) => {
                    current_state.remove(&r.path);
                }
                ClassificationChange::Added(m) | ClassificationChange::Upgraded { to: m, .. } | ClassificationChange::Downgraded { to: m, .. } => {
                    current_state.insert(r.path.clone(), m.clone());
                }
            }

            chain_of_custody.entry(r.path.clone()).or_default().push((
                short_hash,
                patch.author.clone(),
                event_type.to_owned(),
            ));
        }
    }

    let added_count = events.iter().filter(|e| e.event_type == "ADDED").count();
    let removed_count = events.iter().filter(|e| e.event_type == "REMOVED").count();
    let upgraded_count = events.iter().filter(|e| e.event_type == "UPGRADED").count();
    let downgraded_count = events
        .iter()
        .filter(|e| e.event_type == "DOWNGRADED")
        .count();

    let mut sorted_files: Vec<(String, usize)> = file_change_count
        .iter()
        .map(|(k, &v)| (k.clone(), v))
        .collect();
    sorted_files.sort_by_key(|b| std::cmp::Reverse(b.1));

    let mut above_unclassified: Vec<(String, String)> = current_state
        .iter()
        .filter(|(_, cls)| classification_level(cls) > 1)
        .map(|(f, c)| (f.clone(), c.clone()))
        .collect();
    above_unclassified.sort();

    let repo_name = std::env::current_dir()
        .unwrap_or_default()
        .file_name().map_or_else(|| "unknown".to_owned(), |n| n.to_string_lossy().to_string());
    let scan_time = chrono::Utc::now().to_rfc3339();

    let mut report = String::new();
    report.push_str("============================================\n");
    report.push_str("  CLASSIFICATION COMPLIANCE REPORT\n");
    report.push_str("============================================\n\n");
    let _ = writeln!(report, "Repository:   {repo_name}");
    let _ = writeln!(report, "Scan Time:    {scan_time}");
    let _ = writeln!(report, "Total Commits Scanned: {total_commits}");
    let _ = write!(report, 
        "Total Classification Events: {}\n\n",
        events.len()
    );

    report.push_str("--- Event Breakdown ---\n");
    let _ = writeln!(report, "  Added:     {added_count}");
    let _ = writeln!(report, "  Removed:   {removed_count}");
    let _ = writeln!(report, "  Upgraded:  {upgraded_count}");
    let _ = write!(report, "  Downgraded: {downgraded_count}\n\n");

    report.push_str("--- Files with Most Classification Changes ---\n");
    if sorted_files.is_empty() {
        report.push_str("  (none)\n\n");
    } else {
        for (file, count) in sorted_files.iter().take(10) {
            let _ = writeln!(report, "  {file} ({count} changes)");
        }
        report.push('\n');
    }

    report.push_str("--- Current Classification State ---\n");
    if current_state.is_empty() {
        report.push_str("  No files currently classified.\n\n");
    } else {
        let mut sorted: Vec<_> = current_state.iter().collect();
        sorted.sort_by_key(|(f, _)| f.as_str());
        for (file, cls) in sorted {
            let _ = writeln!(report, "  {file} [{cls}]");
        }
        report.push('\n');
    }

    report.push_str("--- Files Currently Above UNCLASSIFIED ---\n");
    if above_unclassified.is_empty() {
        report.push_str("  (none)\n\n");
    } else {
        for (file, cls) in &above_unclassified {
            let _ = writeln!(report, "  {file} [{cls}]");
        }
        report.push('\n');
    }

    report.push_str("--- Chain of Custody Summary ---\n");
    if chain_of_custody.is_empty() {
        report.push_str("  No classification changes recorded.\n\n");
    } else {
        let mut sorted: Vec<_> = chain_of_custody.iter().collect();
        sorted.sort_by_key(|(f, _)| f.as_str());
        for (file, entries) in sorted {
            let _ = writeln!(report, "  {file}:");
            for (hash, author, evt) in entries {
                let _ = writeln!(report, "    {hash} — {author} ({evt})");
            }
        }
        report.push('\n');
    }

    if let Some(output_path) = output {
        std::fs::write(output_path, &report)?;
        println!("Report written to {output_path}");
    } else {
        print!("{report}");
    }

    Ok(())
}
