/// Structured visual diff produced by a driver.
#[derive(Debug, Clone)]
pub struct VisualDiff {
    pub hunks: Vec<DiffHunk>,
    pub summary: DiffSummary,
}

/// A single hunk in a visual diff.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub path: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub hunk_type: DiffHunkType,
}

/// The type of change in a diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffHunkType {
    Added,
    Removed,
    Modified,
    Moved,
}

/// Summary statistics for a diff.
#[derive(Debug, Clone, Default)]
pub struct DiffSummary {
    pub additions: usize,
    pub removals: usize,
    pub modifications: usize,
    pub moves: usize,
}
