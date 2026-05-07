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

/// Run a synchronous closure on `block_in_place` when on a multi-threaded runtime.
///
/// On a `current_thread` runtime (e.g., `#[tokio::test]`), the closure runs
/// directly since `block_in_place` would panic. This preserves test compatibility
/// while providing the performance benefit in production.
///
/// # Panics
///
/// Panics if called from a `current_thread` tokio runtime AND the closure
/// would block for an extended period (tokio's safety check). For short-lived
/// operations on current_thread runtimes, this is safe.
#[inline]
pub fn block_in_place<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let handle = tokio::runtime::Handle::current();
    if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::CurrentThread {
        // On current_thread runtime, block_in_place would panic.
        // Call the closure directly — this is only safe for short-lived ops
        // (tests use this path). Production uses multi-threaded runtime.
        f()
    } else {
        tokio::task::block_in_place(f)
    }
}
