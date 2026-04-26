#![allow(clippy::collapsible_match)]

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

pub use error::{MergeError, MergeResult, MergeStatus};

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

pub fn merge_auto(
    base: &str,
    ours: &str,
    theirs: &str,
    extension: Option<&str>,
) -> Result<MergeResult, MergeError> {
    let ext = extension
        .ok_or_else(|| MergeError::UnsupportedFormat("no extension provided".to_string()))?;
    let registry = build_registry();
    let driver = registry.get(ext)?;
    perform_merge(driver, base, ours, theirs)
}

pub fn diff(
    base: &str,
    modified: &str,
    extension: Option<&str>,
) -> Result<Vec<SemanticChange>, MergeError> {
    let ext = extension
        .ok_or_else(|| MergeError::UnsupportedFormat("no extension provided".to_string()))?;
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
        .ok_or_else(|| MergeError::UnsupportedFormat("no extension provided".to_string()))?;
    let registry = build_registry();
    let driver = registry.get(ext)?;
    Ok(driver.format_diff(Some(base), modified)?)
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
