#![allow(clippy::arc_with_non_send_sync)]
// FUSE filesystems are inherently single-threaded per mount; the Arc is used for
// shared ownership across callback closures dispatched by the FUSE library.

use crate::UnpoisonMutex;
use crate::fuse::inode::{InodeEntry, InodeGenerator, InodeKind};
use crate::path_translation::PathTranslator;
use async_stream::try_stream;
use fuse3::raw::Request;
use fuse3::raw::prelude::*;
use fuse3::raw::reply::{
    DirectoryEntry, ReplyAttr, ReplyCreated, ReplyData, ReplyDirectory, ReplyEntry, ReplyInit,
    ReplyOpen, ReplyStatFs, ReplyWrite,
};
use fuse3::{Errno, FileType, MountOptions, SetAttr, Timestamp};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use suture_core::repository::Repository;

const TTL: Duration = Duration::from_secs(3600);

fn now_timestamp() -> Timestamp {
    SystemTime::now().into()
}

struct OpenFile {
    path: String,
    buffer: Vec<u8>,
    is_new: bool,
}

struct InnerFs {
    inode_map: Mutex<InodeGenerator>,
    path_translator: Mutex<PathTranslator>,
    file_contents: Mutex<HashMap<String, Vec<u8>>>,
    repo: Mutex<Repository>,
    open_files: Mutex<HashMap<u64, OpenFile>>,
    next_fh: AtomicU64,
    repo_path: std::path::PathBuf,
    pending_dirs: Mutex<HashSet<String>>,
    pending_files: Mutex<HashSet<String>>,
}

pub struct RwFilesystem {
    inner: Arc<InnerFs>,
}

impl RwFilesystem {
    pub async fn new(repo_path: &Path, branch: Option<&str>) -> anyhow::Result<Self> {
        let mut repo = Repository::open(repo_path)
            .map_err(|e| anyhow::anyhow!("failed to open repository: {e}"))?;

        if let Some(branch_name) = branch {
            repo.checkout(branch_name)?;
        }

        let file_tree = repo
            .snapshot_head()
            .map_err(|e| anyhow::anyhow!("snapshot failed: {e}"))?;

        let paths: Vec<String> = file_tree.paths().into_iter().cloned().collect();
        let paths_ref: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

        let path_translator = PathTranslator::build(&paths_ref);

        let mut inode_map = InodeGenerator::new();
        inode_map.alloc_dir("");
        for dir in path_translator.all_dirs() {
            if !dir.is_empty() {
                inode_map.alloc_dir(dir);
            }
        }
        for path in path_translator.all_files().keys() {
            inode_map.alloc_file(path);
        }

        let mut file_contents = HashMap::new();
        for (path, hash) in file_tree.iter() {
            match repo.cas().get_blob(hash) {
                Ok(data) => {
                    file_contents.insert(path.clone(), data);
                }
                Err(e) => {
                    tracing::warn!("failed to read blob for {}: {}", path, e);
                }
            }
        }

        let inner = InnerFs {
            inode_map: Mutex::new(inode_map),
            path_translator: Mutex::new(path_translator),
            file_contents: Mutex::new(file_contents),
            repo: Mutex::new(repo),
            open_files: Mutex::new(HashMap::new()),
            next_fh: AtomicU64::new(1),
            repo_path: repo_path.to_path_buf(),
            pending_dirs: Mutex::new(HashSet::new()),
            pending_files: Mutex::new(HashSet::new()),
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    fn alloc_fh(&self) -> u64 {
        self.inner.next_fh.fetch_add(1, Ordering::Relaxed)
    }

    fn resolve_path(&self, parent: u64, name: &str) -> Result<String, Errno> {
        let inode_map = self.inner.inode_map.unpoison_lock();
        let parent_path = inode_map
            .get_path(parent)
            .ok_or_else(Errno::new_not_exist)?;
        Ok(if parent_path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", parent_path, name)
        })
    }

    fn ensure_parent_dirs_in_inode_map(&self, path: &str) {
        let mut inode_map = self.inner.inode_map.unpoison_lock();
        let parts: Vec<&str> = path.split('/').collect();
        let mut prefix = String::new();
        for part in parts.iter().take(parts.len() - 1) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            inode_map.alloc_dir(&prefix);
        }
    }

    fn rebuild_path_translator(&self) {
        let file_contents = self.inner.file_contents.unpoison_lock();
        let pending_dirs = self.inner.pending_dirs.unpoison_lock();

        let mut all_paths: Vec<String> = file_contents.keys().cloned().collect();

        for dir in pending_dirs.iter() {
            let marker = format!("{}.d", dir);
            all_paths.push(marker);
        }

        let all_paths_ref: Vec<&str> = all_paths.iter().map(|s| s.as_str()).collect();
        let path_translator = PathTranslator::build(&all_paths_ref);

        *self.inner.path_translator.unpoison_lock() = path_translator;
    }

    fn commit_file_change(&self, path: &str, content: &[u8], is_new: bool) -> anyhow::Result<()> {
        let full_path = self.inner.repo_path.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, content)?;

        let mut repo = self.inner.repo.unpoison_lock();
        repo.add(path).map_err(|e| anyhow::anyhow!("stage: {e}"))?;

        let msg = if is_new {
            format!("vfs: create {}", path)
        } else {
            format!("vfs: modify {}", path)
        };
        repo.commit(&msg)
            .map_err(|e| anyhow::anyhow!("commit: {e}"))?;

        self.inner.pending_files.unpoison_lock().remove(path);
        self.rebuild_from_repo(&repo)?;
        Ok(())
    }

    fn commit_file_delete(&self, path: &str) -> anyhow::Result<()> {
        let full_path = self.inner.repo_path.join(path);
        if full_path.exists() {
            std::fs::remove_file(&full_path)?;
        }

        let mut repo = self.inner.repo.unpoison_lock();
        repo.add(path)
            .map_err(|e| anyhow::anyhow!("stage delete: {e}"))?;
        repo.commit(&format!("vfs: delete {}", path))
            .map_err(|e| anyhow::anyhow!("commit delete: {e}"))?;

        self.rebuild_from_repo(&repo)?;
        Ok(())
    }

    fn rebuild_from_repo(&self, repo: &Repository) -> anyhow::Result<()> {
        let file_tree = repo
            .snapshot_head()
            .map_err(|e| anyhow::anyhow!("snapshot: {e}"))?;

        let pending_files = self.inner.pending_files.unpoison_lock();
        let mut file_contents = self.inner.file_contents.unpoison_lock();

        for (path, hash) in file_tree.iter() {
            if let Ok(data) = repo.cas().get_blob(hash) {
                file_contents.insert(path.clone(), data);
            }
        }

        file_contents.retain(|path, _| file_tree.contains(path) || pending_files.contains(path));

        drop(pending_files);

        let pending_dirs = self.inner.pending_dirs.unpoison_lock();
        let mut all_paths: Vec<String> = file_contents.keys().cloned().collect();
        for dir in pending_dirs.iter() {
            all_paths.push(format!("{}.d", dir));
        }
        let all_paths_ref: Vec<&str> = all_paths.iter().map(|s| s.as_str()).collect();
        let path_translator = PathTranslator::build(&all_paths_ref);

        let mut inode_map = InodeGenerator::new();
        inode_map.alloc_dir("");
        for dir in path_translator.all_dirs() {
            let clean = dir.strip_suffix(".d").unwrap_or(dir);
            if !clean.is_empty() {
                inode_map.alloc_dir(clean);
            }
        }
        for path in path_translator.all_files().keys() {
            let clean = path.strip_suffix(".d").unwrap_or(path);
            if !clean.ends_with(".d") {
                inode_map.alloc_file(clean);
            }
        }
        for dir in pending_dirs.iter() {
            inode_map.alloc_dir(dir);
        }

        *self.inner.inode_map.unpoison_lock() = inode_map;
        *self.inner.path_translator.unpoison_lock() = path_translator;

        Ok(())
    }

    fn list_dir_entries(&self, dir_path: &str) -> Vec<(String, String, bool)> {
        let path_translator = self.inner.path_translator.unpoison_lock();
        let pending_dirs = self.inner.pending_dirs.unpoison_lock();

        let mut entries: Vec<(String, String, bool)> = path_translator
            .list_dir(dir_path)
            .into_iter()
            .filter(|e| !e.path.ends_with(".d"))
            .map(|e| (e.name, e.path, e.is_dir))
            .collect();

        for dir in pending_dirs.iter() {
            let parent = parent_of(dir).unwrap_or_default();
            if parent == dir_path {
                let name = dir.rsplit('/').next().unwrap_or(dir).to_string();
                if !entries.iter().any(|(n, _, _)| n == &name) {
                    entries.push((name, dir.clone(), true));
                }
            }
        }

        entries.sort_by(|a, b| match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.cmp(&b.0),
        });
        entries
    }

    fn is_dir_empty(&self, dir_path: &str) -> bool {
        let entries = self.list_dir_entries(dir_path);
        entries.is_empty()
    }

    fn process_close(&self, fh: u64) -> anyhow::Result<()> {
        let open_file = {
            let mut open_files = self.inner.open_files.unpoison_lock();
            open_files.remove(&fh)
        };

        if let Some(open_file) = open_file {
            let path = open_file.path;
            let content = open_file.buffer;
            let is_new = open_file.is_new;

            let file_contents = self.inner.file_contents.unpoison_lock();
            let existing = file_contents.get(&path).cloned();
            drop(file_contents);

            let changed = match &existing {
                Some(old) => old != &content,
                None => true,
            };

            if changed {
                self.commit_file_change(&path, &content, is_new)?;
            } else if is_new {
                self.inner.pending_files.unpoison_lock().remove(&path);
                self.inner.file_contents.unpoison_lock().remove(&path);
                self.rebuild_path_translator();
            }
        }

        Ok(())
    }
}

fn parent_of(path: &str) -> Option<String> {
    let pos = path.rfind('/')?;
    Some(path[..pos].to_string())
}

fn make_file_attr(entry: &InodeEntry, inode: u64, size: u64) -> FileAttr {
    let (kind, perm) = match entry.kind {
        InodeKind::Directory => (FileType::Directory, 0o755),
        InodeKind::File => (FileType::RegularFile, 0o644),
    };
    FileAttr {
        ino: inode,
        size,
        blocks: size.div_ceil(512),
        atime: now_timestamp(),
        mtime: now_timestamp(),
        ctime: now_timestamp(),
        kind,
        perm,
        nlink: 1,
        uid: unsafe { libc::getuid() },
        gid: unsafe { libc::getgid() },
        rdev: 0,
        blksize: 4096,
    }
}

impl Filesystem for RwFilesystem {
    async fn init(&self, _req: Request) -> fuse3::Result<ReplyInit> {
        Ok(ReplyInit {
            max_write: std::num::NonZeroU32::new(4 * 1024 * 1024).unwrap(),
        })
    }

    async fn destroy(&self, _req: Request) {}

    async fn lookup(
        &self,
        _req: Request,
        parent: u64,
        name: &std::ffi::OsStr,
    ) -> fuse3::Result<ReplyEntry> {
        let name_str = name.to_str().ok_or_else(Errno::new_not_exist)?;
        let child_path = self.resolve_path(parent, name_str)?;

        let inode_map = self.inner.inode_map.unpoison_lock();
        let inode = inode_map
            .lookup(&child_path)
            .ok_or_else(Errno::new_not_exist)?;
        let entry = inode_map.get(inode).unwrap();

        let size = if entry.kind == InodeKind::File {
            let file_contents = self.inner.file_contents.unpoison_lock();
            file_contents
                .get(&child_path)
                .map(|d| d.len() as u64)
                .unwrap_or(0)
        } else {
            0
        };

        let attr = make_file_attr(entry, inode, size);
        Ok(ReplyEntry {
            ttl: TTL,
            attr,
            generation: 0,
        })
    }

    async fn getattr(
        &self,
        _req: Request,
        inode: u64,
        _fh: Option<u64>,
        _flags: u32,
    ) -> fuse3::Result<ReplyAttr> {
        let inode_map = self.inner.inode_map.unpoison_lock();
        let entry = inode_map.get(inode).ok_or_else(Errno::new_not_exist)?;

        let size = if entry.kind == InodeKind::File {
            let open_files = self.inner.open_files.unpoison_lock();
            let open_data = open_files
                .values()
                .find(|f| f.path == entry.path)
                .map(|f| f.buffer.len() as u64);
            drop(open_files);

            open_data.unwrap_or_else(|| {
                let file_contents = self.inner.file_contents.unpoison_lock();
                file_contents
                    .get(&entry.path)
                    .map(|d| d.len() as u64)
                    .unwrap_or(0)
            })
        } else {
            0
        };

        let attr = make_file_attr(entry, inode, size);
        Ok(ReplyAttr { ttl: TTL, attr })
    }

    async fn setattr(
        &self,
        _req: Request,
        inode: u64,
        _fh: Option<u64>,
        _set_attr: SetAttr,
    ) -> fuse3::Result<ReplyAttr> {
        let inode_map = self.inner.inode_map.unpoison_lock();
        let entry = inode_map.get(inode).ok_or_else(Errno::new_not_exist)?;

        let size = if entry.kind == InodeKind::File {
            let file_contents = self.inner.file_contents.unpoison_lock();
            file_contents
                .get(&entry.path)
                .map(|d| d.len() as u64)
                .unwrap_or(0)
        } else {
            0
        };

        let attr = make_file_attr(entry, inode, size);
        Ok(ReplyAttr { ttl: TTL, attr })
    }

    async fn read(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        size: u32,
    ) -> fuse3::Result<ReplyData> {
        let inode_map = self.inner.inode_map.unpoison_lock();
        let entry = inode_map.get(inode).ok_or_else(Errno::new_not_exist)?;
        if entry.kind != InodeKind::File {
            return Err(Errno::new_is_dir());
        }
        let path = entry.path.clone();
        drop(inode_map);

        let open_files = self.inner.open_files.unpoison_lock();
        let data = open_files
            .values()
            .find(|f| f.path == path)
            .map(|f| f.buffer.clone());
        drop(open_files);

        let data = match data {
            Some(buf) => buf,
            None => {
                let file_contents = self.inner.file_contents.unpoison_lock();
                file_contents.get(&path).cloned().unwrap_or_default()
            }
        };

        let offset = offset as usize;
        let end = std::cmp::min(offset + size as usize, data.len());
        if offset >= data.len() {
            return Ok(ReplyData {
                data: vec![].into(),
            });
        }

        Ok(ReplyData {
            data: data[offset..end].to_vec().into(),
        })
    }

    async fn write(
        &self,
        _req: Request,
        _inode: u64,
        fh: u64,
        offset: u64,
        data: &[u8],
        _write_flags: u32,
        _flags: u32,
    ) -> fuse3::Result<ReplyWrite> {
        let mut open_files = self.inner.open_files.unpoison_lock();
        let open_file = open_files
            .get_mut(&fh)
            .ok_or_else(|| Errno::from(libc::EBADF))?;

        let offset = offset as usize;
        let new_end = offset + data.len();
        if new_end > open_file.buffer.len() {
            open_file.buffer.resize(new_end, 0);
        }
        open_file.buffer[offset..new_end].copy_from_slice(data);

        Ok(ReplyWrite {
            written: data.len() as u32,
        })
    }

    async fn create(
        &self,
        _req: Request,
        parent: u64,
        name: &std::ffi::OsStr,
        _mode: u32,
        _flags: u32,
    ) -> fuse3::Result<ReplyCreated> {
        let name_str = name.to_str().ok_or_else(Errno::new_not_exist)?;
        let path = self.resolve_path(parent, name_str)?;

        let mut is_new;
        {
            let pending_files = self.inner.pending_files.unpoison_lock();
            is_new = !pending_files.contains(&path);
            let file_contents = self.inner.file_contents.unpoison_lock();
            if !file_contents.contains_key(&path) {
                is_new = true;
            }
        }

        self.ensure_parent_dirs_in_inode_map(&path);

        let inode;
        {
            let mut inode_map = self.inner.inode_map.unpoison_lock();
            inode = inode_map.alloc_file(&path);
        }

        {
            let mut file_contents = self.inner.file_contents.unpoison_lock();
            file_contents.entry(path.clone()).or_default();
        }

        self.inner
            .pending_files
            .lock()
            .unwrap()
            .insert(path.clone());
        self.rebuild_path_translator();

        let fh = self.alloc_fh();
        let open_file = OpenFile {
            path: path.clone(),
            buffer: Vec::new(),
            is_new,
        };
        self.inner.open_files.unpoison_lock().insert(fh, open_file);

        let entry = InodeEntry {
            kind: InodeKind::File,
            path: path.clone(),
        };
        let attr = make_file_attr(&entry, inode, 0);

        Ok(ReplyCreated {
            ttl: TTL,
            attr,
            generation: 0,
            fh,
            flags: 0,
        })
    }

    async fn unlink(
        &self,
        _req: Request,
        parent: u64,
        name: &std::ffi::OsStr,
    ) -> fuse3::Result<()> {
        let name_str = name.to_str().ok_or_else(Errno::new_not_exist)?;
        let path = self.resolve_path(parent, name_str)?;

        let is_pending = self.inner.pending_files.unpoison_lock().contains(&path);

        {
            let mut open_files = self.inner.open_files.unpoison_lock();
            open_files.retain(|_, f| f.path != path);
        }

        if is_pending {
            self.inner.pending_files.unpoison_lock().remove(&path);
            self.inner.file_contents.unpoison_lock().remove(&path);
            self.rebuild_path_translator();
        } else {
            self.inner.file_contents.unpoison_lock().remove(&path);
            self.commit_file_delete(&path)
                .map_err(std::io::Error::other)?;
        }

        Ok(())
    }

    async fn mkdir(
        &self,
        _req: Request,
        parent: u64,
        name: &std::ffi::OsStr,
        _mode: u32,
        _umask: u32,
    ) -> fuse3::Result<ReplyEntry> {
        let name_str = name.to_str().ok_or_else(Errno::new_not_exist)?;
        let path = self.resolve_path(parent, name_str)?;

        {
            let mut inode_map = self.inner.inode_map.unpoison_lock();
            inode_map.alloc_dir(&path);
        }

        self.inner.pending_dirs.unpoison_lock().insert(path.clone());
        self.rebuild_path_translator();

        let entry = InodeEntry {
            kind: InodeKind::Directory,
            path: path.clone(),
        };
        let inode = self.inner.inode_map.unpoison_lock().lookup(&path).unwrap();
        let attr = make_file_attr(&entry, inode, 0);

        Ok(ReplyEntry {
            ttl: TTL,
            attr,
            generation: 0,
        })
    }

    async fn rmdir(&self, _req: Request, parent: u64, name: &std::ffi::OsStr) -> fuse3::Result<()> {
        let name_str = name.to_str().ok_or_else(Errno::new_not_exist)?;
        let path = self.resolve_path(parent, name_str)?;

        if !self.is_dir_empty(&path) {
            return Err(Errno::from(libc::ENOTEMPTY));
        }

        self.inner.pending_dirs.unpoison_lock().remove(&path);
        self.rebuild_path_translator();

        Ok(())
    }

    async fn readdir<'a>(
        &'a self,
        _req: Request,
        parent: u64,
        _fh: u64,
        offset: i64,
    ) -> fuse3::Result<
        ReplyDirectory<impl futures_core::Stream<Item = fuse3::Result<DirectoryEntry>> + Send + 'a>,
    > {
        let inode_map = self.inner.inode_map.unpoison_lock();
        let parent_path = inode_map
            .get_path(parent)
            .ok_or_else(Errno::new_not_exist)?
            .to_string();
        drop(inode_map);

        let entries = self.list_dir_entries(&parent_path);

        let precomputed: Vec<(u64, FileAttr, std::ffi::OsString, i64)> = {
            let inode_map = self.inner.inode_map.unpoison_lock();
            let file_contents = self.inner.file_contents.unpoison_lock();

            let mut result = Vec::new();
            for (i, (name, path, is_dir)) in entries.iter().enumerate() {
                let entry_offset = (i + 3) as i64;
                if entry_offset <= offset {
                    continue;
                }

                if let Some(inode) = inode_map.lookup(path)
                    && let Some(entry) = inode_map.get(inode)
                {
                    let size = if *is_dir {
                        0u64
                    } else {
                        file_contents.get(path).map(|d| d.len() as u64).unwrap_or(0)
                    };
                    let attr = make_file_attr(entry, inode, size);
                    result.push((inode, attr, name.clone().into(), entry_offset));
                }
            }
            result
        };

        let stream = try_stream! {
            for (inode, attr, name, entry_offset) in precomputed {
                yield DirectoryEntry {
                    inode,
                    kind: attr.kind,
                    name,
                    offset: entry_offset,
                };
            }
        };

        Ok(ReplyDirectory {
            entries: Box::pin(stream),
        })
    }

    async fn statfs(&self, _req: Request, _inode: u64) -> fuse3::Result<ReplyStatFs> {
        Ok(ReplyStatFs {
            blocks: 1,
            bfree: 0,
            bavail: 0,
            files: self.inner.inode_map.unpoison_lock().len() as u64,
            ffree: 0,
            bsize: 4096,
            namelen: 255,
            frsize: 4096,
        })
    }

    async fn open(&self, _req: Request, inode: u64, flags: u32) -> fuse3::Result<ReplyOpen> {
        let inode_map = self.inner.inode_map.unpoison_lock();
        let entry = inode_map.get(inode).ok_or_else(Errno::new_not_exist)?;
        if entry.kind != InodeKind::File {
            return Err(Errno::new_is_dir());
        }
        let path = entry.path.clone();
        drop(inode_map);

        let fh = self.alloc_fh();

        let access_mode = flags & libc::O_ACCMODE as u32;
        if access_mode == libc::O_WRONLY as u32 || access_mode == libc::O_RDWR as u32 {
            let is_truncate = (flags & libc::O_TRUNC as u32) != 0;
            let content = if is_truncate {
                Vec::new()
            } else {
                self.inner
                    .file_contents
                    .lock()
                    .unwrap()
                    .get(&path)
                    .cloned()
                    .unwrap_or_default()
            };
            let open_file = OpenFile {
                path,
                buffer: content,
                is_new: false,
            };
            self.inner.open_files.unpoison_lock().insert(fh, open_file);
        }

        Ok(ReplyOpen { fh, flags: 0 })
    }

    async fn opendir(&self, _req: Request, _inode: u64, _flags: u32) -> fuse3::Result<ReplyOpen> {
        Ok(ReplyOpen { fh: 0, flags: 0 })
    }

    async fn flush(
        &self,
        _req: Request,
        _inode: u64,
        fh: u64,
        _lock_owner: u64,
    ) -> fuse3::Result<()> {
        self.process_close(fh).map_err(std::io::Error::other)?;
        Ok(())
    }

    async fn release(
        &self,
        _req: Request,
        _inode: u64,
        fh: u64,
        _flags: u32,
        _lock_owner: u64,
        _flush: bool,
    ) -> fuse3::Result<()> {
        if self.inner.open_files.unpoison_lock().contains_key(&fh) {
            self.process_close(fh).map_err(std::io::Error::other)?;
        }
        Ok(())
    }
}

unsafe impl Send for RwFilesystem {}
unsafe impl Sync for RwFilesystem {}

pub async fn mount_rw(
    repo_path: &str,
    mountpoint: &Path,
    branch: Option<&str>,
) -> Result<(), anyhow::Error> {
    tracing::info!(
        "mounting suture repo {} at {} (branch: {:?}, read-write)",
        repo_path,
        mountpoint.display(),
        branch
    );

    let repo = Path::new(repo_path);
    let fs = RwFilesystem::new(repo, branch).await?;

    tokio::fs::create_dir_all(mountpoint).await?;

    let mount_options = MountOptions::default();
    let mountpoint = mountpoint.to_path_buf();

    let _session = fuse3::raw::Session::new(mount_options)
        .mount(fs, &mountpoint)
        .await
        .map_err(|e| anyhow::anyhow!("FUSE mount failed: {e}"))?;

    tracing::info!("FUSE read-write mount ready at {}", mountpoint.display());
    _session.unmount().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let mut repo = Repository::init(repo_path, "test").unwrap();

        let file = repo_path.join("hello.txt");
        std::fs::write(&file, b"hello world").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("initial").unwrap();

        let fs = RwFilesystem::new(repo_path, None).await.unwrap();

        let _inode = fs
            .inner
            .inode_map
            .lock()
            .unwrap()
            .lookup("hello.txt")
            .unwrap();
        let fh = fs.alloc_fh();

        {
            let file_contents = fs.inner.file_contents.unpoison_lock();
            let content = file_contents.get("hello.txt").cloned().unwrap();
            drop(file_contents);

            let open_file = OpenFile {
                path: "hello.txt".to_string(),
                buffer: content,
                is_new: false,
            };
            fs.inner.open_files.unpoison_lock().insert(fh, open_file);
        }

        {
            let mut open_files = fs.inner.open_files.unpoison_lock();
            let open_file = open_files.get_mut(&fh).unwrap();
            open_file.buffer[6..11].copy_from_slice(b"WORLD");
        }

        let buffer = {
            let lock = fs.inner.open_files.unpoison_lock();
            lock.get(&fh).unwrap().buffer.clone()
        };
        assert_eq!(buffer.as_slice(), b"hello WORLD");
    }

    #[tokio::test]
    async fn test_patch_detection() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let mut repo = Repository::init(repo_path, "test").unwrap();

        let file = repo_path.join("hello.txt");
        std::fs::write(&file, b"hello world").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("initial").unwrap();

        let fs = RwFilesystem::new(repo_path, None).await.unwrap();

        let original_count = {
            let repo = fs.inner.repo.unpoison_lock();
            repo.log(None).unwrap().len()
        };

        let fh = fs.alloc_fh();
        {
            let file_contents = fs.inner.file_contents.unpoison_lock();
            let content = file_contents.get("hello.txt").cloned().unwrap();
            drop(file_contents);

            let open_file = OpenFile {
                path: "hello.txt".to_string(),
                buffer: content,
                is_new: false,
            };
            fs.inner.open_files.unpoison_lock().insert(fh, open_file);
        }

        fs.process_close(fh).unwrap();

        let new_count = {
            let repo = fs.inner.repo.unpoison_lock();
            repo.log(None).unwrap().len()
        };

        assert_eq!(
            new_count, original_count,
            "identical content should not create a patch"
        );
    }

    #[tokio::test]
    async fn test_inode_tracking() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let _ = Repository::init(repo_path, "test");

        let fs = RwFilesystem::new(repo_path, None).await.unwrap();

        let fh1 = fs.alloc_fh();
        let fh2 = fs.alloc_fh();
        let fh3 = fs.alloc_fh();

        assert_eq!(fh1, 1);
        assert_eq!(fh2, 2);
        assert_eq!(fh3, 3);

        {
            let mut open_files = fs.inner.open_files.unpoison_lock();
            open_files.insert(
                fh1,
                OpenFile {
                    path: "a.txt".to_string(),
                    buffer: b"a".to_vec(),
                    is_new: true,
                },
            );
            open_files.insert(
                fh2,
                OpenFile {
                    path: "b.txt".to_string(),
                    buffer: b"b".to_vec(),
                    is_new: true,
                },
            );
        }

        assert_eq!(fs.inner.open_files.unpoison_lock().len(), 2);

        fs.inner.open_files.unpoison_lock().remove(&fh1);
        assert_eq!(fs.inner.open_files.unpoison_lock().len(), 1);

        let remaining = {
            let lock = fs.inner.open_files.unpoison_lock();
            lock.get(&fh2).unwrap().path.clone()
        };
        assert_eq!(remaining, "b.txt");
    }

    #[tokio::test]
    async fn test_create_and_commit() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let mut repo = Repository::init(repo_path, "test").unwrap();

        let file = repo_path.join("existing.txt");
        std::fs::write(&file, b"original").unwrap();
        repo.add("existing.txt").unwrap();
        repo.commit("initial").unwrap();

        let fs = RwFilesystem::new(repo_path, None).await.unwrap();

        fs.commit_file_change("new_file.txt", b"new content", true)
            .unwrap();

        let repo = fs.inner.repo.unpoison_lock();
        let tree = repo.snapshot_head().unwrap();
        assert!(tree.contains("new_file.txt"));
        assert!(tree.contains("existing.txt"));

        let blob = repo
            .cas()
            .get_blob(tree.get("new_file.txt").unwrap())
            .unwrap();
        assert_eq!(blob.as_slice(), b"new content");
    }

    #[tokio::test]
    async fn test_modify_and_commit() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let mut repo = Repository::init(repo_path, "test").unwrap();

        let file = repo_path.join("hello.txt");
        std::fs::write(&file, b"hello").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("initial").unwrap();

        let fs = RwFilesystem::new(repo_path, None).await.unwrap();

        fs.commit_file_change("hello.txt", b"hello modified", false)
            .unwrap();

        let repo = fs.inner.repo.unpoison_lock();
        let blob = repo
            .cas()
            .get_blob(repo.snapshot_head().unwrap().get("hello.txt").unwrap())
            .unwrap();
        assert_eq!(blob.as_slice(), b"hello modified");
    }

    #[tokio::test]
    async fn test_delete_and_commit() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let mut repo = Repository::init(repo_path, "test").unwrap();

        let file = repo_path.join("hello.txt");
        std::fs::write(&file, b"hello").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("initial").unwrap();

        let fs = RwFilesystem::new(repo_path, None).await.unwrap();

        fs.commit_file_delete("hello.txt").unwrap();

        let repo = fs.inner.repo.unpoison_lock();
        let tree = repo.snapshot_head().unwrap();
        assert!(!tree.contains("hello.txt"));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_readwrite_mount() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "test").unwrap();

        std::fs::write(repo_path.join("hello.txt"), b"hello").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("initial").unwrap();

        let mount_dir = tempfile::tempdir().unwrap();
        let mountpoint = mount_dir.path().join("mnt");

        let mountpoint_for_mount = mountpoint.clone();
        let repo_path_for_mount = repo_path.clone();
        let handle = tokio::spawn(async move {
            mount_rw(
                repo_path_for_mount.to_str().unwrap(),
                &mountpoint_for_mount,
                None,
            )
            .await
            .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let content = std::fs::read_to_string(mountpoint.join("hello.txt")).unwrap();
        assert_eq!(content, "hello");

        std::fs::write(mountpoint.join("new_file.txt"), b"created via fuse").unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let new_content = std::fs::read_to_string(mountpoint.join("new_file.txt")).unwrap();
        assert_eq!(new_content, "created via fuse");

        let repo2 = Repository::open(&repo_path).unwrap();
        let tree = repo2.snapshot_head().unwrap();
        assert!(tree.contains("new_file.txt"));

        handle.abort();
    }
}
