// SPDX-License-Identifier: MIT OR Apache-2.0
/// Merge granularity selected based on file size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeGranularity {
    /// Full semantic merge — best for small files where precision matters most.
    FullSemantic,
    /// Merge at key-path level only — good for medium files.
    KeyPathOnly,
    /// Section-based merge — for very large files where full analysis is costly.
    SectionBased,
}

/// Select an optimal merge granularity based on file size.
///
/// These thresholds are heuristics and can be tuned per use-case.
///
/// # Thresholds
///
/// | Size | Granularity |
/// |------|-------------|
/// | < 1 KiB | `FullSemantic` |
/// | < 100 KiB | `KeyPathOnly` |
/// | >= 100 KiB | `SectionBased` |
#[must_use]
pub fn optimal_merge_granularity(file_size: usize) -> MergeGranularity {
    if file_size < 1024 {
        MergeGranularity::FullSemantic
    } else if file_size < 100_000 {
        MergeGranularity::KeyPathOnly
    } else {
        MergeGranularity::SectionBased
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_file() {
        assert_eq!(
            optimal_merge_granularity(512),
            MergeGranularity::FullSemantic
        );
    }

    #[test]
    fn test_boundary_small() {
        assert_eq!(
            optimal_merge_granularity(1023),
            MergeGranularity::FullSemantic
        );
    }

    #[test]
    fn test_medium_file() {
        assert_eq!(
            optimal_merge_granularity(2048),
            MergeGranularity::KeyPathOnly
        );
    }

    #[test]
    fn test_boundary_medium() {
        assert_eq!(
            optimal_merge_granularity(99_999),
            MergeGranularity::KeyPathOnly
        );
    }

    #[test]
    fn test_large_file() {
        assert_eq!(
            optimal_merge_granularity(100_000),
            MergeGranularity::SectionBased
        );
    }

    #[test]
    fn test_very_large_file() {
        assert_eq!(
            optimal_merge_granularity(10_000_000),
            MergeGranularity::SectionBased
        );
    }

    #[test]
    fn test_zero_size() {
        assert_eq!(optimal_merge_granularity(0), MergeGranularity::FullSemantic);
    }
}
