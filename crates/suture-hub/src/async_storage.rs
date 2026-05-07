//! Async wrapper for HubStorage.
//!
//! Provides `block_in_place` wrappers around the synchronous HubStorage methods
//! to avoid blocking the tokio runtime thread during SQLite I/O.

use crate::storage::{HubStorage, StorageError};
use std::path::Path;

/// Extension trait that provides async-compatible wrappers for HubStorage.
///
/// Uses `tokio::task::block_in_place` to run sync SQLite operations without
/// blocking the tokio runtime thread pool. This is the recommended pattern
/// for calling sync code from async on a multi-threaded runtime.
///
/// # Why not `spawn_blocking`?
///
/// `block_in_place` is preferred here because:
/// 1. It avoids the overhead of task spawning for short-lived SQLite queries
/// 2. It preserves the current tracing/span context
/// 3. It works correctly with the existing `tokio::sync::RwLock<HubStorage>` pattern
///    since we already hold the RwLock when calling these methods
pub trait AsyncHubStorage {
    fn open_async(path: &Path) -> Result<HubStorage, StorageError>;
}

impl AsyncHubStorage for HubStorage {
    fn open_async(path: &Path) -> Result<HubStorage, StorageError> {
        HubStorage::open(path)
    }
}

/// Run a synchronous closure on `block_in_place`.
///
/// This is a convenience function for the common pattern of wrapping
/// HubStorage calls in `block_in_place` when called from async contexts.
///
/// # Panics
///
/// Panics if called from a `current_thread` tokio runtime.
/// The hub server uses a multi-threaded runtime, so this is safe.
#[inline]
pub fn block_in_place<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    tokio::task::block_in_place(f)
}
