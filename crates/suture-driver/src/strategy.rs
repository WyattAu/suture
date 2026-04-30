// SPDX-License-Identifier: MIT OR Apache-2.0
/// Merge strategy selected based on file size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Full semantic merge — best for small files where precision matters most.
    FullSemantic,
    /// Merge at key-path level only — good for medium files.
    KeyPathOnly,
    /// Section-based merge — for very large files where full analysis is costly.
    SectionBased,
}

/// Select an optimal merge strategy based on file size.
///
/// These thresholds are heuristics and can be tuned per use-case.
///
/// # Thresholds
///
/// | Size | Strategy |
/// |------|----------|
/// | < 1 KiB | `FullSemantic` |
/// | < 100 KiB | `KeyPathOnly` |
/// | >= 100 KiB | `SectionBased` |
pub fn optimal_merge_strategy(file_size: usize) -> MergeStrategy {
    if file_size < 1024 {
        MergeStrategy::FullSemantic
    } else if file_size < 100_000 {
        MergeStrategy::KeyPathOnly
    } else {
        MergeStrategy::SectionBased
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_file() {
        assert_eq!(optimal_merge_strategy(512), MergeStrategy::FullSemantic);
    }

    #[test]
    fn test_boundary_small() {
        assert_eq!(optimal_merge_strategy(1023), MergeStrategy::FullSemantic);
    }

    #[test]
    fn test_medium_file() {
        assert_eq!(optimal_merge_strategy(2048), MergeStrategy::KeyPathOnly);
    }

    #[test]
    fn test_boundary_medium() {
        assert_eq!(optimal_merge_strategy(99_999), MergeStrategy::KeyPathOnly);
    }

    #[test]
    fn test_large_file() {
        assert_eq!(optimal_merge_strategy(100_000), MergeStrategy::SectionBased);
    }

    #[test]
    fn test_very_large_file() {
        assert_eq!(optimal_merge_strategy(10_000_000), MergeStrategy::SectionBased);
    }

    #[test]
    fn test_zero_size() {
        assert_eq!(optimal_merge_strategy(0), MergeStrategy::FullSemantic);
    }
}
