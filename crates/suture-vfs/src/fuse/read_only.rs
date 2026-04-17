use crate::fuse::inode::{InodeEntry, InodeGenerator, InodeKind};
use crate::path_translation::PathTranslator;
use async_stream::try_stream;
use fuse3::raw::prelude::*;
use fuse3::raw::reply::{
    DirectoryEntry, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, ReplyInit, ReplyOpen,
    ReplyStatFs,
};
use fuse3::raw::Request;
use fuse3::MountOptions;
use fuse3::{Errno, FileType, Timestamp};
use suture_core::repository::Repository;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const TTL: Duration = Duration::from_secs(3600);

fn now_timestamp() -> Timestamp {
    SystemTime::now().into()
}

struct InnerFs {
    inode_map: InodeGenerator,
    path_translator: PathTranslator,
    file_contents: HashMap<String, Vec<u8>>,
}

pub struct SutureFilesystem {
    inner: Arc<InnerFs>,
}

impl SutureFilesystem {
    pub async fn new(repo_path: &Path, branch: Option<&str>) -> anyhow::Result<Self> {
        let mut repo = Repository::open(repo_path)
            .map_err(|e| anyhow::anyhow!("failed to open repository: {e}"))?;

        if let Some(branch_name) = branch {
            repo.checkout(branch_name)?;
        }

        let file_tree = repo.snapshot_head()?;

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
            inode_map,
            path_translator,
            file_contents,
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }
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

impl Filesystem for SutureFilesystem {
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

        let parent_path = self.inner.inode_map.get_path(parent);
        let parent_path = parent_path.ok_or_else(Errno::new_not_exist)?;

        let child_path = if parent_path.is_empty() {
            name_str.to_string()
        } else {
            format!("{}/{}", parent_path, name_str)
        };

        let inode = self
            .inner
            .inode_map
            .lookup(&child_path)
            .ok_or_else(Errno::new_not_exist)?;

        let entry = self.inner.inode_map.get(inode).unwrap();
        let size = if entry.kind == InodeKind::File {
            self.inner
                .file_contents
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
        let entry = self.inner.inode_map.get(inode).ok_or_else(Errno::new_not_exist)?;

        let size = if entry.kind == InodeKind::File {
            self.inner
                .file_contents
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
        let entry = self.inner.inode_map.get(inode).ok_or_else(Errno::new_not_exist)?;

        if entry.kind != InodeKind::File {
            return Err(Errno::new_is_dir());
        }

        let data = self
            .inner
            .file_contents
            .get(&entry.path)
            .cloned()
            .unwrap_or_default();

        let offset = offset as usize;
        let end = std::cmp::min(offset + size as usize, data.len());

        if offset >= data.len() {
            return Ok(ReplyData { data: vec![].into() });
        }

        Ok(ReplyData {
            data: data[offset..end].to_vec().into(),
        })
    }

    async fn readdir<'a>(
        &'a self,
        _req: Request,
        parent: u64,
        _fh: u64,
        offset: i64,
    ) -> fuse3::Result<ReplyDirectory<impl futures_core::Stream<Item = fuse3::Result<DirectoryEntry>> + Send + 'a>> {
        let parent_path = self.inner.inode_map.get_path(parent);
        let parent_path = match parent_path {
            Some(p) => p.to_string(),
            None => return Err(Errno::new_not_exist()),
        };

        let entries = self.inner.path_translator.list_dir(&parent_path);

        let inode_map = &self.inner.inode_map;
        let file_contents = &self.inner.file_contents;

        let stream = try_stream! {
            for (i, dir_entry) in entries.iter().enumerate() {
                let entry_offset = (i + 3) as i64;
                if entry_offset <= offset {
                    continue;
                }

                let inode = inode_map
                    .lookup(&dir_entry.path)
                    .ok_or_else(Errno::new_not_exist)?;

                let entry = inode_map.get(inode).unwrap();
                let size = if dir_entry.is_dir {
                    0u64
                } else {
                    file_contents.get(&dir_entry.path).map(|d| d.len() as u64).unwrap_or(0)
                };
                let attr = make_file_attr(entry, inode, size);

                yield DirectoryEntry {
                    inode,
                    kind: attr.kind,
                    name: dir_entry.name.clone().into(),
                    offset: entry_offset,
                };
            }
        };

        Ok(ReplyDirectory {
            entries: Box::pin(stream),
        })
    }

    async fn statfs(
        &self,
        _req: Request,
        _inode: u64,
    ) -> fuse3::Result<ReplyStatFs> {
        Ok(ReplyStatFs {
            blocks: 1,
            bfree: 0,
            bavail: 0,
            files: self.inner.inode_map.len() as u64,
            ffree: 0,
            bsize: 4096,
            namelen: 255,
            frsize: 4096,
        })
    }

    async fn open(
        &self,
        _req: Request,
        _inode: u64,
        _flags: u32,
    ) -> fuse3::Result<ReplyOpen> {
        Ok(ReplyOpen {
            fh: 0,
            flags: 0,
        })
    }

    async fn opendir(
        &self,
        _req: Request,
        _inode: u64,
        _flags: u32,
    ) -> fuse3::Result<ReplyOpen> {
        Ok(ReplyOpen {
            fh: 0,
            flags: 0,
        })
    }
}

pub async fn mount(
    repo_path: &str,
    mountpoint: &Path,
    branch: Option<&str>,
) -> Result<(), anyhow::Error> {
    tracing::info!(
        "mounting suture repo {} at {} (branch: {:?})",
        repo_path,
        mountpoint.display(),
        branch
    );

    let repo = Path::new(repo_path);
    let fs = SutureFilesystem::new(repo, branch).await?;

    tokio::fs::create_dir_all(mountpoint).await?;

    let mount_options = MountOptions::default();
    let mountpoint = mountpoint.to_path_buf();

    let _session = fuse3::raw::Session::new(mount_options)
        .mount(fs, &mountpoint)
        .await
        .map_err(|e| anyhow::anyhow!("FUSE mount failed: {e}"))?;

    tracing::info!("FUSE mount ready at {}", mountpoint.display());
    _session.unmount().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_filesystem_init() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let _ = Repository::init(repo_path, "test");

        let result = SutureFilesystem::new(repo_path, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_filesystem_inode_tree() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let mut repo = Repository::init(repo_path, "test").unwrap();

        let file1 = repo_path.join("hello.txt");
        std::fs::write(&file1, b"hello world").unwrap();
        repo.add("hello.txt").unwrap();

        let subdir = repo_path.join("sub");
        std::fs::create_dir_all(&subdir).unwrap();
        let file2 = repo_path.join("sub/nested.rs");
        std::fs::write(&file2, b"fn main() {}").unwrap();
        repo.add("sub/nested.rs").unwrap();

        repo.commit("initial").unwrap();

        let fs = SutureFilesystem::new(repo_path, None).await.unwrap();

        let root = fs.inner.inode_map.root_inode().unwrap();
        assert_eq!(root, 1);
        assert_eq!(fs.inner.inode_map.get(root).unwrap().kind, InodeKind::Directory);

        let hello = fs.inner.inode_map.lookup("hello.txt").unwrap();
        assert_eq!(fs.inner.inode_map.get(hello).unwrap().kind, InodeKind::File);

        let sub = fs.inner.inode_map.lookup("sub").unwrap();
        assert_eq!(fs.inner.inode_map.get(sub).unwrap().kind, InodeKind::Directory);

        let nested = fs.inner.inode_map.lookup("sub/nested.rs").unwrap();
        assert_eq!(
            fs.inner.inode_map.get(nested).unwrap().kind,
            InodeKind::File
        );
    }

    #[tokio::test]
    async fn test_filesystem_read_content() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let mut repo = Repository::init(repo_path, "test").unwrap();

        let content = b"The quick brown fox jumps over the lazy dog.";
        let file = repo_path.join("fox.txt");
        std::fs::write(&file, content).unwrap();
        repo.add("fox.txt").unwrap();
        repo.commit("add fox").unwrap();

        let fs = SutureFilesystem::new(repo_path, None).await.unwrap();
        let _inode = fs.inner.inode_map.lookup("fox.txt").unwrap();

        let contents = &fs.inner.file_contents;
        let data = contents.get("fox.txt").unwrap();
        assert_eq!(data.as_slice(), content);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_readonly_mount() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "test").unwrap();

        std::fs::write(repo_path.join("hello.txt"), b"hello").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("initial").unwrap();

        let mount_dir = tempfile::tempdir().unwrap();
        let mountpoint = mount_dir.path().join("mnt");

        let mountpoint_for_mount = mountpoint.clone();
        let mountpoint_for_read = mountpoint.clone();
        let handle = tokio::spawn(async move {
            mount(
                repo_path.to_str().unwrap(),
                &mountpoint_for_mount,
                None,
            )
            .await
            .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let entries: Vec<_> = std::fs::read_dir(&mountpoint_for_read)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(entries.contains(&"hello.txt".to_string()));

        let content = std::fs::read_to_string(mountpoint.join("hello.txt")).unwrap();
        assert_eq!(content, "hello");

        handle.abort();
    }
}
