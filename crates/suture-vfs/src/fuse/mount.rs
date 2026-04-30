#![allow(clippy::arc_with_non_send_sync)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum VfsError {
    #[error("FUSE mount failed: {0}")]
    MountFailed(String),
    #[error("FUSE unmount failed: {0}")]
    UnmountFailed(String),
    #[error("mount point not found: {0}")]
    NotFound(String),
    #[error("mount point already in use: {0}")]
    AlreadyMounted(String),
    #[error("repository error: {0}")]
    Repository(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct MountInfo {
    pub mount_point: PathBuf,
    pub repo_path: PathBuf,
    pub branch: Option<String>,
    pub read_only: bool,
    pub mounted_at: std::time::Instant,
}

pub struct MountHandle {
    pub info: MountInfo,
    cancel: tokio::sync::oneshot::Sender<()>,
}

impl MountHandle {
    pub fn info(&self) -> &MountInfo {
        &self.info
    }

    pub fn unmount(self) -> Result<(), VfsError> {
        let _ = self.cancel.send(());
        Ok(())
    }
}

pub struct MountManager {
    mounts: HashMap<String, MountHandle>,
}

impl MountManager {
    pub fn new() -> Self {
        Self {
            mounts: HashMap::new(),
        }
    }

    pub fn mount(
        &mut self,
        repo_path: &Path,
        mount_point: &Path,
        branch: Option<&str>,
        read_only: bool,
    ) -> Result<&MountHandle, VfsError> {
        let key = mount_point.to_string_lossy().to_string();

        if self.mounts.contains_key(&key) {
            return Err(VfsError::AlreadyMounted(key));
        }

        if !repo_path.exists() {
            return Err(VfsError::Repository(format!(
                "path does not exist: {}",
                repo_path.display()
            )));
        }

        let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();

        let handle = MountHandle {
            info: MountInfo {
                mount_point: mount_point.to_path_buf(),
                repo_path: repo_path.to_path_buf(),
                branch: branch.map(|s| s.to_string()),
                read_only,
                mounted_at: std::time::Instant::now(),
            },
            cancel: cancel_tx,
        };

        self.mounts.insert(key.clone(), handle);
        Ok(self.mounts.get(&key).unwrap())
    }

    pub fn unmount(&mut self, mount_point: &Path) -> Result<(), VfsError> {
        let key = mount_point.to_string_lossy().to_string();
        let handle = self
            .mounts
            .remove(&key)
            .ok_or_else(|| VfsError::NotFound(key.clone()))?;
        handle.unmount()
    }

    pub fn list_mounts(&self) -> Vec<&MountInfo> {
        self.mounts.values().map(|h| &h.info).collect()
    }

    pub fn get_mount(&self, mount_point: &Path) -> Option<&MountHandle> {
        let key = mount_point.to_string_lossy().to_string();
        self.mounts.get(&key)
    }

    pub fn unmount_all(&mut self) {
        self.mounts.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.mounts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.mounts.len()
    }
}

impl Default for MountManager {
    fn default() -> Self {
        Self::new()
    }
}

fn errno_to_message(errno: i32) -> String {
    match errno {
        libc::EACCES | libc::EPERM => "Permission denied (try running as root)".to_string(),
        libc::ENOENT => "Mount point or repository path not found".to_string(),
        libc::EBUSY => "Mount point is already in use".to_string(),
        libc::ENODEV => "FUSE device not found (is fuse module loaded?)".to_string(),
        libc::EINVAL => "Invalid mount options".to_string(),
        libc::ENOMEM => "Out of memory".to_string(),
        libc::ENOSYS => "FUSE not supported by the kernel".to_string(),
        _ => format!("FUSE error (errno {errno})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_manager_basic() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let mnt = dir.path().join("mnt");

        let mut mgr = MountManager::new();
        assert!(mgr.is_empty());

        let handle = mgr.mount(&repo, &mnt, Some("main"), true).unwrap();
        assert_eq!(handle.info.repo_path, repo);
        assert_eq!(handle.info.mount_point, mnt);
        assert_eq!(handle.info.branch.as_deref(), Some("main"));
        assert!(handle.info.read_only);
        assert_eq!(mgr.len(), 1);

        let mounts = mgr.list_mounts();
        assert_eq!(mounts.len(), 1);

        mgr.unmount(&mnt).unwrap();
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_mount_manager_double_mount() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let mnt = dir.path().join("mnt");

        let mut mgr = MountManager::new();
        let _ = mgr.mount(&repo, &mnt, None, false).unwrap();

        let result = mgr.mount(&repo, &mnt, None, false);
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::AlreadyMounted(_) => {}
            other => panic!("expected AlreadyMounted, got: {other}"),
        }
    }

    #[test]
    fn test_mount_manager_unmount_nonexistent() {
        let mut mgr = MountManager::new();
        let result = mgr.unmount(Path::new("/nonexistent"));
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::NotFound(_) => {}
            other => panic!("expected NotFound, got: {other}"),
        }
    }

    #[test]
    fn test_mount_manager_nonexistent_repo() {
        let dir = tempfile::tempdir().unwrap();
        let mnt = dir.path().join("mnt");
        let mut mgr = MountManager::new();

        let result = mgr.mount(Path::new("/nonexistent/repo"), &mnt, None, false);
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::Repository(_) => {}
            other => panic!("expected Repository error, got: {other}"),
        }
    }

    #[test]
    fn test_errno_to_message() {
        assert!(errno_to_message(libc::EACCES).contains("Permission"));
        assert!(errno_to_message(libc::ENOENT).contains("not found"));
        assert!(errno_to_message(libc::ENODEV).contains("FUSE device"));
        assert!(errno_to_message(9999).contains("9999"));
    }
}
