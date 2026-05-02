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
    Pending,
    Active,
    Error(String),
    Stopped,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
    repo_path: PathBuf,
    next_id: u64,
}

#[allow(dead_code)]
impl MountManager {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            mounts: HashMap::new(),
            webdav_handles: HashMap::new(),
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

        let id = self.generate_id();

        std::fs::create_dir_all(mount_path).map_err(|e| {
            MountError::MountFailed(format!("failed to create mount directory: {e}"))
        })?;

        let placeholder = mount_path.join(".suture_mount");
        std::fs::write(&placeholder, &id).map_err(|e| {
            MountError::MountFailed(format!("failed to create mount placeholder: {e}"))
        })?;

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

        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
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
                let placeholder = mount.mount_path.join(".suture_mount");
                let _ = std::fs::remove_file(&placeholder);
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

        for mount in self.mounts.values_mut() {
            if mount.mount_type == MountType::Fuse {
                let placeholder = mount.mount_path.join(".suture_mount");
                let _ = std::fs::remove_file(&placeholder);
            }
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
    async fn test_mount_fuse_create_and_stop() {
        let tmp = create_temp_dir();
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);
        let mount_path = tmp.path().join("fuse_mount");

        let id = manager.mount_fuse(&mount_path).unwrap();
        assert!(id.starts_with("mnt-"));

        let mount = manager.get_mount(&id).unwrap();
        assert_eq!(mount.mount_type, MountType::Fuse);
        assert_eq!(mount.status, MountStatus::Active);
        assert!(mount.mounted_at.is_some());
        assert!(mount.pid.is_some());
        assert!(mount_path.join(".suture_mount").exists());

        manager.unmount(&id).unwrap();
        assert_eq!(manager.status(&id).unwrap(), MountStatus::Stopped);
        assert!(!mount_path.join(".suture_mount").exists());
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
        let tmp = create_temp_dir();
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);

        let fuse_path = tmp.path().join("fuse");
        let id1 = manager.mount_fuse(&fuse_path).unwrap();
        let id2 = manager.mount_webdav(18081).unwrap();

        let mounts = manager.list_mounts();
        assert_eq!(mounts.len(), 2);
        assert!(mounts.iter().any(|m| m.id == id1));
        assert!(mounts.iter().any(|m| m.id == id2));

        manager.unmount(&id1).unwrap();
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
        let tmp = create_temp_dir();
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);

        let fuse_path1 = tmp.path().join("fuse1");
        let fuse_path2 = tmp.path().join("fuse2");
        manager.mount_fuse(&fuse_path1).unwrap();
        manager.mount_webdav(18082).unwrap();
        manager.mount_fuse(&fuse_path2).unwrap();

        assert_eq!(manager.list_mounts().len(), 3);

        manager.stop_all();

        for mount in manager.list_mounts() {
            assert_eq!(mount.status, MountStatus::Stopped);
            assert!(mount.mounted_at.is_none());
        }

        assert!(!fuse_path1.join(".suture_mount").exists());
        assert!(!fuse_path2.join(".suture_mount").exists());
    }

    #[tokio::test]
    async fn test_duplicate_mount_id() {
        let tmp = create_temp_dir();
        let repo_path = create_temp_dir().path().to_path_buf();
        let mut manager = MountManager::new(repo_path);

        let fuse_path = tmp.path().join("fuse_dup");
        manager.mount_fuse(&fuse_path).unwrap();
        let result = manager.mount_fuse(&fuse_path);
        assert!(matches!(result, Err(MountError::AlreadyMounted)));

        manager.mount_webdav(18083).unwrap();
        let result = manager.mount_webdav(18083);
        assert!(matches!(result, Err(MountError::AlreadyMounted)));
    }
}
