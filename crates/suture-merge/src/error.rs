use suture_driver::{DriverError, SutureDriver};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MergeResult {
    pub merged: String,
    pub status: MergeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MergeStatus {
    Clean,
    Conflict,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum MergeError {
    UnsupportedFormat(String),
    ParseError(String),
    NoDriver(String),
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat(fmt) => write!(f, "unsupported format: {fmt}"),
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
            Self::NoDriver(ext) => write!(f, "no driver available for extension: {ext}"),
            #[allow(unreachable_patterns)]
            _ => write!(f, "unknown merge error"),
        }
    }
}

impl std::error::Error for MergeError {}

impl From<DriverError> for MergeError {
    fn from(err: DriverError) -> Self {
        match err {
            DriverError::DriverNotFound(ext) => Self::NoDriver(ext),
            DriverError::UnsupportedExtension(ext) => Self::UnsupportedFormat(ext),
            DriverError::ParseError(msg) | DriverError::SerializationError(msg) => {
                Self::ParseError(msg)
            }
            DriverError::IoError(e) => Self::ParseError(e.to_string()),
        }
    }
}

pub fn perform_merge(
    driver: &dyn SutureDriver,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<MergeResult, MergeError> {
    Ok(driver.merge(base, ours, theirs)?.map_or_else(
        || MergeResult {
            merged: ours.to_owned(),
            status: MergeStatus::Conflict,
        },
        |merged| MergeResult {
            merged,
            status: MergeStatus::Clean,
        },
    ))
}

/// A single merge conflict that can be resolved programmatically.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MergeConflict {
    /// The file path that has the conflict.
    pub path: String,
    /// The base version content.
    pub base: String,
    /// Our version content.
    pub ours: String,
    /// Their version content.
    pub theirs: String,
}

/// Result of a merge that may contain conflicts.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MergeOutput {
    /// Successfully merged content (if no conflicts).
    pub content: Option<String>,
    /// List of unresolved conflicts (empty if merge was clean).
    pub conflicts: Vec<MergeConflict>,
    /// Overall merge status.
    pub status: MergeStatus,
}

/// How to resolve a single conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConflictResolution {
    /// Take our version.
    Ours,
    /// Take their version.
    Theirs,
    /// Take both (concatenate with newline separator).
    Both,
    /// Custom resolution content.
    Custom(String),
}

/// Merge with conflict details returned for programmatic resolution.
///
/// Performs the same semantic merge as [`perform_merge`], but instead of
/// returning only the final result, returns detailed conflict information
/// so callers can resolve conflicts programmatically.
pub fn merge_with_conflicts(base: &str, ours: &str, theirs: &str, path: &str) -> MergeOutput {
    let registry = crate::registry::build_registry();

    let resolved_path = if path.is_empty() {
        None
    } else {
        std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
    };

    let result = if let Some(ref ext) = resolved_path {
        registry.get(ext).ok().and_then(|driver| {
            driver
                .merge(base, ours, theirs)
                .ok()
                .map(|opt| (opt, MergeStatus::Clean))
        })
    } else {
        None
    };

    match result {
        Some((Some(merged), status)) => MergeOutput {
            content: Some(merged),
            conflicts: vec![],
            status,
        },
        _ => MergeOutput {
            content: None,
            conflicts: vec![MergeConflict {
                path: path.to_owned(),
                base: base.to_owned(),
                ours: ours.to_owned(),
                theirs: theirs.to_owned(),
            }],
            status: MergeStatus::Conflict,
        },
    }
}

/// Resolve a conflict by choosing a side.
pub fn resolve_conflict(conflict: &MergeConflict, resolution: &ConflictResolution) -> String {
    match resolution {
        ConflictResolution::Ours => conflict.ours.clone(),
        ConflictResolution::Theirs => conflict.theirs.clone(),
        ConflictResolution::Both => {
            if conflict.ours.is_empty() {
                conflict.theirs.clone()
            } else if conflict.theirs.is_empty() {
                conflict.ours.clone()
            } else {
                format!("{}\n{}", conflict.ours, conflict.theirs)
            }
        }
        ConflictResolution::Custom(content) => content.clone(),
    }
}

#[cfg(test)]
mod conflict_tests {
    use super::*;

    #[test]
    fn clean_merge_returns_no_conflicts() {
        let base = r#"{"name": "Alice", "age": 30}"#;
        let ours = r#"{"name": "Alice", "age": 31}"#;
        let theirs = r#"{"name": "Alice", "city": "NYC"}"#;

        let output = merge_with_conflicts(base, ours, theirs, "data.json");
        assert_eq!(output.status, MergeStatus::Clean);
        assert!(output.content.is_some());
        assert!(output.conflicts.is_empty());
    }

    #[test]
    fn conflicting_merge_returns_conflict() {
        let base = r#"{"key": "original"}"#;
        let ours = r#"{"key": "ours"}"#;
        let theirs = r#"{"key": "theirs"}"#;

        let output = merge_with_conflicts(base, ours, theirs, "data.json");
        assert_eq!(output.status, MergeStatus::Conflict);
        assert!(output.content.is_none());
        assert_eq!(output.conflicts.len(), 1);
        assert_eq!(output.conflicts[0].path, "data.json");
        assert_eq!(output.conflicts[0].base, base);
        assert_eq!(output.conflicts[0].ours, ours);
        assert_eq!(output.conflicts[0].theirs, theirs);
    }

    #[test]
    fn resolve_conflict_ours() {
        let conflict = MergeConflict {
            path: "f.txt".into(),
            base: "base".into(),
            ours: "ours-content".into(),
            theirs: "theirs-content".into(),
        };
        assert_eq!(
            resolve_conflict(&conflict, &ConflictResolution::Ours),
            "ours-content"
        );
    }

    #[test]
    fn resolve_conflict_theirs() {
        let conflict = MergeConflict {
            path: "f.txt".into(),
            base: "base".into(),
            ours: "ours-content".into(),
            theirs: "theirs-content".into(),
        };
        assert_eq!(
            resolve_conflict(&conflict, &ConflictResolution::Theirs),
            "theirs-content"
        );
    }

    #[test]
    fn resolve_conflict_both() {
        let conflict = MergeConflict {
            path: "f.txt".into(),
            base: "base".into(),
            ours: "line-a".into(),
            theirs: "line-b".into(),
        };
        let resolved = resolve_conflict(&conflict, &ConflictResolution::Both);
        assert_eq!(resolved, "line-a\nline-b");
    }

    #[test]
    fn resolve_conflict_both_empty_ours() {
        let conflict = MergeConflict {
            path: "f.txt".into(),
            base: "base".into(),
            ours: String::new(),
            theirs: "line-b".into(),
        };
        assert_eq!(
            resolve_conflict(&conflict, &ConflictResolution::Both),
            "line-b"
        );
    }

    #[test]
    fn resolve_conflict_both_empty_theirs() {
        let conflict = MergeConflict {
            path: "f.txt".into(),
            base: "base".into(),
            ours: "line-a".into(),
            theirs: String::new(),
        };
        assert_eq!(
            resolve_conflict(&conflict, &ConflictResolution::Both),
            "line-a"
        );
    }

    #[test]
    fn resolve_conflict_custom() {
        let conflict = MergeConflict {
            path: "f.txt".into(),
            base: "base".into(),
            ours: "ours".into(),
            theirs: "theirs".into(),
        };
        assert_eq!(
            resolve_conflict(&conflict, &ConflictResolution::Custom("merged".into())),
            "merged"
        );
    }

    #[test]
    fn no_extension_returns_conflict() {
        let output = merge_with_conflicts("a", "b", "c", "noext");
        assert_eq!(output.status, MergeStatus::Conflict);
        assert_eq!(output.conflicts.len(), 1);
    }

    #[test]
    fn empty_path_returns_conflict() {
        let output = merge_with_conflicts("a", "b", "c", "");
        assert_eq!(output.status, MergeStatus::Conflict);
    }

    #[test]
    fn unknown_extension_returns_conflict() {
        let output = merge_with_conflicts("a", "b", "c", "file.xyz123");
        assert_eq!(output.status, MergeStatus::Conflict);
        assert_eq!(output.conflicts[0].path, "file.xyz123");
    }

    #[test]
    fn merge_output_status_matches() {
        let clean = MergeOutput {
            content: Some("ok".into()),
            conflicts: vec![],
            status: MergeStatus::Clean,
        };
        assert_eq!(clean.status, MergeStatus::Clean);

        let conflict = MergeOutput {
            content: None,
            conflicts: vec![MergeConflict {
                path: "x".into(),
                base: "b".into(),
                ours: "o".into(),
                theirs: "t".into(),
            }],
            status: MergeStatus::Conflict,
        };
        assert_eq!(conflict.status, MergeStatus::Conflict);
    }
}
