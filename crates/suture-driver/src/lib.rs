// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::collapsible_match)]
//! SutureDriver trait and registry for format-specific drivers.
//!
//! Drivers translate between file formats and semantic patches,
//! enabling Suture to understand *what changed* rather than just
//! *which bytes changed*.

pub mod cache;
pub mod error;
pub mod interner;
pub mod plugin;
pub mod registry;
pub mod strategy;
pub mod structured_merge;
pub mod types;

pub use cache::MergeCache;
pub use error::DriverError;
pub use interner::KeyInterner;
pub use plugin::{BuiltinDriverPlugin, DriverPlugin, PluginError, PluginRegistry};
pub use registry::DriverRegistry;
pub use strategy::{optimal_merge_strategy, MergeStrategy};
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

    /// Perform a semantic three-way merge.
    ///
    /// Given base, ours, and theirs content, produce a merged result.
    /// Returns `None` if the merge cannot be resolved automatically (conflict).
    /// Returns `Some(merged_content)` if the merge is clean.
    fn merge(
        &self,
        _base: &str,
        _ours: &str,
        _theirs: &str,
    ) -> Result<Option<String>, DriverError> {
        Ok(None)
    }

    /// Byte-level three-way merge for binary formats.
    ///
    /// Like `merge()` but operates on raw bytes instead of `&str`.
    /// Binary drivers (DOCX, XLSX, PPTX, PDF, images) should override this
    /// to avoid `unsafe { String::from_utf8_unchecked }`.
    /// The default implementation converts to/from UTF-8 lossy and delegates
    /// to `merge()` — text drivers do not need to override this.
    fn merge_raw(
        &self,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<Option<Vec<u8>>, DriverError> {
        let base_str = String::from_utf8_lossy(base);
        let ours_str = String::from_utf8_lossy(ours);
        let theirs_str = String::from_utf8_lossy(theirs);
        match self.merge(&base_str, &ours_str, &theirs_str)? {
            Some(s) => Ok(Some(s.into_bytes())),
            None => Ok(None),
        }
    }

    /// Byte-level semantic diff for binary formats.
    ///
    /// Like `diff()` but operates on raw bytes instead of `&str`.
    /// Binary drivers should override this to avoid `unsafe` conversions.
    fn diff_raw(
        &self,
        base: Option<&[u8]>,
        new_content: &[u8],
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let base_str = base.map(|b| String::from_utf8_lossy(b));
        let new_str = String::from_utf8_lossy(new_content);
        self.diff(base_str.as_deref(), &new_str)
    }
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
