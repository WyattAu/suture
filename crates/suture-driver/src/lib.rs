//! SutureDriver trait and registry for format-specific drivers.
//!
//! Drivers translate between file formats and semantic patches,
//! enabling Suture to understand *what changed* rather than just
//! *which bytes changed*.

pub mod error;
pub mod registry;
pub mod types;

pub use error::DriverError;
pub use registry::DriverRegistry;
pub use types::{DiffHunk, DiffHunkType, DiffSummary, VisualDiff};

/// Format-specific driver for translating between file formats and Suture patches.
///
/// Implementations must be `Send + Sync` for concurrent use across threads.
/// A driver understands the *semantics* of a file format — it knows that
/// changing a key in a JSON object is a different operation than appending
/// to an array.
pub trait SutureDriver: Send + Sync {
    /// Human-readable driver name (e.g., "JSON", "OpenTimelineIO", "CSV").
    fn name(&self) -> &str;

    /// File extensions this driver handles (e.g., `[".json", ".jsonl"]`).
    fn supported_extensions(&self) -> &[&str];

    /// Parse a file and produce a semantic diff between it and an optional base.
    ///
    /// If `base_content` is `None`, this is a new file — produce creation patches.
    /// If `base_content` is `Some`, produce patches representing the differences.
    ///
    /// Each returned `SemanticChange` describes a meaningful semantic operation
    /// (e.g., "key `users.2.email` changed from `old@example.com` to `new@example.com`").
    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError>;

    /// Produce a human-readable diff string between two versions of a file.
    ///
    /// This is used by `suture diff` when a driver is available for the file type.
    /// The output should be more meaningful than raw line diffs — showing
    /// semantic operations like key changes, array insertions, etc.
    fn format_diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<String, DriverError>;
}

/// A single semantic change detected by a driver.
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticChange {
    /// A value was added at a path (e.g., new key in JSON object).
    Added { path: String, value: String },
    /// A value was removed at a path.
    Removed { path: String, old_value: String },
    /// A value was modified at a path.
    Modified {
        path: String,
        old_value: String,
        new_value: String,
    },
    /// A value was moved/renamed from one path to another.
    Moved {
        old_path: String,
        new_path: String,
        value: String,
    },
}
