// SPDX-License-Identifier: MIT OR Apache-2.0

//! # suture-merge
//!
//! Dead-simple semantic merge for structured files, including DOCX, XLSX, and PPTX binary documents.
//!
//! ## Quick Start
//!
//! ```rust
//! use suture_merge::{merge_json, MergeResult, MergeStatus};
//!
//! let base = r#"{"name": "Alice", "age": 30}"#;
//! let ours = r#"{"name": "Alice", "age": 31}"#;
//! let theirs = r#"{"name": "Alice", "city": "NYC"}"#;
//!
//! let result = merge_json(base, ours, theirs)?;
//! assert_eq!(result.status, MergeStatus::Clean);
//! assert!(result.merged.contains(r#""age": 31"#));
//! assert!(result.merged.contains(r#""city": "NYC""#));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
mod registry;

pub use error::{
    ConflictResolution, MergeConflict, MergeError, MergeOutput, MergeResult, MergeStatus,
    merge_with_conflicts, resolve_conflict,
};

use error::perform_merge;
use registry::build_registry;
use suture_driver::SemanticChange;

#[cfg(feature = "json")]
pub fn merge_json(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_json::JsonDriver, base, ours, theirs)
}

#[cfg(feature = "yaml")]
pub fn merge_yaml(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_yaml::YamlDriver, base, ours, theirs)
}

#[cfg(feature = "toml")]
pub fn merge_toml(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_toml::TomlDriver, base, ours, theirs)
}

#[cfg(feature = "csv")]
pub fn merge_csv(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_csv::CsvDriver, base, ours, theirs)
}

#[cfg(feature = "xml")]
pub fn merge_xml(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_xml::XmlDriver, base, ours, theirs)
}

#[cfg(feature = "markdown")]
pub fn merge_markdown(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_markdown::MarkdownDriver, base, ours, theirs)
}

#[cfg(feature = "svg")]
pub fn merge_svg(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_svg::SvgDriver, base, ours, theirs)
}

#[cfg(feature = "html")]
pub fn merge_html(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_html::HtmlDriver, base, ours, theirs)
}

#[cfg(feature = "ical")]
pub fn merge_ical(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_ical::IcalDriver, base, ours, theirs)
}

#[cfg(feature = "feed")]
pub fn merge_feed(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_feed::FeedDriver, base, ours, theirs)
}

#[cfg(feature = "docx")]
pub fn merge_docx(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_docx::DocxDriver, base, ours, theirs)
}

#[cfg(feature = "xlsx")]
pub fn merge_xlsx(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_xlsx::XlsxDriver, base, ours, theirs)
}

#[cfg(feature = "pptx")]
pub fn merge_pptx(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_pptx::PptxDriver, base, ours, theirs)
}

#[cfg(feature = "properties")]
pub fn merge_properties(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    perform_merge(&suture_driver_properties::PropertiesDriver, base, ours, theirs)
}

pub fn merge_auto(
    base: &str,
    ours: &str,
    theirs: &str,
    extension: Option<&str>,
) -> Result<MergeResult, MergeError> {
    let ext = extension
        .ok_or_else(|| MergeError::UnsupportedFormat("no extension provided".to_owned()))?;
    let registry = build_registry();
    let driver = registry.get(ext)?;
    perform_merge(driver, base, ours, theirs)
}

#[cfg(feature = "json")]
pub fn merge_lockfile(
    path: &str,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<MergeResult, MergeError> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "json" => merge_json(base, ours, theirs),
        #[cfg(feature = "yaml")]
        "yaml" | "yml" => merge_yaml(base, ours, theirs),
        _ => {
            let strategy = suture_driver::LockfileMergeStrategy::new();
            let result = suture_driver::MergeStrategy::merge(
                &strategy,
                base.as_bytes(),
                ours.as_bytes(),
                theirs.as_bytes(),
            )
            .map_err(|e| MergeError::ParseError(e.to_string()))?;
            Ok(MergeResult {
                merged: String::from_utf8_lossy(&result.content).into_owned(),
                status: if result.had_conflicts {
                    MergeStatus::Conflict
                } else {
                    MergeStatus::Clean
                },
            })
        }
    }
}

pub fn diff(
    base: &str,
    modified: &str,
    extension: Option<&str>,
) -> Result<Vec<SemanticChange>, MergeError> {
    let ext = extension
        .ok_or_else(|| MergeError::UnsupportedFormat("no extension provided".to_owned()))?;
    let registry = build_registry();
    let driver = registry.get(ext)?;
    Ok(driver.diff(Some(base), modified)?)
}

pub fn format_diff(
    base: &str,
    modified: &str,
    extension: Option<&str>,
) -> Result<String, MergeError> {
    let ext = extension
        .ok_or_else(|| MergeError::UnsupportedFormat("no extension provided".to_owned()))?;
    let registry = build_registry();
    let driver = registry.get(ext)?;
    Ok(driver.format_diff(Some(base), modified)?)
}

/// Callback trait for programmatic conflict resolution.
/// Implement this to define custom conflict resolution logic
/// (e.g., always take ours for .json files, always take theirs for .lock files).
#[cfg(feature = "json")]
pub trait ConflictResolver {
    /// Given a conflict, return how to resolve it.
    fn resolve(&self, conflict: &MergeConflict) -> ConflictResolution;
}

/// Merge and auto-resolve any conflicts using a resolver.
/// Returns a MergeResult with Clean status if all conflicts were resolved.
///
/// # Example
///
/// ```rust
/// use suture_merge::{merge_resolve, ConflictResolver, ConflictResolution, MergeConflict, MergeStatus};
///
/// struct TakeTheirs;
/// impl ConflictResolver for TakeTheirs {
///     fn resolve(&self, _conflict: &MergeConflict) -> ConflictResolution {
///         ConflictResolution::Theirs
///     }
/// }
///
/// let base  = r#"{"key": "original"}"#;
/// let ours  = r#"{"key": "ours"}"#;
/// let theirs = r#"{"key": "theirs"}"#;
///
/// let result = merge_resolve(Some("data.json"), base, ours, theirs, &TakeTheirs).unwrap();
/// assert_eq!(result.status, MergeStatus::Clean);
/// assert!(result.merged.contains(r#""key": "theirs""#));
/// ```
#[cfg(feature = "json")]
pub fn merge_resolve(
    extension: Option<&str>,
    base: &str,
    ours: &str,
    theirs: &str,
    resolver: &dyn ConflictResolver,
) -> Result<MergeResult, MergeError> {
    let path = extension.unwrap_or("file.json");
    let output = merge_with_conflicts(base, ours, theirs, path);

    match output.status {
        MergeStatus::Clean => {
            let merged = output.content.unwrap_or_else(|| ours.to_string());
            Ok(MergeResult {
                merged,
                status: MergeStatus::Clean,
            })
        }
        MergeStatus::Conflict => {
            let resolved_parts: Vec<String> = output
                .conflicts
                .iter()
                .map(|c| {
                    let resolution = resolver.resolve(c);
                    resolve_conflict(c, &resolution)
                })
                .collect();
            let merged = if resolved_parts.is_empty() {
                ours.to_string()
            } else {
                resolved_parts.join("\n")
            };
            Ok(MergeResult {
                merged,
                status: MergeStatus::Clean,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_json_clean() {
        let base = r#"{"name": "Alice", "age": 30}"#;
        let ours = r#"{"name": "Alice", "age": 31}"#;
        let theirs = r#"{"name": "Alice", "city": "NYC"}"#;

        let result = merge_json(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
        assert!(result.merged.contains(r#""age": 31"#));
        assert!(result.merged.contains(r#""city": "NYC""#));
    }

    #[test]
    fn merge_json_conflict() {
        let base = r#"{"key": "original"}"#;
        let ours = r#"{"key": "ours"}"#;
        let theirs = r#"{"key": "theirs"}"#;

        let result = merge_json(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Conflict);
    }

    #[test]
    fn merge_json_new_file() {
        let base = "{}";
        let ours = r#"{"a": 1}"#;
        let theirs = r#"{"b": 2}"#;

        let result = merge_json(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
        assert!(result.merged.contains(r#""a": 1"#));
        assert!(result.merged.contains(r#""b": 2"#));
    }

    #[test]
    fn merge_yaml_clean() {
        let base = "name: Alice\nage: 30\n";
        let ours = "name: Alice\nage: 31\n";
        let theirs = "name: Alice\ncity: NYC\n";

        let result = merge_yaml(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
        assert!(result.merged.contains("age: 31"));
        assert!(result.merged.contains("city: NYC"));
    }

    #[test]
    fn merge_yaml_conflict() {
        let base = "key: original\n";
        let ours = "key: ours\n";
        let theirs = "key: theirs\n";

        let result = merge_yaml(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Conflict);
    }

    #[test]
    fn merge_toml_clean() {
        let base = "name = \"Alice\"\nage = 30\n";
        let ours = "name = \"Alice\"\nage = 31\n";
        let theirs = "name = \"Alice\"\ncity = \"NYC\"\n";

        let result = merge_toml(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
    }

    #[test]
    fn merge_toml_conflict() {
        let base = "key = \"original\"\n";
        let ours = "key = \"ours\"\n";
        let theirs = "key = \"theirs\"\n";

        let result = merge_toml(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Conflict);
    }

    #[test]
    fn merge_csv_clean() {
        let base = "name,age,city\nAlice,30,NYC\n";
        let ours = "name,age,city\nAlice,31,NYC\n";
        let theirs = "name,age,city\nAlice,30,SF\n";

        let result = merge_csv(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
    }

    #[test]
    fn merge_csv_conflict() {
        let base = "name,age\nAlice,30\n";
        let ours = "name,age\nAlice,31\n";
        let theirs = "name,age\nAlice,99\n";

        let result = merge_csv(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Conflict);
    }

    #[cfg(feature = "xml")]
    #[test]
    fn merge_xml_clean() {
        let base = "<root><a>1</a><b>2</b></root>";
        let ours = "<root><a>10</a><b>2</b></root>";
        let theirs = "<root><a>1</a><b>20</b></root>";

        let result = merge_xml(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn merge_markdown_clean() {
        let base = "# Title\n\nSome text\n";
        let ours = "# Title\n\nSome updated text\n";
        let theirs = "# Title\n\nSome text\n\nNew paragraph\n";

        let result = merge_markdown(base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
    }

    #[test]
    fn merge_auto_json_detection() {
        let base = r#"{"name": "Alice", "age": 30}"#;
        let ours = r#"{"name": "Alice", "age": 31}"#;
        let theirs = r#"{"name": "Alice", "city": "NYC"}"#;

        let result = merge_auto(base, ours, theirs, Some(".json")).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
        assert!(result.merged.contains(r#""age": 31"#));
        assert!(result.merged.contains(r#""city": "NYC""#));
    }

    #[test]
    fn merge_auto_yaml_detection() {
        let base = "name: Alice\nage: 30\n";
        let ours = "name: Alice\nage: 31\n";
        let theirs = "name: Alice\ncity: NYC\n";

        let result = merge_auto(base, ours, theirs, Some(".yaml")).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
    }

    #[test]
    fn diff_json() {
        let base = r#"{"name": "Alice", "age": 30}"#;
        let modified = r#"{"name": "Bob", "age": 31}"#;

        let changes = diff(base, modified, Some(".json")).unwrap();
        assert!(!changes.is_empty());
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/name"))
        );
    }

    #[test]
    fn format_diff_json() {
        let base = r#"{"name": "Alice"}"#;
        let modified = r#"{"name": "Bob", "email": "bob@example.com"}"#;

        let output = format_diff(base, modified, Some(".json")).unwrap();
        assert!(output.contains("MODIFIED"));
        assert!(output.contains("ADDED"));
    }

    #[test]
    fn merge_result_status_clean() {
        let result = MergeResult {
            merged: "{}".to_string(),
            status: MergeStatus::Clean,
        };
        assert_eq!(result.status, MergeStatus::Clean);
        assert!(result.merged == "{}");
    }

    #[test]
    fn merge_result_status_conflict() {
        let result = MergeResult {
            merged: r#"{"key": "ours"}"#.to_string(),
            status: MergeStatus::Conflict,
        };
        assert_eq!(result.status, MergeStatus::Conflict);
    }

    #[test]
    fn merge_error_unsupported_format() {
        let err = MergeError::UnsupportedFormat("xyz".to_string());
        assert!(err.to_string().contains("xyz"));
    }

    #[test]
    #[cfg(feature = "json")]
    fn merge_resolve_clean() {
        let base = r#"{"name": "Alice", "age": 30}"#;
        let ours = r#"{"name": "Alice", "age": 31}"#;
        let theirs = r#"{"name": "Alice", "city": "NYC"}"#;

        struct TakeOurs;
        impl ConflictResolver for TakeOurs {
            fn resolve(&self, _conflict: &MergeConflict) -> ConflictResolution {
                ConflictResolution::Ours
            }
        }

        let result =
            merge_resolve(Some("data.json"), base, ours, theirs, &TakeOurs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
        assert!(result.merged.contains(r#""age": 31"#));
        assert!(result.merged.contains(r#""city": "NYC""#));
    }

    #[test]
    fn merge_lockfile_json_delegates_to_merge_json() {
        let base = r#"{"name": "Alice", "age": 30}"#;
        let ours = r#"{"name": "Alice", "age": 31}"#;
        let theirs = r#"{"name": "Alice", "city": "NYC"}"#;

        let result = merge_lockfile("package-lock.json", base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
        assert!(result.merged.contains(r#""age": 31"#));
        assert!(result.merged.contains(r#""city": "NYC""#));
    }

    #[test]
    fn merge_lockfile_strategy_ours_changed() {
        let base = "base";
        let ours = "ours-changed";
        let theirs = "base";

        let result = merge_lockfile("Cargo.lock", base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
        assert_eq!(result.merged, "ours-changed");
    }

    #[test]
    fn merge_lockfile_strategy_theirs_changed() {
        let base = "base";
        let ours = "base";
        let theirs = "theirs-changed";

        let result = merge_lockfile("yarn.lock", base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
        assert_eq!(result.merged, "theirs-changed");
    }

    #[test]
    fn merge_lockfile_strategy_both_changed() {
        let base = "base";
        let ours = "ours-changed";
        let theirs = "theirs-changed";

        let result = merge_lockfile("Cargo.lock", base, ours, theirs).unwrap();
        assert_eq!(result.status, MergeStatus::Conflict);
        assert_eq!(result.merged, "theirs-changed");
    }

    #[test]
    fn merge_error_no_driver() {
        let base = "x";
        let ours = "y";
        let theirs = "z";

        let result = merge_auto(base, ours, theirs, Some(".xyz"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("xyz") || err.contains("no driver"));
    }
}
