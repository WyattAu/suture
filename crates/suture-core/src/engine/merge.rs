//! Line-level three-way merge algorithm.
//!
//! Given a common base and two modified versions, produces a merged result
//! with minimal conflict markers. Uses a longest common subsequence (LCS)
//! approach to compute diffs and merge them intelligently.

/// A change in a sequence of lines relative to a base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineChange {
    /// Lines that are unchanged between base and modified.
    Unchanged(Vec<String>),
    /// Lines present in base but deleted in modified.
    Deleted(Vec<String>),
    /// Lines inserted in modified (not in base).
    Inserted(Vec<String>),
}

impl LineChange {
    #[allow(dead_code)]
    fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged(_))
    }
}

/// Compute the diff between two line sequences using LCS.
///
/// Returns a list of changes that transform `base` into `modified`.
#[must_use] 
pub fn diff_lines(base: &[&str], modified: &[&str]) -> Vec<LineChange> {
    if base.is_empty() && modified.is_empty() {
        return Vec::new();
    }
    if base.is_empty() {
        return vec![LineChange::Inserted(
            modified.iter().map(std::string::ToString::to_string).collect(),
        )];
    }
    if modified.is_empty() {
        return vec![LineChange::Deleted(
            base.iter().map(std::string::ToString::to_string).collect(),
        )];
    }

    let m = base.len();
    let n = modified.len();

    // Build LCS DP table
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if base[i - 1] == modified[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to produce diff
    let mut changes = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && base[i - 1] == modified[j - 1] {
            changes.push(LineChange::Unchanged(vec![base[i - 1].to_owned()]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            changes.push(LineChange::Inserted(vec![modified[j - 1].to_owned()]));
            j -= 1;
        } else {
            changes.push(LineChange::Deleted(vec![base[i - 1].to_owned()]));
            i -= 1;
        }
    }

    changes.reverse();
    coalesce_changes(changes)
}

/// Try to extend the last change in `result` with `change` if they're the same variant.
/// Returns true if merged, false if not (caller should push `change`).
fn try_extend_last(result: &mut [LineChange], change: &LineChange) -> bool {
    let can_merge = match result.last() {
        Some(LineChange::Unchanged(_)) => matches!(change, LineChange::Unchanged(_)),
        Some(LineChange::Deleted(_)) => matches!(change, LineChange::Deleted(_)),
        Some(LineChange::Inserted(_)) => matches!(change, LineChange::Inserted(_)),
        None => false,
    };
    if !can_merge {
        return false;
    }
    let Some(last) = result.last_mut() else {
        return false;
    };
    match (last, change) {
        (LineChange::Unchanged(v), LineChange::Unchanged(new))
        | (LineChange::Deleted(v), LineChange::Deleted(new))
        | (LineChange::Inserted(v), LineChange::Inserted(new)) => {
            v.extend(new.iter().cloned());
            true
        }
        _ => false,
    }
}

/// Merge consecutive changes of the same type.
fn coalesce_changes(changes: Vec<LineChange>) -> Vec<LineChange> {
    let mut result = Vec::new();
    for change in changes {
        if !try_extend_last(&mut result, &change) {
            result.push(change);
        }
    }
    result
}

/// Result of a line-level three-way merge.
#[derive(Debug, Clone)]
pub struct MergeOutput {
    /// The merged content (with conflict markers if any).
    pub lines: Vec<String>,
    /// Whether the merge was clean (no conflicts).
    pub is_clean: bool,
    /// Number of auto-merged regions.
    pub auto_merged: usize,
    /// Number of conflicting regions.
    pub conflicts: usize,
}

/// Perform a line-level three-way merge.
///
/// Given base, ours, and theirs content:
/// 1. Compute diffs base→ours and base→theirs
/// 2. Extract edit chunks from both
/// 3. For each chunk: auto-merge if possible, otherwise conflict markers
///
/// Returns a `MergeOutput` with the merged lines and conflict status.
#[must_use] 
pub fn three_way_merge_lines(
    base: &[&str],
    ours: &[&str],
    theirs: &[&str],
    ours_label: &str,
    theirs_label: &str,
) -> MergeOutput {
    // Trivial cases
    if ours == theirs {
        return MergeOutput {
            lines: ours.iter().map(std::string::ToString::to_string).collect(),
            is_clean: true,
            auto_merged: 0,
            conflicts: 0,
        };
    }
    if base == ours {
        return MergeOutput {
            lines: theirs.iter().map(std::string::ToString::to_string).collect(),
            is_clean: true,
            auto_merged: 0,
            conflicts: 0,
        };
    }
    if base == theirs {
        return MergeOutput {
            lines: ours.iter().map(std::string::ToString::to_string).collect(),
            is_clean: true,
            auto_merged: 0,
            conflicts: 0,
        };
    }

    let ours_diff = diff_lines(base, ours);
    let theirs_diff = diff_lines(base, theirs);

    let mut merged = Vec::new();
    let mut is_clean = true;
    let mut auto_merged = 0usize;
    let mut conflicts = 0usize;

    // Build a map of base line index → what each side did at that position
    // Use `mut` so we can `remove` entries to avoid infinite loops on Insert actions.
    let mut ours_map = build_change_map(base, &ours_diff);
    let mut theirs_map = build_change_map(base, &theirs_diff);

    let base_len = base.len();
    let mut i = 0;

    while i < base_len {
        let ours_action = ours_map.remove(&i);
        let theirs_action = theirs_map.remove(&i);

        match (ours_action, theirs_action) {
            (None, None) => {
                // Both sides kept this line
                merged.push(base[i].to_owned());
                i += 1;
            }
            (Some(a), None) => {
                // Only ours changed
                apply_action(&mut merged, &a, &mut i);
            }
            (None, Some(a)) => {
                // Only theirs changed
                apply_action(&mut merged, &a, &mut i);
            }
            (Some(a), Some(b)) => {
                // Both sides changed at the same base position
                // Check if they made the same change
                if a.output_lines() == b.output_lines() {
                    // Same change — take either
                    merged.extend(a.output_lines());
                    i += a.advance();
                    auto_merged += 1;
                } else {
                    // Conflict!
                    is_clean = false;
                    conflicts += 1;
                    merged.push(format!("<<<<<<< {ours_label}"));
                    merged.extend(a.output_lines());
                    merged.push("=======".to_owned());
                    merged.extend(b.output_lines());
                    merged.push(format!(">>>>>>> {theirs_label}"));
                    // Advance past the longer action
                    i += a.advance().max(b.advance());
                }
            }
        }
    }

    // Handle trailing insertions (after the last base line).
    // Both sides may have appended content — check for conflicts.
    let ours_trailing = ours_map.remove(&base_len);
    let theirs_trailing = theirs_map.remove(&base_len);
    match (ours_trailing, theirs_trailing) {
        (None, None) => {}
        (Some(a), None) | (None, Some(a)) => {
            merged.extend(a.output_lines());
        }
        (Some(a), Some(b)) => {
            if a.output_lines() == b.output_lines() {
                merged.extend(a.output_lines());
            } else {
                is_clean = false;
                conflicts += 1;
                merged.push(format!("<<<<<<< {ours_label}"));
                merged.extend(a.output_lines());
                merged.push("=======".to_owned());
                merged.extend(b.output_lines());
                merged.push(format!(">>>>>>> {theirs_label}"));
            }
        }
    }

    MergeOutput {
        lines: merged,
        is_clean,
        auto_merged,
        conflicts,
    }
}

/// What one side did at a particular base line position.
#[derive(Debug, Clone)]
enum SideAction {
    /// Deleted N lines starting at this position, then inserted M lines.
    DeleteInsert {
        deleted: usize,
        inserted: Vec<String>,
    },
    /// Inserted lines at this position (no deletion).
    Insert { lines: Vec<String> },
}

impl SideAction {
    fn advance(&self) -> usize {
        match self {
            Self::DeleteInsert { deleted, .. } => *deleted,
            Self::Insert { .. } => 0,
        }
    }

    fn output_lines(&self) -> Vec<String> {
        match self {
            Self::DeleteInsert { inserted, .. } => inserted.clone(),
            Self::Insert { lines } => lines.clone(),
        }
    }
}

/// Build a map from base line index to side action.
///
/// Consecutive `Deleted` + `Inserted` changes are merged into a single
/// `DeleteInsert` so that replacements are handled atomically.
fn build_change_map(
    _base: &[&str],
    changes: &[LineChange],
) -> std::collections::HashMap<usize, SideAction> {
    let mut map = std::collections::HashMap::new();
    let mut base_idx = 0;
    // Track the index of the most recent DeleteInsert so that a following
    // Inserted can be merged into it (representing a replacement).
    let mut last_delete_idx: Option<usize> = None;

    for change in changes {
        match change {
            LineChange::Unchanged(lines) => {
                base_idx += lines.len();
                last_delete_idx = None;
            }
            LineChange::Deleted(lines) => {
                map.insert(
                    base_idx,
                    SideAction::DeleteInsert {
                        deleted: lines.len(),
                        inserted: Vec::new(),
                    },
                );
                last_delete_idx = Some(base_idx);
                base_idx += lines.len();
            }
            LineChange::Inserted(lines) => {
                if let Some(del_idx) = last_delete_idx {
                    // Merge into the preceding DeleteInsert (replacement).
                    if let Some(SideAction::DeleteInsert { inserted, .. }) = map.get_mut(&del_idx) {
                        inserted.extend(lines.iter().cloned());
                    }
                } else if let Some(existing) = map.get_mut(&base_idx) {
                    // Merge with an existing action at this position.
                    match existing {
                        SideAction::DeleteInsert { inserted, .. } => {
                            inserted.extend(lines.iter().cloned());
                        }
                        SideAction::Insert { lines: v } => {
                            v.extend(lines.iter().cloned());
                        }
                    }
                } else {
                    map.insert(
                        base_idx,
                        SideAction::Insert {
                            lines: lines.clone(),
                        },
                    );
                }
                // Insertions don't advance base_idx
            }
        }
    }

    map
}

/// Apply a side action to the merged output.
fn apply_action(merged: &mut Vec<String>, action: &SideAction, base_idx: &mut usize) {
    match action {
        SideAction::DeleteInsert { deleted, inserted } => {
            merged.extend(inserted.iter().cloned());
            *base_idx += deleted;
        }
        SideAction::Insert { lines } => {
            merged.extend(lines.iter().cloned());
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_identical() {
        let base = ["a", "b", "c"];
        let changes = diff_lines(&base, &base);
        assert_eq!(changes.len(), 1);
        assert!(matches!(changes[0], LineChange::Unchanged(_)));
    }

    #[test]
    fn test_diff_insert() {
        let base = ["a", "c"];
        let modified = ["a", "b", "c"];
        let changes = diff_lines(&base, &modified);
        assert_eq!(changes.len(), 3); // Unchanged(a), Inserted(b), Unchanged(c)
        assert!(matches!(&changes[1], LineChange::Inserted(v) if v == &["b"]));
    }

    #[test]
    fn test_diff_delete() {
        let base = ["a", "b", "c"];
        let modified = ["a", "c"];
        let changes = diff_lines(&base, &modified);
        assert_eq!(changes.len(), 3);
        assert!(matches!(&changes[1], LineChange::Deleted(v) if v == &["b"]));
    }

    #[test]
    fn test_diff_replace() {
        let base = ["a", "b", "c"];
        let modified = ["a", "x", "c"];
        let changes = diff_lines(&base, &modified);
        assert!(changes.iter().any(|c| matches!(c, LineChange::Deleted(_))));
        assert!(changes.iter().any(|c| matches!(c, LineChange::Inserted(_))));
    }

    #[test]
    fn test_merge_trivial_unchanged() {
        let base = ["a", "b", "c"];
        let result = three_way_merge_lines(&base, &base, &base, "ours", "theirs");
        assert!(result.is_clean);
        assert_eq!(result.lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_merge_one_side_changed() {
        let base = ["a", "b", "c"];
        let ours = ["a", "X", "c"];
        let theirs = ["a", "b", "c"];
        let result = three_way_merge_lines(&base, &ours, &theirs, "ours", "theirs");
        assert!(result.is_clean);
        assert_eq!(result.lines, vec!["a", "X", "c"]);
    }

    #[test]
    fn test_merge_both_sides_different_regions() {
        let base = ["a", "b", "c", "d"];
        let ours = ["a", "X", "c", "d"]; // changed line 2
        let theirs = ["a", "b", "c", "Y"]; // changed line 4
        let result = three_way_merge_lines(&base, &ours, &theirs, "ours", "theirs");
        assert!(result.is_clean);
        assert_eq!(result.lines, vec!["a", "X", "c", "Y"]);
    }

    #[test]
    fn test_merge_both_sides_same_change() {
        let base = ["a", "b", "c"];
        let ours = ["a", "X", "c"];
        let theirs = ["a", "X", "c"];
        let result = three_way_merge_lines(&base, &ours, &theirs, "ours", "theirs");
        assert!(result.is_clean);
        assert_eq!(result.lines, vec!["a", "X", "c"]);
    }

    #[test]
    fn test_merge_conflict() {
        let base = ["a", "b", "c"];
        let ours = ["a", "X", "c"];
        let theirs = ["a", "Y", "c"];
        let result = three_way_merge_lines(&base, &ours, &theirs, "ours", "theirs");
        assert!(!result.is_clean);
        assert_eq!(result.conflicts, 1);
        let content = result.lines.join("\n");
        assert!(content.contains("X"));
        assert!(content.contains("Y"));
        assert!(content.contains("<<<<<<< ours"));
        assert!(content.contains(">>>>>>> theirs"));
    }

    #[test]
    fn test_merge_conflict_markers_format() {
        let base = ["line1"];
        let ours = ["ours_version"];
        let theirs = ["theirs_version"];
        let result = three_way_merge_lines(&base, &ours, &theirs, "ours (HEAD)", "theirs");
        assert!(!result.is_clean);
        let lines = result.lines;
        assert_eq!(lines[0], "<<<<<<< ours (HEAD)");
        assert_eq!(lines[1], "ours_version");
        assert_eq!(lines[2], "=======");
        assert_eq!(lines[3], "theirs_version");
        assert_eq!(lines[4], ">>>>>>> theirs");
    }

    #[test]
    fn test_merge_additions_both_sides() {
        let base = ["a", "c"];
        let ours = ["a", "b", "c"]; // inserted b
        let theirs = ["a", "c", "d"]; // appended d
        let result = three_way_merge_lines(&base, &ours, &theirs, "ours", "theirs");
        assert!(result.is_clean);
        // Both insertions should be present
        assert!(result.lines.contains(&"b".to_string()));
        assert!(result.lines.contains(&"d".to_string()));
    }

    #[test]
    fn test_merge_empty_base() {
        let base: [&str; 0] = [];
        let ours = ["a", "b"];
        let theirs = ["c", "d"];
        let result = three_way_merge_lines(&base, &ours, &theirs, "ours", "theirs");
        assert!(!result.is_clean);
        assert_eq!(result.conflicts, 1);
    }

    #[test]
    fn test_merge_large_file_different_regions() {
        let base: Vec<String> = (0..20).map(|i| format!("line {}", i)).collect();
        let mut ours = base.clone();
        let mut theirs = base.clone();

        // Ours changes lines 0-4
        for (i, item) in ours.iter_mut().enumerate().take(5) {
            *item = format!("OURS {}", i);
        }
        // Theirs changes lines 15-19
        for (i, item) in theirs.iter_mut().enumerate().skip(15) {
            *item = format!("THEIRS {}", i);
        }

        let base_refs: Vec<&str> = base.iter().map(|s| s.as_str()).collect();
        let ours_refs: Vec<&str> = ours.iter().map(|s| s.as_str()).collect();
        let theirs_refs: Vec<&str> = theirs.iter().map(|s| s.as_str()).collect();

        let result = three_way_merge_lines(&base_refs, &ours_refs, &theirs_refs, "ours", "theirs");
        assert!(result.is_clean, "should auto-merge non-overlapping changes");
        assert_eq!(result.lines.len(), 20);
        assert_eq!(result.lines[0], "OURS 0");
        assert_eq!(result.lines[15], "THEIRS 15");
        // Middle lines should be unchanged
        assert_eq!(result.lines[10], "line 10");
    }
}
