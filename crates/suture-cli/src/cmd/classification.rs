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
        let re = match regex::Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let count = re.find_iter(text).count();
        if count > 0 {
            found.push((name.to_string(), count));
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
                    return Some((name.to_string(), count, count));
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
                out.push_str(&format!(
                    "  OLD: not found\n  NEW: \"{}\" (found in {} location{})\n",
                    marking,
                    r.new_count,
                    if r.new_count == 1 { "" } else { "s" }
                ));
                out.push_str("  ! ADDED: Classification marking added\n\n");
            }
            ClassificationChange::Removed(marking) => {
                out.push_str(&format!(
                    "  OLD: \"{}\" (found in {} location{})\n  NEW: not found\n",
                    marking,
                    r.old_count,
                    if r.old_count == 1 { "" } else { "s" }
                ));
                out.push_str("  ! REMOVED: Classification marking removed\n\n");
            }
            ClassificationChange::Upgraded { from, to } => {
                out.push_str(&format!(
                    "  OLD: \"{}\" (found in {} location{})\n  NEW: \"{}\" (found in {} location{})\n",
                    from,
                    r.old_count,
                    if r.old_count == 1 { "" } else { "s" },
                    to,
                    r.new_count,
                    if r.new_count == 1 { "" } else { "s" }
                ));
                out.push_str(&format!("  ! UPGRADE: {} -> {}\n\n", from, to));
            }
            ClassificationChange::Downgraded { from, to } => {
                out.push_str(&format!(
                    "  OLD: \"{}\" (found in {} location{})\n  NEW: \"{}\" (found in {} location{})\n",
                    from,
                    r.old_count,
                    if r.old_count == 1 { "" } else { "s" },
                    to,
                    r.new_count,
                    if r.new_count == 1 { "" } else { "s" }
                ));
                out.push_str(&format!("  ! DOWNGRADE: {} -> {}\n\n", from, to));
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
    if let Some(h) = hash && let Ok(blob) = repo.cas().get_blob(h) {
        return blob;
    }
    std::fs::read(repo.root().join(path)).unwrap_or_default()
}
