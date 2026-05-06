use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeKind {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub struct InodeEntry {
    pub kind: InodeKind,
    pub path: String,
}

pub struct InodeGenerator {
    next: u64,
    entries: HashMap<u64, InodeEntry>,
    path_to_inode: HashMap<String, u64>,
}

impl InodeGenerator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next: 1,
            entries: HashMap::new(),
            path_to_inode: HashMap::new(),
        }
    }

    pub fn alloc_file(&mut self, path: &str) -> u64 {
        self.alloc(path, InodeKind::File)
    }

    pub fn alloc_dir(&mut self, path: &str) -> u64 {
        self.alloc(path, InodeKind::Directory)
    }

    fn alloc(&mut self, path: &str, kind: InodeKind) -> u64 {
        if let Some(&inode) = self.path_to_inode.get(path) {
            return inode;
        }
        let inode = self.next;
        self.next += 1;
        self.entries.insert(
            inode,
            InodeEntry {
                kind,
                path: path.to_owned(),
            },
        );
        self.path_to_inode.insert(path.to_owned(), inode);
        inode
    }

    #[must_use]
    pub fn lookup(&self, path: &str) -> Option<u64> {
        self.path_to_inode.get(path).copied()
    }

    #[must_use]
    pub fn get(&self, inode: u64) -> Option<&InodeEntry> {
        self.entries.get(&inode)
    }

    #[must_use]
    pub fn get_path(&self, inode: u64) -> Option<&str> {
        self.entries.get(&inode).map(|e| e.path.as_str())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn root_inode(&self) -> Option<u64> {
        self.path_to_inode.get("").copied()
    }
}

impl Default for InodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_allocation() {
        let mut allocator = InodeGenerator::new();
        let ino1 = allocator.alloc_file("README.md");
        let ino2 = allocator.alloc_file("src/main.rs");
        let ino3 = allocator.alloc_dir("src");

        assert_eq!(ino1, 1);
        assert_eq!(ino2, 2);
        assert_eq!(ino3, 3);
        assert!(ino1 != ino2);
        assert!(ino1 != ino3);
        assert!(ino2 != ino3);
    }

    #[test]
    fn test_inode_lookup() {
        let mut allocator = InodeGenerator::new();
        let ino = allocator.alloc_file("a.txt");
        assert_eq!(allocator.lookup("a.txt"), Some(ino));
        assert_eq!(allocator.lookup("nonexistent"), None);
    }

    #[test]
    fn test_inode_dedup() {
        let mut allocator = InodeGenerator::new();
        let ino1 = allocator.alloc_file("dup.txt");
        let ino2 = allocator.alloc_file("dup.txt");
        assert_eq!(ino1, ino2);
        assert_eq!(allocator.len(), 1);
    }

    #[test]
    fn test_inode_get() {
        let mut allocator = InodeGenerator::new();
        let ino = allocator.alloc_dir("foo");
        let entry = allocator.get(ino).unwrap();
        assert_eq!(entry.kind, InodeKind::Directory);
        assert_eq!(entry.path, "foo");
    }
}
