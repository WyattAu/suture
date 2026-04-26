use std::collections::HashMap;
use std::path::Path;

use crate::cas::BlobStore;
use crate::engine::diff::{DiffEntry, DiffType};
use crate::engine::tree::FileTree;

pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut freq = [0usize; 256];
    for &b in data {
        freq[b as usize] += 1;
    }

    let len = data.len() as f64;
    let mut entropy = 0.0;
    for &count in &freq {
        if count == 0 {
            continue;
        }
        let p = count as f64 / len;
        entropy -= p * p.log2();
    }

    entropy
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyCategory {
    Uniform,
    Low,
    Text,
    Mixed,
    High,
    Maximum,
}

impl EntropyCategory {
    pub fn from_entropy(entropy: f64) -> Self {
        match entropy {
            e if e < 1.0 => EntropyCategory::Uniform,
            e if e < 3.0 => EntropyCategory::Low,
            e if e < 6.0 => EntropyCategory::Text,
            e if e < 7.0 => EntropyCategory::Mixed,
            e if e < 7.8 => EntropyCategory::High,
            _ => EntropyCategory::Maximum,
        }
    }
}

impl std::fmt::Display for EntropyCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntropyCategory::Uniform => write!(f, "UNIFORM"),
            EntropyCategory::Low => write!(f, "LOW"),
            EntropyCategory::Text => write!(f, "TEXT"),
            EntropyCategory::Mixed => write!(f, "MIXED"),
            EntropyCategory::High => write!(f, "HIGH"),
            EntropyCategory::Maximum => write!(f, "MAXIMUM"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskIndicator {
    HighEntropyInSourceFile,
    BinaryContentInTextFile,
    HighNullByteRatio,
    SuddenEntropyIncrease,
    BuildScriptModified,
    TestInfrastructureModified,
    ExecutableBitSetOnNewFile,
    LargeBinaryBlob,
    LockfileModifiedWithoutSource,
    NewDependencyAdded,
    Base64EncodedContent,
    EmbeddedScriptInNonScriptFile,
    CompressedFileModified,
}

impl std::fmt::Display for RiskIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskIndicator::HighEntropyInSourceFile => write!(f, "HighEntropyInSourceFile"),
            RiskIndicator::BinaryContentInTextFile => write!(f, "BinaryContentInTextFile"),
            RiskIndicator::HighNullByteRatio => write!(f, "HighNullByteRatio"),
            RiskIndicator::SuddenEntropyIncrease => write!(f, "SuddenEntropyIncrease"),
            RiskIndicator::BuildScriptModified => write!(f, "BuildScriptModified"),
            RiskIndicator::TestInfrastructureModified => write!(f, "TestInfrastructureModified"),
            RiskIndicator::ExecutableBitSetOnNewFile => write!(f, "ExecutableBitSetOnNewFile"),
            RiskIndicator::LargeBinaryBlob => write!(f, "LargeBinaryBlob"),
            RiskIndicator::LockfileModifiedWithoutSource => {
                write!(f, "LockfileModifiedWithoutSource")
            }
            RiskIndicator::NewDependencyAdded => write!(f, "NewDependencyAdded"),
            RiskIndicator::Base64EncodedContent => write!(f, "Base64EncodedContent"),
            RiskIndicator::EmbeddedScriptInNonScriptFile => {
                write!(f, "EmbeddedScriptInNonScriptFile")
            }
            RiskIndicator::CompressedFileModified => write!(f, "CompressedFileModified"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskScore {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskScore {
    fn from_indicator_count(count: usize) -> Self {
        match count {
            0 => RiskScore::None,
            1 => RiskScore::Low,
            2..=3 => RiskScore::Medium,
            4..=5 => RiskScore::High,
            _ => RiskScore::Critical,
        }
    }

    #[allow(dead_code)]
    fn max(a: Self, b: Self) -> Self {
        if a > b { a } else { b }
    }
}

impl std::fmt::Display for RiskScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskScore::None => write!(f, "NONE"),
            RiskScore::Low => write!(f, "LOW"),
            RiskScore::Medium => write!(f, "MEDIUM"),
            RiskScore::High => write!(f, "HIGH"),
            RiskScore::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileIntegrityReport {
    pub path: String,
    pub size_bytes: usize,
    pub shannon_entropy: f64,
    pub entropy_category: EntropyCategory,
    pub null_byte_ratio: f64,
    pub printable_ascii_ratio: f64,
    pub high_byte_ratio: f64,
    pub is_binary: bool,
    pub risk_indicators: Vec<RiskIndicator>,
    pub risk_score: RiskScore,
}

fn file_extension(path: &str) -> Option<&str> {
    let filename = path.rsplit('/').next().unwrap_or(path);
    let dot_pos = filename.rfind('.')?;
    Some(&filename[dot_pos + 1..])
}

fn is_source_extension(ext: &str) -> bool {
    matches!(
        ext,
        "c" | "h" | "rs" | "py" | "js" | "ts" | "go" | "java" | "cpp" | "hpp" | "cc" | "cxx"
    )
}

fn is_text_extension(ext: &str) -> bool {
    is_source_extension(ext)
        || matches!(
            ext,
            "txt"
                | "md"
                | "toml"
                | "yaml"
                | "yml"
                | "json"
                | "xml"
                | "html"
                | "css"
                | "sh"
                | "bash"
                | "zsh"
                | "fish"
                | "rb"
                | "pl"
                | "lua"
                | "r"
                | "swift"
                | "kt"
                | "scala"
                | "ex"
                | "exs"
                | "erl"
                | "hrl"
                | "clj"
                | "cljs"
                | "hs"
                | "ml"
                | "mli"
                | "jsx"
                | "tsx"
                | "vue"
                | "svelte"
                | "php"
                | "cs"
                | "dart"
                | "zig"
                | "nim"
                | "v"
                | "adb"
                | "ada"
        )
}

fn is_compressed_extension(ext: &str) -> bool {
    matches!(
        ext,
        "gz" | "xz" | "bz2" | "zst" | "zip" | "tar" | "tgz" | "7z" | "rar"
    )
}

fn is_build_script(path: &str) -> bool {
    let filename = path.rsplit('/').next().unwrap_or(path);
    matches!(
        filename,
        "configure"
            | "Makefile"
            | "CMakeLists.txt"
            | "build.rs"
            | "Cargo.toml"
            | "package.json"
            | "setup.py"
            | "setup.cfg"
            | "pyproject.toml"
            | "Makefile.am"
            | "Makefile.in"
            | "meson.build"
            | "build.gradle"
            | "pom.xml"
            | "build.sbt"
            | "Gemfile"
            | "Rakefile"
            | "Justfile"
            | "justfile"
            | "Taskfile.yml"
            | "Cargo.lock"
            | "package-lock.json"
            | "yarn.lock"
    )
}

fn is_test_file(path: &str) -> bool {
    let filename = path.rsplit('/').next().unwrap_or(path);
    let lower = filename.to_lowercase();

    if lower.contains("test")
        || lower.contains("spec")
        || lower.contains("fixture")
        || lower.contains("__tests__")
    {
        return true;
    }

    if let Some(ext) = file_extension(path)
        && matches!(ext, "test" | "spec" | "snap")
    {
        return true;
    }

    if filename.ends_with("_test.rs")
        || filename.ends_with("_tests.rs")
        || filename.ends_with("_spec.rs")
        || filename.ends_with(".test.js")
        || filename.ends_with(".test.ts")
        || filename.ends_with(".spec.js")
        || filename.ends_with(".spec.ts")
        || filename.ends_with("_test.go")
        || filename.ends_with("_test.py")
    {
        return true;
    }

    let parts: Vec<&str> = path.split('/').collect();
    if parts
        .iter()
        .any(|p| *p == "tests" || *p == "spec" || *p == "fixtures")
    {
        return true;
    }

    false
}

fn is_lockfile(path: &str) -> bool {
    let filename = path.rsplit('/').next().unwrap_or(path);
    matches!(
        filename,
        "Cargo.lock"
            | "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "Gemfile.lock"
            | "poetry.lock"
            | "pipfile.lock"
            | "composer.lock"
            | "go.sum"
            | "flake.lock"
    )
}

fn looks_like_base64(content: &[u8]) -> bool {
    if content.is_empty() {
        return false;
    }

    let text = match std::str::from_utf8(content) {
        Ok(t) => t,
        Err(_) => return false,
    };

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return false;
    }

    let mut matching_lines = 0usize;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let is_b64 = trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
            && trimmed.len() >= 16
            && trimmed.len() % 4 <= 1;

        if is_b64 {
            matching_lines += 1;
        }
    }

    let non_empty_lines = lines.iter().filter(|l| !l.trim().is_empty()).count();
    if non_empty_lines == 0 {
        return false;
    }

    (matching_lines as f64 / non_empty_lines as f64) > 0.8
}

fn looks_like_embedded_script(path: &str, content: &[u8]) -> bool {
    if let Some(ext) = file_extension(path)
        && matches!(
            ext,
            "rs" | "c"
                | "h"
                | "cpp"
                | "hpp"
                | "hxx"
                | "toml"
                | "yaml"
                | "yml"
                | "json"
                | "xml"
                | "md"
                | "txt"
                | "html"
                | "css"
                | "cmake"
        )
    {
        let text = match std::str::from_utf8(content) {
            Ok(t) => t,
            Err(_) => return false,
        };

        let lower = text.to_lowercase();
        let script_patterns = [
            "#!/bin/bash",
            "#!/bin/sh",
            "#!/usr/bin/env python",
            "#!/usr/bin/env bash",
            "eval(",
            "exec(",
            "system(",
            "popen(",
            "__import__(",
            "subprocess.call",
            "os.system",
            "os.popen",
            "child_process",
        ];

        for pattern in &script_patterns {
            if lower.contains(pattern) {
                return true;
            }
        }
    }
    false
}

pub fn analyze_file(path: &str, content: &[u8]) -> FileIntegrityReport {
    let size_bytes = content.len();
    let entropy = shannon_entropy(content);
    let entropy_category = EntropyCategory::from_entropy(entropy);

    let mut null_count = 0usize;
    let mut printable_count = 0usize;
    let mut high_count = 0usize;

    for &b in content {
        if b == 0 {
            null_count += 1;
        }
        if (0x20..=0x7e).contains(&b) || b == b'\n' || b == b'\r' || b == b'\t' {
            printable_count += 1;
        }
        if b >= 0x80 {
            high_count += 1;
        }
    }

    let total = if size_bytes > 0 { size_bytes } else { 1 };
    let null_byte_ratio = null_count as f64 / total as f64;
    let printable_ascii_ratio = printable_count as f64 / total as f64;
    let high_byte_ratio = high_count as f64 / total as f64;
    let is_binary = printable_ascii_ratio < 0.7 && size_bytes > 0;

    let mut risk_indicators = Vec::new();

    if let Some(ext) = file_extension(path) {
        if is_source_extension(ext) && entropy > 6.5 {
            risk_indicators.push(RiskIndicator::HighEntropyInSourceFile);
        }

        if is_text_extension(ext) && printable_ascii_ratio < 0.7 && size_bytes > 0 {
            risk_indicators.push(RiskIndicator::BinaryContentInTextFile);
        }

        if is_compressed_extension(ext) {
            risk_indicators.push(RiskIndicator::CompressedFileModified);
        }
    }

    if null_byte_ratio > 0.05 && size_bytes > 0 {
        risk_indicators.push(RiskIndicator::HighNullByteRatio);
    }

    if is_build_script(path) {
        risk_indicators.push(RiskIndicator::BuildScriptModified);
    }

    if is_test_file(path) {
        risk_indicators.push(RiskIndicator::TestInfrastructureModified);
    }

    if is_lockfile(path) {
        risk_indicators.push(RiskIndicator::LockfileModifiedWithoutSource);
    }

    if size_bytes > 1_048_576 && printable_ascii_ratio < 0.3 {
        risk_indicators.push(RiskIndicator::LargeBinaryBlob);
    }

    if looks_like_base64(content) {
        risk_indicators.push(RiskIndicator::Base64EncodedContent);
    }

    if looks_like_embedded_script(path, content) {
        risk_indicators.push(RiskIndicator::EmbeddedScriptInNonScriptFile);
    }

    let risk_score = RiskScore::from_indicator_count(risk_indicators.len());

    FileIntegrityReport {
        path: path.to_string(),
        size_bytes,
        shannon_entropy: entropy,
        entropy_category,
        null_byte_ratio,
        printable_ascii_ratio,
        high_byte_ratio,
        is_binary,
        risk_indicators,
        risk_score,
    }
}

#[derive(Debug, Clone)]
pub struct DiffIntegrityReport {
    pub files: Vec<FileIntegrityReport>,
    pub old_files: HashMap<String, FileIntegrityReport>,
    pub summary: IntegritySummary,
}

#[derive(Debug, Clone)]
pub struct IntegritySummary {
    pub total_files_changed: usize,
    pub high_risk_count: usize,
    pub critical_risk_count: usize,
    pub new_binary_files: usize,
    pub build_system_changes: usize,
    pub overall_risk: RiskScore,
    pub warnings: Vec<String>,
}

pub fn analyze_diff(
    diff_entries: &[DiffEntry],
    _old_tree: &FileTree,
    _new_tree: &FileTree,
    cas: &BlobStore,
    _working_dir: &Path,
) -> Result<DiffIntegrityReport, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    let mut old_files = HashMap::new();
    let mut has_build_changes = false;
    let mut has_test_changes = false;
    let mut has_lockfile_changes = false;
    let mut has_manifest_changes = false;
    let mut test_and_source_paths: Vec<String> = Vec::new();
    let mut lockfile_paths: Vec<String> = Vec::new();

    for entry in diff_entries {
        match &entry.diff_type {
            DiffType::Added => {
                if let Some(hash) = &entry.new_hash {
                    match cas.get_blob(hash) {
                        Ok(content) => {
                            let mut report = analyze_file(&entry.path, &content);

                            if has_test_changes && !is_test_file(&entry.path) {
                                report
                                    .risk_indicators
                                    .push(RiskIndicator::SuddenEntropyIncrease);
                            }

                            files.push(report);
                        }
                        Err(_) => {
                            let report = analyze_file(&entry.path, &[]);
                            files.push(report);
                        }
                    }
                }
            }
            DiffType::Modified => {
                if let Some(hash) = &entry.new_hash {
                    match cas.get_blob(hash) {
                        Ok(content) => {
                            let mut report = analyze_file(&entry.path, &content);

                            if let Some(old_hash) = &entry.old_hash
                                && let Ok(old_content) = cas.get_blob(old_hash)
                            {
                                let old_entropy = shannon_entropy(&old_content);
                                let new_entropy = shannon_entropy(&content);
                                if new_entropy > old_entropy + 2.0 {
                                    report
                                        .risk_indicators
                                        .push(RiskIndicator::SuddenEntropyIncrease);
                                }

                                let old_report = analyze_file(&entry.path, &old_content);
                                old_files.insert(entry.path.clone(), old_report);
                            }

                            report.risk_score =
                                RiskScore::from_indicator_count(report.risk_indicators.len());
                            files.push(report);
                        }
                        Err(_) => {
                            let report = analyze_file(&entry.path, &[]);
                            files.push(report);
                        }
                    }
                }
            }
            DiffType::Deleted => {
                if let Some(hash) = &entry.old_hash
                    && let Ok(content) = cas.get_blob(hash)
                {
                    let old_report = analyze_file(&entry.path, &content);
                    old_files.insert(entry.path.clone(), old_report);
                }
            }
            DiffType::Renamed { .. } => {
                if let Some(hash) = &entry.new_hash {
                    match cas.get_blob(hash) {
                        Ok(content) => {
                            let report = analyze_file(&entry.path, &content);
                            files.push(report);
                        }
                        Err(_) => {
                            let report = analyze_file(&entry.path, &[]);
                            files.push(report);
                        }
                    }
                }
                if let Some(hash) = &entry.old_hash
                    && let Ok(content) = cas.get_blob(hash)
                {
                    let old_report = analyze_file(&entry.path, &content);
                    old_files.insert(entry.path.clone(), old_report);
                }
            }
        }
    }

    let mut warnings = Vec::new();
    let mut high_risk_count = 0usize;
    let mut critical_risk_count = 0usize;
    let mut new_binary_files = 0usize;
    let mut build_system_changes = 0usize;

    for report in &files {
        if report.risk_score == RiskScore::High {
            high_risk_count += 1;
        } else if report.risk_score == RiskScore::Critical {
            critical_risk_count += 1;
        }

        if report.is_binary {
            new_binary_files += 1;
        }

        if report
            .risk_indicators
            .contains(&RiskIndicator::BuildScriptModified)
        {
            has_build_changes = true;
            build_system_changes += 1;
        }

        if report
            .risk_indicators
            .contains(&RiskIndicator::TestInfrastructureModified)
        {
            has_test_changes = true;
            test_and_source_paths.push(report.path.clone());
        }

        if is_lockfile(&report.path) {
            has_lockfile_changes = true;
            lockfile_paths.push(report.path.clone());
        }

        if is_manifest_file(&report.path) {
            has_manifest_changes = true;
        }
    }

    if has_build_changes && has_test_changes {
        warnings.push(
            "Build script modified alongside test infrastructure. \
             This pattern was used in the XZ Utils backdoor (CVE-2024-3094)."
                .to_string(),
        );
        let mut paths = test_and_source_paths.clone();
        for report in &files {
            if report
                .risk_indicators
                .contains(&RiskIndicator::BuildScriptModified)
            {
                paths.push(report.path.clone());
            }
        }
        paths.sort();
        paths.dedup();
        warnings.push(format!("Review: {}", paths.join(", ")));
    }

    if has_lockfile_changes && !has_manifest_changes {
        warnings.push(
            "Lockfile modified without corresponding manifest change. \
             This could indicate a supply chain injection attempt."
                .to_string(),
        );
        warnings.push(format!("Review: {}", lockfile_paths.join(", ")));
    }

    let overall_risk = if critical_risk_count > 0 {
        RiskScore::Critical
    } else if high_risk_count > 0 {
        RiskScore::High
    } else if !warnings.is_empty() {
        RiskScore::Medium
    } else {
        RiskScore::None
    };

    let summary = IntegritySummary {
        total_files_changed: diff_entries.len(),
        high_risk_count,
        critical_risk_count,
        new_binary_files,
        build_system_changes,
        overall_risk,
        warnings,
    };

    Ok(DiffIntegrityReport {
        files,
        old_files,
        summary,
    })
}

fn is_manifest_file(path: &str) -> bool {
    let filename = path.rsplit('/').next().unwrap_or(path);
    matches!(
        filename,
        "Cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "setup.py"
            | "setup.cfg"
            | "Gemfile"
            | "build.gradle"
            | "pom.xml"
            | "build.sbt"
            | "go.mod"
            | "flake.nix"
    )
}

pub fn format_file_integrity(file: &FileIntegrityReport) -> String {
    let mut lines = Vec::new();

    lines.push(format!("  {}", file.path));

    lines.push(format!(
        "    Size: {} bytes  Entropy: {:.2} [{}]",
        file.size_bytes, file.shannon_entropy, file.entropy_category
    ));

    lines.push(format!(
        "    Printable: {:.1}%  Null: {:.1}%  Binary: {}",
        file.printable_ascii_ratio * 100.0,
        file.null_byte_ratio * 100.0,
        if file.is_binary { "yes" } else { "no" }
    ));

    lines.push(format!("    Risk: {}", format_risk_score(file.risk_score)));

    if !file.risk_indicators.is_empty() {
        lines.push("    Indicators:".to_string());
        for indicator in &file.risk_indicators {
            lines.push(format!("      - {}", indicator));
        }
    }

    lines.join("\n")
}

fn format_risk_score(score: RiskScore) -> String {
    match score {
        RiskScore::None => "[OK] NONE".to_string(),
        RiskScore::Low => "[!] LOW".to_string(),
        RiskScore::Medium => "[!] MEDIUM".to_string(),
        RiskScore::High => "[!!] HIGH".to_string(),
        RiskScore::Critical => "[!!!] CRITICAL".to_string(),
    }
}

pub fn format_integrity_report(report: &DiffIntegrityReport) -> String {
    let mut lines = Vec::new();

    let border_top = String::from_utf8(vec![0xE2, 0x95, 0x94]).unwrap();
    let border_horiz = String::from_utf8(vec![0xE2, 0x95, 0x90]).unwrap();
    let border_right = String::from_utf8(vec![0xE2, 0x95, 0x97]).unwrap();
    let border_left = String::from_utf8(vec![0xE2, 0x95, 0x91]).unwrap();
    let border_mid_l = String::from_utf8(vec![0xE2, 0x95, 0xA0]).unwrap();
    let border_mid_r = String::from_utf8(vec![0xE2, 0x95, 0x9D]).unwrap();
    let border_bottom = String::from_utf8(vec![0xE2, 0x95, 0x9A]).unwrap();

    let title = " INTEGRITY ANALYSIS - Supply Chain Transparency ";
    let width = 54;
    let title_padded = format!("{title:^width$}");

    lines.push(format!(
        "{}{}{}",
        border_top,
        border_horiz.repeat(width),
        border_right
    ));
    lines.push(format!("{}{}{}", border_left, title_padded, border_right));
    lines.push(format!(
        "{}{}{}",
        border_mid_l,
        border_horiz.repeat(width),
        border_mid_r
    ));
    lines.push(format!(
        "{}  Files changed: {:<40}{}",
        border_left, report.summary.total_files_changed, border_right
    ));

    let risk_label = format!("Risk: {}", report.summary.overall_risk);
    lines.push(format!(
        "{}  {:<50}{}",
        border_left, risk_label, border_right
    ));
    lines.push(format!(
        "{}{}{}",
        border_bottom,
        border_horiz.repeat(width),
        border_right
    ));
    lines.push(String::new());

    for file in &report.files {
        lines.push(format_file_integrity(file));
        lines.push(String::new());
    }

    if !report.summary.warnings.is_empty() {
        for warning in &report.summary.warnings {
            lines.push(format!("[!] WARNING: {}", warning));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::diff::DiffEntry;
    use crate::engine::tree::FileTree;

    #[test]
    fn test_shannon_entropy_uniform() {
        let data = vec![0x42u8; 10000];
        let entropy = shannon_entropy(&data);
        assert!(
            entropy < 0.01,
            "Expected near-zero entropy for uniform data, got {}",
            entropy
        );
    }

    #[test]
    fn test_shannon_entropy_random() {
        let data: Vec<u8> = (0..=255u16)
            .flat_map(|i| {
                let b = i as u8;
                std::iter::repeat(b).take(40)
            })
            .collect();
        let entropy = shannon_entropy(&data);
        assert!(
            entropy > 7.5,
            "Expected high entropy for random data, got {}",
            entropy
        );
    }

    #[test]
    fn test_shannon_entropy_text() {
        let text = "The quick brown fox jumps over the lazy dog. \
                    This is a sample of English text that should have \
                    moderate entropy typical of natural language. \
                    Programming involves writing code in various languages \
                    such as Rust, Python, JavaScript, and many others. \
                    Supply chain security is important for open source.";
        let entropy = shannon_entropy(text.as_bytes());
        assert!(
            entropy >= 3.5 && entropy <= 5.5,
            "Expected entropy in range 3.5-5.5 for English text, got {}",
            entropy
        );
    }

    #[test]
    fn test_shannon_entropy_empty() {
        let entropy = shannon_entropy(&[]);
        assert_eq!(entropy, 0.0);
    }

    #[test]
    fn test_entropy_category_classification() {
        assert_eq!(EntropyCategory::from_entropy(0.5), EntropyCategory::Uniform);
        assert_eq!(EntropyCategory::from_entropy(1.5), EntropyCategory::Low);
        assert_eq!(EntropyCategory::from_entropy(4.5), EntropyCategory::Text);
        assert_eq!(EntropyCategory::from_entropy(6.5), EntropyCategory::Mixed);
        assert_eq!(EntropyCategory::from_entropy(7.5), EntropyCategory::High);
        assert_eq!(EntropyCategory::from_entropy(7.9), EntropyCategory::Maximum);
        assert_eq!(
            EntropyCategory::from_entropy(0.99),
            EntropyCategory::Uniform
        );
        assert_eq!(EntropyCategory::from_entropy(1.0), EntropyCategory::Low);
        assert_eq!(EntropyCategory::from_entropy(2.99), EntropyCategory::Low);
        assert_eq!(EntropyCategory::from_entropy(3.0), EntropyCategory::Text);
        assert_eq!(EntropyCategory::from_entropy(5.99), EntropyCategory::Text);
        assert_eq!(EntropyCategory::from_entropy(6.0), EntropyCategory::Mixed);
        assert_eq!(EntropyCategory::from_entropy(6.99), EntropyCategory::Mixed);
        assert_eq!(EntropyCategory::from_entropy(7.0), EntropyCategory::High);
        assert_eq!(EntropyCategory::from_entropy(7.79), EntropyCategory::High);
        assert_eq!(EntropyCategory::from_entropy(7.8), EntropyCategory::Maximum);
    }

    #[test]
    fn test_analyze_source_file_high_entropy() {
        let high_entropy_data: Vec<u8> = (0..=255u16)
            .flat_map(|i| {
                let b = i as u8;
                std::iter::repeat(b).take(100)
            })
            .collect();
        let report = analyze_file("src/main.rs", &high_entropy_data);
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::HighEntropyInSourceFile),
            "Expected HighEntropyInSourceFile for high-entropy .rs file"
        );
        assert!(report.risk_score >= RiskScore::Low);
    }

    #[test]
    fn test_analyze_binary_in_text_file() {
        let mut data = b"hello world\n".to_vec();
        data.extend(std::iter::repeat(0u8).take(200));
        let report = analyze_file("readme.txt", &data);
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::BinaryContentInTextFile),
            "Expected BinaryContentInTextFile for null bytes in .txt"
        );
    }

    #[test]
    fn test_analyze_build_script() {
        let report = analyze_file("configure", b"#!/bin/sh\necho hello\n");
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::BuildScriptModified),
            "Expected BuildScriptModified for configure script"
        );
    }

    #[test]
    fn test_analyze_build_script_makefile() {
        let report = analyze_file("Makefile", b"all:\n\techo build\n");
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::BuildScriptModified),
            "Expected BuildScriptModified for Makefile"
        );
    }

    #[test]
    fn test_analyze_compressed_file() {
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let report = analyze_file("archive.tar.gz", &data);
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::CompressedFileModified),
            "Expected CompressedFileModified for .gz file"
        );
    }

    #[test]
    fn test_risk_score_calculation() {
        assert_eq!(RiskScore::from_indicator_count(0), RiskScore::None);
        assert_eq!(RiskScore::from_indicator_count(1), RiskScore::Low);
        assert_eq!(RiskScore::from_indicator_count(2), RiskScore::Medium);
        assert_eq!(RiskScore::from_indicator_count(3), RiskScore::Medium);
        assert_eq!(RiskScore::from_indicator_count(4), RiskScore::High);
        assert_eq!(RiskScore::from_indicator_count(5), RiskScore::High);
        assert_eq!(RiskScore::from_indicator_count(6), RiskScore::Critical);
        assert_eq!(RiskScore::from_indicator_count(10), RiskScore::Critical);
    }

    #[test]
    fn test_format_integrity_report() {
        let file = analyze_file("src/main.rs", b"fn main() { println!(\"hello\"); }");
        let report = DiffIntegrityReport {
            files: vec![file],
            old_files: HashMap::new(),
            summary: IntegritySummary {
                total_files_changed: 1,
                high_risk_count: 0,
                critical_risk_count: 0,
                new_binary_files: 0,
                build_system_changes: 0,
                overall_risk: RiskScore::None,
                warnings: vec![],
            },
        };

        let output = format_integrity_report(&report);
        assert!(output.contains("INTEGRITY ANALYSIS"));
        assert!(output.contains("Files changed: 1"));
        assert!(output.contains("src/main.rs"));
        assert!(output.contains("Entropy:"));
    }

    #[test]
    fn test_base64_detection() {
        let mut lines: Vec<String> = Vec::new();
        for i in 0..20 {
            let chunk: String = (0..64)
                .map(|j| {
                    let idx = (i * 64 + j) % 64;
                    let charset =
                        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
                    charset.chars().nth(idx).unwrap()
                })
                .collect();
            lines.push(chunk);
        }
        let content = lines.join("\n");
        let report = analyze_file("data.b64", content.as_bytes());
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::Base64EncodedContent),
            "Expected Base64EncodedContent for base64 data"
        );
    }

    #[test]
    fn test_null_byte_detection() {
        let mut data = "normal text".as_bytes().to_vec();
        data.extend(std::iter::repeat(0u8).take(1000));
        let report = analyze_file("data.bin", &data);
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::HighNullByteRatio),
            "Expected HighNullByteRatio for data with many null bytes"
        );
        assert!(report.null_byte_ratio > 0.05);
    }

    #[test]
    fn test_analyze_test_file() {
        let report = analyze_file("tests/integration_test.rs", b"#[test]\nfn it_works() {}\n");
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::TestInfrastructureModified),
            "Expected TestInfrastructureModified for test file"
        );
    }

    #[test]
    fn test_large_binary_blob() {
        let data: Vec<u8> = (0..2_000_000)
            .map(|i| {
                let v = (i % 256) as u8;
                if v >= 0x20 && v <= 0x7e { 0x01 } else { v }
            })
            .collect();
        let report = analyze_file("blob.bin", &data);
        assert!(
            report
                .risk_indicators
                .contains(&RiskIndicator::LargeBinaryBlob),
            "Expected LargeBinaryBlob for file > 1MB with low printable ratio"
        );
    }

    #[test]
    fn test_format_file_integrity() {
        let report = analyze_file("example.rs", b"fn main() {}\n");
        let output = format_file_integrity(&report);
        assert!(output.contains("example.rs"));
        assert!(output.contains("Entropy:"));
        assert!(output.contains("Printable:"));
    }

    #[test]
    fn test_analyze_diff_with_trees() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new_uncompressed(dir.path()).unwrap();

        let old_content = b"fn main() { println!(\"v1\"); }";
        let new_content = b"fn main() { println!(\"v2\"); }";
        let old_hash = store.put_blob(old_content).unwrap();
        let new_hash = store.put_blob(new_content).unwrap();

        let mut old_tree = FileTree::empty();
        old_tree.insert("src/main.rs".to_string(), old_hash);

        let mut new_tree = FileTree::empty();
        new_tree.insert("src/main.rs".to_string(), new_hash);

        let entries = vec![DiffEntry {
            path: "src/main.rs".to_string(),
            diff_type: DiffType::Modified,
            old_hash: Some(old_hash),
            new_hash: Some(new_hash),
        }];

        let report = analyze_diff(&entries, &old_tree, &new_tree, &store, dir.path()).unwrap();
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.summary.total_files_changed, 1);
    }
}
