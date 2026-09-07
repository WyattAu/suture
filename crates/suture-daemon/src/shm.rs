use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use shm_rings::status::PodStatus;

pub const SHM_MAGIC: u64 = 0x5755544D;
pub const SHM_VERSION: u32 = 1;
const SHM_SIZE: usize = 176;

pub fn pid_file_path() -> PathBuf {
    std::env::temp_dir().join("suture-daemon.pid")
}

/// Status structure mapped directly into a file-backed memory region.
///
/// # Shared-memory layout contract
///
/// This struct is written by the daemon process and read by CLI clients
/// via a memory-mapped file. The following invariants must be maintained:
///
/// - `#[repr(C)]` guarantees a stable, predictable field order and
///   alignment that is identical across processes and compiler versions.
/// - The compile-time assertion below ensures `size_of::<ShmStatus>()`
///   exactly equals `SHM_SIZE` (176 bytes). Adding, removing, or
///   reordering fields without updating `SHM_SIZE` will cause a compile
///   error.
/// - All fields must be plain-old-data (no heap pointers, no `Drop` impls,
///   no interior mutability) so that a bitwise copy is always a valid
///   value, which is required for safe cross-process mapping.
/// - New fields must be appended at the end (before `padding`) to avoid
///   breaking the ABI for older readers.
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

// SAFETY: ShmStatus is trivially Send + Sync: every field is plain old data
// (u64, u32, fixed-size byte/integer arrays) with no interior mutability,
// no heap pointers, and no Drop impl. The compiler would auto-derive these
// traits for a struct of this shape; the unsafe impls are defensive to
// make the intent explicit. #[repr(C)] guarantees layout stability for
// cross-process shared-memory mapping but does not affect Send/Sync.
unsafe impl Send for ShmStatus {}
unsafe impl Sync for ShmStatus {}

/// Layout/identity contract for `shm_rings::status`: magic at offset 0,
/// version at offset 8 (the convention `ShmStatus` already follows), with
/// the suture-specific constants carried here rather than in the shared
/// crate.
impl PodStatus for ShmStatus {
    const MAGIC: u64 = SHM_MAGIC;
    const VERSION: u32 = SHM_VERSION;

    fn magic(&self) -> u64 {
        self.magic
    }

    fn version(&self) -> u32 {
        self.version
    }

    fn set_magic(&mut self) {
        self.magic = SHM_MAGIC;
    }
}

pub fn shm_path_for_pid(pid: u32) -> PathBuf {
    std::env::temp_dir().join(format!("suture-shm-{pid}"))
}

pub fn create_shm_segment(
    repo_count: u32,
    total_patches: u32,
    total_blobs: u32,
    head_branch: &str,
    pid: u32,
) -> Result<PathBuf, anyhow::Error> {
    let path = shm_path_for_pid(pid);
    shm_rings::status::create(
        &path,
        &ShmStatus::new(repo_count, total_patches, total_blobs, head_branch, pid),
    )?;
    Ok(path)
}

pub fn read_shm_status(path: &Path) -> Result<ShmStatus, anyhow::Error> {
    Ok(shm_rings::status::read(path)?)
}

pub fn update_shm_status(path: &Path, status: &ShmStatus) -> Result<(), anyhow::Error> {
    Ok(shm_rings::status::update(path, status)?)
}

pub fn cleanup_shm(path: &Path) -> Result<(), anyhow::Error> {
    Ok(shm_rings::status::cleanup(path)?)
}

pub fn write_pid_file(pid: u32) -> Result<(), anyhow::Error> {
    let path = pid_file_path();
    let mut file = File::create(path)?;
    file.write_all(pid.to_string().as_bytes())?;
    file.flush()?;
    Ok(())
}

pub fn read_pid_file() -> Result<u32, anyhow::Error> {
    let path = pid_file_path();
    if !path.exists() {
        anyhow::bail!("PID file not found: daemon may not be running");
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > 256 {
        anyhow::bail!("PID file too large ({} bytes)", metadata.len());
    }
    let mut contents = String::with_capacity(metadata.len() as usize);
    file.take(256).read_to_string(&mut contents)?;
    let pid: u32 = contents.trim().parse()?;
    Ok(pid)
}

pub fn remove_pid_file() -> Result<(), anyhow::Error> {
    let path = pid_file_path();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use memmap2::MmapMut;
    use std::fs::OpenOptions;

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
        // SAFETY: Same as create_shm_segment — file is created with
        // known size, aligned, and mapped as readable/writable.
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
        let orig_pid = pid_file_path();
        let orig_exists = orig_pid.exists();
        if orig_exists {
            let bak = std::env::temp_dir().join("suture-daemon.pid.bak");
            let _ = std::fs::copy(&orig_pid, &bak);
            let _ = std::fs::remove_file(&orig_pid);
        }

        write_pid_file(42424).expect("write should succeed");
        let read = read_pid_file().expect("read should succeed");
        assert_eq!(read, 42424);

        remove_pid_file().expect("remove should succeed");
        assert!(!pid_file_path().exists());

        if orig_exists {
            let bak = std::env::temp_dir().join("suture-daemon.pid.bak");
            let _ = std::fs::copy(&bak, &orig_pid);
            let _ = std::fs::remove_file(&bak);
        }
    }
}
