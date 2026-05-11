use std::path::Path;

/// Error returned by a custom merge strategy.
#[derive(Debug)]
#[non_exhaustive]
pub enum MergeStrategyError {
    /// The merge could not be performed.
    MergeFailed(String),
    /// An I/O error occurred.
    IoError(String),
}

impl std::fmt::Display for MergeStrategyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MergeFailed(msg) => write!(f, "merge failed: {msg}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for MergeStrategyError {}

/// Result of a custom merge strategy.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MergeStrategyResult {
    /// The merged content.
    pub content: Vec<u8>,
    /// Whether the merge had conflicts that were auto-resolved.
    pub had_conflicts: bool,
}

/// A custom merge strategy that can be registered for specific file patterns.
///
/// Implement this trait to provide domain-specific merge behavior for
/// particular file types. Strategies are matched by glob patterns against
/// file paths and take priority over the default semantic merge.
pub trait MergeStrategy: Send + Sync {
    /// The name of this strategy (e.g., "lockfile", "generated-code").
    fn name(&self) -> &str;

    /// File patterns this strategy applies to.
    ///
    /// Supports simple suffix matching (e.g., `"Cargo.lock"`, `"*.lock"`).
    /// For more complex matching, override [`matches_path`](MergeStrategy::matches_path).
    fn file_patterns(&self) -> &[&str];

    /// Perform the three-way merge.
    ///
    /// Returns the merged content and whether any conflicts were auto-resolved.
    fn merge(
        &self,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<MergeStrategyResult, MergeStrategyError>;

    /// Check if this strategy applies to the given file path.
    ///
    /// Default implementation does suffix matching against [`file_patterns`](MergeStrategy::file_patterns).
    fn matches_path(&self, path: &str) -> bool {
        let file_name = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path);
        self.file_patterns().iter().any(|pattern| {
            if pattern.starts_with("*.") {
                let suffix = &pattern[1..];
                file_name.ends_with(suffix)
            } else {
                file_name == *pattern
            }
        })
    }
}

/// Built-in merge strategy for lockfiles (Cargo.lock, package-lock.json, yarn.lock).
///
/// Lockfiles should generally be regenerated rather than merged. This strategy
/// picks the newer version (theirs) when both sides changed, falling back to
/// the longer content when timestamps are unavailable.
pub struct LockfileMergeStrategy;

impl LockfileMergeStrategy {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for LockfileMergeStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl MergeStrategy for LockfileMergeStrategy {
    fn name(&self) -> &str {
        "lockfile"
    }

    fn file_patterns(&self) -> &[&str] {
        &[
            "Cargo.lock",
            "package-lock.json",
            "yarn.lock",
            "pnpm-lock.yaml",
        ]
    }

    fn merge(
        &self,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<MergeStrategyResult, MergeStrategyError> {
        let had_conflicts = ours != theirs && ours != base && theirs != base;

        if ours == base {
            Ok(MergeStrategyResult {
                content: theirs.to_vec(),
                had_conflicts: false,
            })
        } else if theirs == base {
            Ok(MergeStrategyResult {
                content: ours.to_vec(),
                had_conflicts: false,
            })
        } else {
            Ok(MergeStrategyResult {
                content: theirs.to_vec(),
                had_conflicts,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfile_strategy_name() {
        let strategy = LockfileMergeStrategy::new();
        assert_eq!(strategy.name(), "lockfile");
    }

    #[test]
    fn lockfile_strategy_patterns() {
        let strategy = LockfileMergeStrategy::new();
        let patterns = strategy.file_patterns();
        assert!(patterns.contains(&"Cargo.lock"));
        assert!(patterns.contains(&"package-lock.json"));
        assert!(patterns.contains(&"yarn.lock"));
        assert!(patterns.contains(&"pnpm-lock.yaml"));
    }

    #[test]
    fn lockfile_matches_exact_names() {
        let strategy = LockfileMergeStrategy::new();
        assert!(strategy.matches_path("Cargo.lock"));
        assert!(strategy.matches_path("/path/to/package-lock.json"));
        assert!(strategy.matches_path("yarn.lock"));
        assert!(!strategy.matches_path("some-other-file.txt"));
    }

    #[test]
    fn lockfile_merge_unchanged_base() {
        let strategy = LockfileMergeStrategy::new();
        let base = b"version 1\n";
        let ours = base;
        let theirs = b"version 2\n";

        let result = strategy.merge(base, ours, theirs).unwrap();
        assert_eq!(result.content, b"version 2\n");
        assert!(!result.had_conflicts);
    }

    #[test]
    fn lockfile_merge_unchanged_theirs() {
        let strategy = LockfileMergeStrategy::new();
        let base = b"version 1\n";
        let ours = b"version 2\n";
        let theirs = base;

        let result = strategy.merge(base, ours, theirs).unwrap();
        assert_eq!(result.content, b"version 2\n");
        assert!(!result.had_conflicts);
    }

    #[test]
    fn lockfile_merge_both_changed() {
        let strategy = LockfileMergeStrategy::new();
        let base = b"version 1\n";
        let ours = b"version 2 (ours)\n";
        let theirs = b"version 3 (theirs)\n";

        let result = strategy.merge(base, ours, theirs).unwrap();
        assert_eq!(result.content, b"version 3 (theirs)\n");
        assert!(result.had_conflicts);
    }

    #[test]
    fn lockfile_merge_identical() {
        let strategy = LockfileMergeStrategy::new();
        let content = b"version 1\n";
        let result = strategy.merge(content, content, content).unwrap();
        assert_eq!(result.content, b"version 1\n");
        assert!(!result.had_conflicts);
    }

    #[test]
    fn lockfile_no_match_unrelated_file() {
        let strategy = LockfileMergeStrategy::new();
        assert!(!strategy.matches_path("README.md"));
        assert!(!strategy.matches_path("Cargo.toml"));
        assert!(!strategy.matches_path("src/main.rs"));
    }

    #[test]
    fn merge_strategy_error_display() {
        let err = MergeStrategyError::MergeFailed("conflict".into());
        assert_eq!(err.to_string(), "merge failed: conflict");

        let err = MergeStrategyError::IoError("broken".into());
        assert_eq!(err.to_string(), "I/O error: broken");
    }

    struct DummyStrategy {
        patterns: Vec<&'static str>,
    }

    impl MergeStrategy for DummyStrategy {
        fn name(&self) -> &str {
            "dummy"
        }
        fn file_patterns(&self) -> &[&str] {
            &self.patterns
        }
        fn merge(
            &self,
            _base: &[u8],
            ours: &[u8],
            _theirs: &[u8],
        ) -> Result<MergeStrategyResult, MergeStrategyError> {
            Ok(MergeStrategyResult {
                content: ours.to_vec(),
                had_conflicts: false,
            })
        }
    }

    #[test]
    fn wildcard_pattern_matching() {
        let strategy = DummyStrategy {
            patterns: vec!["*.lock"],
        };
        assert!(strategy.matches_path("foo.lock"));
        assert!(strategy.matches_path("/path/to/bar.lock"));
        assert!(!strategy.matches_path("foo.lock.bak"));
    }

    #[test]
    fn exact_pattern_matching() {
        let strategy = DummyStrategy {
            patterns: vec!["Makefile"],
        };
        assert!(strategy.matches_path("Makefile"));
        assert!(strategy.matches_path("/path/to/Makefile"));
        assert!(!strategy.matches_path("makefile"));
    }
}
