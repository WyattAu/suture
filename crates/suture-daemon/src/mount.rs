use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountType {
    Fuse,
    WebDav,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MountStatus {
    /// Test-only: mount operation has been initiated but not yet completed.
    #[cfg(test)]
    Pending,
    Active,
    /// Test-only: mount operation failed with an error.
    #[cfg(test)]
    Error(String),
    Stopped,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used in tests and by consumers
pub struct MountPoint {
    pub id: String,
    pub repo_path: PathBuf,
    pub mount_path: PathBuf,
    pub mount_type: MountType,
    pub pid: Option<u32>,
    pub status: MountStatus,
    pub mounted_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Variants used in tests and by consumers
pub enum MountError {
    NotFound,
    AlreadyMounted,
    MountFailed(String),
    UnmountFailed(String),
    InvalidPath,
}

impl std::fmt::Display for MountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "mount not found"),
            Self::AlreadyMounted => write!(f, "already mounted at this location"),
            Self::MountFailed(msg) => write!(f, "mount failed: {msg}"),
            Self::UnmountFailed(msg) => write!(f, "unmount failed: {msg}"),
            Self::InvalidPath => write!(f, "invalid mount path"),
        }
    }
}

impl std::error::Error for MountError {}

pub struct MountManager {
    mounts: HashMap<String, MountPoint>,
    webdav_handles: HashMap<String, tokio::task::JoinHandle<()>>,
    fuse_manager: suture_vfs::fuse::MountManager,
    repo_path: PathBuf,
    next_id: u64,
}

#[allow(dead_code)] // Methods used in tests and by consumers
impl MountManager {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            mounts: HashMap::new(),
            webdav_handles: HashMap::new(),
            fuse_manager: suture_vfs::fuse::MountManager::new(),
            repo_path,
            next_id: 1,
        }
    }

    fn generate_id(&mut self) -> String {
        let id = format!("mnt-{}", self.next_id);
        self.next_id += 1;
        id
    }

    pub fn mount_fuse(&mut self, mount_path: &Path) -> Result<String, MountError> {
        if mount_path.as_os_str().is_empty() {
            return Err(MountError::InvalidPath);
        }

        for mount in self.mounts.values() {
            if mount.mount_type == MountType::Fuse && mount.mount_path == mount_path {
                return Err(MountError::AlreadyMounted);
            }
        }

        self.fuse_manager
            .mount(&self.repo_path, mount_path, None, true)
            .map_err(|e| MountError::MountFailed(e.to_string()))?;

        let id = self.generate_id();

        let mount_point = MountPoint {
            id: id.clone(),
            repo_path: self.repo_path.clone(),
            mount_path: mount_path.to_path_buf(),
            mount_type: MountType::Fuse,
            pid: Some(std::process::id()),
            status: MountStatus::Active,
            mounted_at: Some(SystemTime::now()),
        };

        self.mounts.insert(id.clone(), mount_point);
        Ok(id)
    }

    pub fn mount_webdav(&mut self, port: u16) -> Result<String, MountError> {
        let mount_addr = format!("127.0.0.1:{port}");

        for mount in self.mounts.values() {
            if mount.mount_type == MountType::WebDav
                && mount.mount_path.to_string_lossy() == mount_addr
            {
                return Err(MountError::AlreadyMounted);
            }
        }

        let id = self.generate_id();
        let repo_path_str = self.repo_path.to_string_lossy().to_string();
        let serve_addr = mount_addr.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = suture_vfs::webdav::serve_webdav(&repo_path_str, &serve_addr).await {
                tracing::error!("WebDAV server error: {e}");
            }
        });

        let mount_point = MountPoint {
            id: id.clone(),
            repo_path: self.repo_path.clone(),
            mount_path: PathBuf::from(&mount_addr),
            mount_type: MountType::WebDav,
            pid: None,
            status: MountStatus::Active,
            mounted_at: Some(SystemTime::now()),
        };

        self.webdav_handles.insert(id.clone(), handle);
        self.mounts.insert(id.clone(), mount_point);
        Ok(id)
    }

    pub fn unmount(&mut self, mount_id: &str) -> Result<(), MountError> {
        let mount = self.mounts.get(mount_id).ok_or(MountError::NotFound)?;

        match mount.mount_type {
            MountType::Fuse => {
                self.fuse_manager
                    .unmount(&mount.mount_path)
                    .map_err(|e| MountError::UnmountFailed(e.to_string()))?;
            }
            MountType::WebDav => {
                if let Some(handle) = self.webdav_handles.remove(mount_id) {
                    handle.abort();
                }
            }
        }

        if let Some(mount) = self.mounts.get_mut(mount_id) {
            mount.status = MountStatus::Stopped;
            mount.mounted_at = None;
        }

        Ok(())
    }

    pub fn list_mounts(&self) -> Vec<&MountPoint> {
        self.mounts.values().collect()
    }

    pub fn get_mount(&self, mount_id: &str) -> Option<&MountPoint> {
        self.mounts.get(mount_id)
    }

    pub fn status(&self, mount_id: &str) -> Result<MountStatus, MountError> {
        self.mounts
            .get(mount_id)
            .map(|m| m.status.clone())
            .ok_or(MountError::NotFound)
    }

    pub fn stop_all(&mut self) {
        for (_, handle) in self.webdav_handles.drain() {
            handle.abort();
        }

        self.fuse_manager.unmount_all();

        for mount in self.mounts.values_mut() {
            mount.status = MountStatus::Stopped;
            mount.mounted_at = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[tokio::test]
    async fn test_mount_webdav_create_and_stop() {
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);

        let id = manager.mount_webdav(18080).unwrap();
        assert!(id.starts_with("mnt-"));

        let mount = manager.get_mount(&id).unwrap();
        assert_eq!(mount.mount_type, MountType::WebDav);
        assert_eq!(mount.status, MountStatus::Active);
        assert!(mount.mounted_at.is_some());
        assert!(mount.pid.is_none());
        assert_eq!(mount.mount_path.to_string_lossy(), "127.0.0.1:18080");

        manager.unmount(&id).unwrap();
        assert_eq!(manager.status(&id).unwrap(), MountStatus::Stopped);
    }

    #[tokio::test]
    async fn test_list_mounts() {
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);

        let id2 = manager.mount_webdav(18081).unwrap();

        let mounts = manager.list_mounts();
        assert_eq!(mounts.len(), 1);
        assert!(mounts.iter().any(|m| m.id == id2));

        manager.unmount(&id2).unwrap();
    }

    #[tokio::test]
    async fn test_unmount_nonexistent() {
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);

        let result = manager.unmount("mnt-99999");
        assert!(matches!(result, Err(MountError::NotFound)));
    }

    #[tokio::test]
    async fn test_stop_all() {
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);

        manager.mount_webdav(18082).unwrap();

        assert_eq!(manager.list_mounts().len(), 1);

        manager.stop_all();

        for mount in manager.list_mounts() {
            assert_eq!(mount.status, MountStatus::Stopped);
            assert!(mount.mounted_at.is_none());
        }
    }

    #[tokio::test]
    async fn test_duplicate_mount_id() {
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);

        manager.mount_webdav(18083).unwrap();
        let result = manager.mount_webdav(18083);
        assert!(matches!(result, Err(MountError::AlreadyMounted)));
    }
}
