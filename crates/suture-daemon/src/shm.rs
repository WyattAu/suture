use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use memmap2::MmapMut;

pub const SHM_MAGIC: u64 = 0x5755544D;
pub const SHM_VERSION: u32 = 1;
const SHM_SIZE: usize = 176;

const PID_FILE: &str = "/tmp/suture-daemon.pid";

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ShmStatus {
    pub magic: u64,
    pub version: u32,
    pub repo_count: u32,
    pub total_patches: u32,
    pub total_blobs: u32,
    pub head_branch: [u8; 64],
    pub is_mounted: u32,
    pub last_commit_ts: u64,
    pub last_sync_ts: u64,
    pub pid: u32,
    pub padding: [u64; 7],
}

const _: () = assert!(std::mem::size_of::<ShmStatus>() == SHM_SIZE);

impl ShmStatus {
    pub fn new(
        repo_count: u32,
        total_patches: u32,
        total_blobs: u32,
        head_branch: &str,
        pid: u32,
    ) -> Self {
        let mut status = Self {
            magic: SHM_MAGIC,
            version: SHM_VERSION,
            repo_count,
            total_patches,
            total_blobs,
            head_branch: [0u8; 64],
            is_mounted: 0,
            last_commit_ts: 0,
            last_sync_ts: 0,
            pid,
            padding: [0; 7],
        };
        let branch_bytes = head_branch.as_bytes();
        let len = branch_bytes.len().min(63);
        status.head_branch[..len].copy_from_slice(&branch_bytes[..len]);
        status
    }

    pub fn head_branch_str(&self) -> &str {
        let end = self.head_branch.iter().position(|&b| b == 0).unwrap_or(64);
        std::str::from_utf8(&self.head_branch[..end]).unwrap_or("")
    }
}

unsafe impl Send for ShmStatus {}
unsafe impl Sync for ShmStatus {}

pub fn shm_path_for_pid(pid: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/suture-shm-{pid}"))
}

pub fn create_shm_segment(
    repo_count: u32,
    total_patches: u32,
    total_blobs: u32,
    head_branch: &str,
    pid: u32,
) -> Result<PathBuf, anyhow::Error> {
    let path = shm_path_for_pid(pid);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;

    file.set_len(SHM_SIZE as u64)?;
    file.write_all(&[0u8; SHM_SIZE])?;
    file.flush()?;

    let mut mmap = unsafe { MmapMut::map_mut(&file)? };
    let status = ShmStatus::new(repo_count, total_patches, total_blobs, head_branch, pid);
    let bytes = unsafe {
        std::slice::from_raw_parts(
            &status as *const ShmStatus as *const u8,
            std::mem::size_of::<ShmStatus>(),
        )
    };
    mmap[..bytes.len()].copy_from_slice(bytes);
    mmap.flush()?;

    Ok(path)
}

pub fn read_shm_status(path: &Path) -> Result<ShmStatus, anyhow::Error> {
    let file = File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };

    let status: ShmStatus = unsafe { std::ptr::read(mmap.as_ptr() as *const ShmStatus) };

    if status.magic != SHM_MAGIC {
        anyhow::bail!(
            "invalid SHM magic: expected {SHM_MAGIC:#x}, got {:#x}",
            status.magic
        );
    }

    Ok(status)
}

pub fn update_shm_status(path: &Path, status: &ShmStatus) -> Result<(), anyhow::Error> {
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    let mut mmap = unsafe { MmapMut::map_mut(&file)? };

    let bytes = unsafe {
        std::slice::from_raw_parts(
            status as *const ShmStatus as *const u8,
            std::mem::size_of::<ShmStatus>(),
        )
    };
    mmap[..bytes.len()].copy_from_slice(bytes);
    mmap.flush()?;

    Ok(())
}

pub fn cleanup_shm(path: &Path) -> Result<(), anyhow::Error> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn write_pid_file(pid: u32) -> Result<(), anyhow::Error> {
    let path = Path::new(PID_FILE);
    let mut file = File::create(path)?;
    file.write_all(pid.to_string().as_bytes())?;
    file.flush()?;
    Ok(())
}

pub fn read_pid_file() -> Result<u32, anyhow::Error> {
    let path = Path::new(PID_FILE);
    if !path.exists() {
        anyhow::bail!("PID file not found: daemon may not be running");
    }
    let mut contents = String::new();
    File::open(path)?.read_to_string(&mut contents)?;
    let pid: u32 = contents.trim().parse()?;
    Ok(pid)
}

pub fn remove_pid_file() -> Result<(), anyhow::Error> {
    let path = Path::new(PID_FILE);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize SHM tests since they all share the same file path
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn test_shm_path() -> PathBuf {
        shm_path_for_pid(std::process::id())
    }

    fn cleanup_test_shm() {
        let path = test_shm_path();
        let _ = cleanup_shm(&path);
    }

    #[test]
    fn test_shm_round_trip() {
        let _guard = TEST_LOCK.lock().unwrap();
        cleanup_test_shm();

        let path = create_shm_segment(3, 42, 1000, "main", std::process::id())
            .expect("create should succeed");
        let status = read_shm_status(&path).expect("read should succeed");

        assert_eq!(status.magic, SHM_MAGIC);
        assert_eq!(status.version, SHM_VERSION);
        assert_eq!(status.repo_count, 3);
        assert_eq!(status.total_patches, 42);
        assert_eq!(status.total_blobs, 1000);
        assert_eq!(status.head_branch_str(), "main");
        assert_eq!(status.pid, std::process::id());

        cleanup_shm(&path).expect("cleanup should succeed");
    }

    #[test]
    fn test_shm_magic() {
        let _guard = TEST_LOCK.lock().unwrap();
        cleanup_test_shm();

        let path =
            create_shm_segment(1, 1, 1, "test", std::process::id()).expect("create should succeed");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open should succeed");
        let mut mmap = unsafe { MmapMut::map_mut(&file).expect("mmap should succeed") };
        mmap[..8].copy_from_slice(&0xDEADBEEFu64.to_le_bytes());
        mmap.flush().expect("flush should succeed");
        drop(mmap);
        drop(file);

        let result = read_shm_status(&path);
        assert!(result.is_err(), "should reject invalid magic");

        cleanup_shm(&path).expect("cleanup should succeed");
    }

    #[test]
    fn test_shm_cleanup() {
        let _guard = TEST_LOCK.lock().unwrap();
        cleanup_test_shm();

        let path =
            create_shm_segment(1, 1, 1, "test", std::process::id()).expect("create should succeed");
        assert!(path.exists(), "SHM file should exist after creation");

        cleanup_shm(&path).expect("cleanup should succeed");
        assert!(!path.exists(), "SHM file should be removed after cleanup");
    }

    #[test]
    fn test_shm_update() {
        let _guard = TEST_LOCK.lock().unwrap();
        cleanup_test_shm();

        let path = create_shm_segment(1, 10, 20, "develop", std::process::id())
            .expect("create should succeed");

        let mut status = read_shm_status(&path).expect("read should succeed");
        assert_eq!(status.total_patches, 10);

        status.total_patches = 99;
        status.is_mounted = 1;
        update_shm_status(&path, &status).expect("update should succeed");

        let updated = read_shm_status(&path).expect("read should succeed");
        assert_eq!(updated.total_patches, 99);
        assert_eq!(updated.is_mounted, 1);

        cleanup_shm(&path).expect("cleanup should succeed");
    }

    #[test]
    fn test_shm_long_branch_name_truncated() {
        let _guard = TEST_LOCK.lock().unwrap();
        cleanup_test_shm();

        let long_name = "a".repeat(128);
        let path = create_shm_segment(1, 1, 1, &long_name, std::process::id())
            .expect("create should succeed");

        let status = read_shm_status(&path).expect("read should succeed");
        assert_eq!(status.head_branch_str().len(), 63);
        assert!(status.head_branch_str().starts_with("a"));

        cleanup_shm(&path).expect("cleanup should succeed");
    }

    #[test]
    fn test_pid_file_round_trip() {
        let _guard = TEST_LOCK.lock().unwrap();
        let orig_pid = Path::new(PID_FILE);
        let orig_exists = orig_pid.exists();
        if orig_exists {
            let bak = Path::new("/tmp/suture-daemon.pid.bak");
            let _ = std::fs::copy(orig_pid, bak);
            let _ = std::fs::remove_file(orig_pid);
        }

        write_pid_file(42424).expect("write should succeed");
        let read = read_pid_file().expect("read should succeed");
        assert_eq!(read, 42424);

        remove_pid_file().expect("remove should succeed");
        assert!(!Path::new(PID_FILE).exists());

        if orig_exists {
            let bak = Path::new("/tmp/suture-daemon.pid.bak");
            let _ = std::fs::copy(bak, orig_pid);
            let _ = std::fs::remove_file(bak);
        }
    }
}
