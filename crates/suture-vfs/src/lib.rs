// SPDX-License-Identifier: MIT OR Apache-2.0

/// Extension trait for recovering from poisoned `std::sync::Mutex` locks.
/// In filesystem code (FUSE / WebDAV) a panic is unrecoverable, so we prefer
/// to continue operating with the guarded data rather than crash.
pub(crate) trait UnpoisonMutex<T> {
    fn unpoison_lock(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> UnpoisonMutex<T> for std::sync::Mutex<T> {
    fn unpoison_lock(&self) -> std::sync::MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

pub mod fuse;
pub mod path_translation;
pub mod webdav;
