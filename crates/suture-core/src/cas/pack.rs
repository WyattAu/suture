//! Pack file support for the Content Addressable Storage.
//!
//! Pack files bundle multiple blobs into a single file, reducing
//! filesystem overhead for repositories with many small objects.

use crate::cas::compressor;
use crate::cas::hasher;
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use suture_common::Hash;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PackError {
    #[error("invalid pack magic: {0}")]
    InvalidMagic(String),
    #[error("unsupported pack version: {0}")]
    UnsupportedVersion(u32),
    #[error("invalid index magic: {0}")]
    InvalidIndexMagic(String),
    #[error("blob not found in pack: {0}")]
    BlobNotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("compression error: {0}")]
    CompressionError(String),
    #[error("decompression error: {0}")]
    DecompressionError(String),
    #[error("cannot create empty pack")]
    EmptyPack,
    #[error("unexpected object type: {0}")]
    UnexpectedObjectType(u8),
    #[error("hash mismatch in pack: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
}

const PACK_MAGIC: &[u8; 4] = b"SPCK";
const INDEX_MAGIC: &[u8; 4] = b"SIDX";
const PACK_VERSION: u32 = 1;
const TYPE_BLOB: u8 = 1;

#[derive(Clone, Debug)]
struct PackIndexEntry {
    hash: Hash,
    offset: u64,
}

#[derive(Clone, Debug)]
pub struct PackIndex {
    entries: Vec<PackIndexEntry>,
}

impl PackIndex {
    pub fn load(path: &std::path::Path) -> Result<Self, PackError> {
        let file = fs::File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != INDEX_MAGIC {
            return Err(PackError::InvalidIndexMagic(
                String::from_utf8_lossy(&magic).to_string(),
            ));
        }

        let mut version = [0u8; 4];
        reader.read_exact(&mut version)?;
        let version = u32::from_le_bytes(version);
        if version != PACK_VERSION {
            return Err(PackError::UnsupportedVersion(version));
        }

        let mut count = [0u8; 4];
        reader.read_exact(&mut count)?;
        let count = u32::from_le_bytes(count) as usize;

        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let mut hash_bytes = [0u8; 32];
            reader.read_exact(&mut hash_bytes)?;
            let mut offset_bytes = [0u8; 8];
            reader.read_exact(&mut offset_bytes)?;
            entries.push(PackIndexEntry {
                hash: Hash::from(hash_bytes),
                offset: u64::from_le_bytes(offset_bytes),
            });
        }

        entries.sort_by_key(|e| e.hash);

        Ok(Self { entries })
    }

    #[must_use]
    pub fn find(&self, hash: &Hash) -> Option<u64> {
        self.entries
            .binary_search_by_key(hash, |e| e.hash)
            .ok()
            .map(|idx| self.entries[idx].offset)
    }

    #[must_use]
    pub fn hashes(&self) -> Vec<Hash> {
        self.entries.iter().map(|e| e.hash).collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub struct PackFile;

impl PackFile {
    pub fn create(
        pack_dir: &std::path::Path,
        objects: &[(Hash, Vec<u8>)],
    ) -> Result<(PathBuf, PathBuf), PackError> {
        if objects.is_empty() {
            return Err(PackError::EmptyPack);
        }

        fs::create_dir_all(pack_dir)?;

        let mut pack_data = Vec::new();
        let mut index_entries = Vec::new();

        pack_data.extend_from_slice(PACK_MAGIC);
        pack_data.extend_from_slice(&PACK_VERSION.to_le_bytes());
        pack_data.extend_from_slice(&(objects.len() as u32).to_le_bytes());

        for (hash, data) in objects {
            let offset = pack_data.len() as u64;

            let compressed = compressor::compress(data, compressor::DEFAULT_COMPRESSION_LEVEL)
                .map_err(|e| PackError::CompressionError(e.to_string()))?;

            pack_data.push(TYPE_BLOB);
            pack_data.extend_from_slice(&(data.len() as u32).to_le_bytes());
            pack_data.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
            pack_data.extend_from_slice(&hash.0);
            pack_data.extend_from_slice(&compressed);

            index_entries.push(PackIndexEntry {
                hash: *hash,
                offset,
            });
        }

        let index_data = Self::serialize_index(&index_entries);
        let index_hash = hasher::hash_bytes(&index_data);
        let name = format!("pack-{}", index_hash.to_hex());

        let pack_path = pack_dir.join(format!("{name}.pack"));
        let idx_path = pack_dir.join(format!("{name}.idx"));

        fs::write(&pack_path, &pack_data)?;
        fs::write(&idx_path, &index_data)?;

        Ok((pack_path, idx_path))
    }

    fn serialize_index(entries: &[PackIndexEntry]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(INDEX_MAGIC);
        data.extend_from_slice(&PACK_VERSION.to_le_bytes());
        data.extend_from_slice(&(entries.len() as u32).to_le_bytes());

        for entry in entries {
            data.extend_from_slice(&entry.hash.0);
            data.extend_from_slice(&entry.offset.to_le_bytes());
        }

        data
    }

    pub fn read_blob(
        pack_path: &std::path::Path,
        index: &PackIndex,
        hash: &Hash,
    ) -> Result<Vec<u8>, PackError> {
        let offset = index
            .find(hash)
            .ok_or_else(|| PackError::BlobNotFound(hash.to_hex()))?;

        let file = fs::File::open(pack_path)?;
        let mut reader = BufReader::new(file);

        reader.seek(SeekFrom::Start(offset))?;

        let mut type_byte = [0u8; 1];
        reader.read_exact(&mut type_byte)?;
        if type_byte[0] != TYPE_BLOB {
            return Err(PackError::UnexpectedObjectType(type_byte[0]));
        }

        let mut uncomp_size = [0u8; 4];
        reader.read_exact(&mut uncomp_size)?;
        let _uncomp_size = u32::from_le_bytes(uncomp_size) as usize;

        let mut comp_size = [0u8; 4];
        reader.read_exact(&mut comp_size)?;
        let comp_size = u32::from_le_bytes(comp_size) as usize;

        let mut stored_hash = [0u8; 32];
        reader.read_exact(&mut stored_hash)?;

        let mut compressed = vec![0u8; comp_size];
        reader.read_exact(&mut compressed)?;

        let data = compressor::decompress(&compressed)
            .map_err(|e| PackError::DecompressionError(e.to_string()))?;

        let actual_hash = hasher::hash_bytes(&data);
        if actual_hash != *hash {
            return Err(PackError::HashMismatch {
                expected: hash.to_hex(),
                actual: actual_hash.to_hex(),
            });
        }

        Ok(data)
    }

    pub fn list_packs(pack_dir: &std::path::Path) -> io::Result<Vec<PathBuf>> {
        if !pack_dir.exists() {
            return Ok(Vec::new());
        }
        let mut packs = Vec::new();
        for entry in fs::read_dir(pack_dir)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str()
                && name.ends_with(".pack")
            {
                packs.push(entry.path());
            }
        }
        packs.sort();
        Ok(packs)
    }
}

/// Cache of loaded pack indices for efficient lookup.
#[derive(Debug)]
pub struct PackCache {
    indices: HashMap<PathBuf, PackIndex>,
}

// Public API; reserved for future pack operations
#[allow(dead_code)]
impl PackCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            indices: HashMap::new(),
        }
    }

    /// Load all pack indices from the pack directory.
    pub fn load_all(pack_dir: &std::path::Path) -> Result<Self, PackError> {
        let mut cache = Self::new();
        let pack_files = PackFile::list_packs(pack_dir)?;

        for pack_path in &pack_files {
            let idx_path = pack_path.with_extension("idx");
            if idx_path.exists() {
                let index = PackIndex::load(&idx_path)?;
                cache.indices.insert(pack_path.clone(), index);
            }
        }

        Ok(cache)
    }

    /// Find a hash across all loaded pack indices, returning the pack path and offset.
    #[must_use]
    pub fn find(&self, hash: &Hash) -> Option<(&PathBuf, u64)> {
        for (pack_path, index) in &self.indices {
            if let Some(offset) = index.find(hash) {
                return Some((pack_path, offset));
            }
        }
        None
    }

    /// List all hashes across all loaded pack indices.
    #[must_use]
    pub fn all_hashes(&self) -> Vec<Hash> {
        let mut hashes = Vec::new();
        for index in self.indices.values() {
            hashes.extend(index.hashes());
        }
        hashes.sort();
        hashes.dedup();
        hashes
    }

    /// Number of pack files loaded.
    #[must_use]
    pub fn pack_count(&self) -> usize {
        self.indices.len()
    }

    /// Total number of objects across all packs.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.indices.values().map(PackIndex::len).sum()
    }
}

impl Default for PackCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_test_objects() -> Vec<(Hash, Vec<u8>)> {
        vec![
            {
                let data = b"hello, world!".to_vec();
                let hash = hasher::hash_bytes(&data);
                (hash, data)
            },
            {
                let data = b"second blob content".to_vec();
                let hash = hasher::hash_bytes(&data);
                (hash, data)
            },
            {
                let data = vec![0u8; 1024];
                let hash = hasher::hash_bytes(&data);
                (hash, data)
            },
        ]
    }

    #[test]
    fn test_pack_create_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("pack");
        let objects = make_test_objects();

        let (pack_path, idx_path) = PackFile::create(&pack_dir, &objects).unwrap();
        assert!(pack_path.exists());
        assert!(idx_path.exists());
        assert!(pack_path.to_str().unwrap().ends_with(".pack"));
        assert!(idx_path.to_str().unwrap().ends_with(".idx"));

        let index = PackIndex::load(&idx_path).unwrap();
        assert_eq!(index.len(), 3);

        for (hash, data) in &objects {
            let retrieved = PackFile::read_blob(&pack_path, &index, hash).unwrap();
            assert_eq!(*data, retrieved);
        }
    }

    #[test]
    fn test_pack_index_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("pack");
        let objects = make_test_objects();

        let (_, idx_path) = PackFile::create(&pack_dir, &objects).unwrap();
        let index = PackIndex::load(&idx_path).unwrap();

        let hashes = index.hashes();
        let mut sorted = hashes.clone();
        sorted.sort();
        assert_eq!(hashes, sorted);
    }

    #[test]
    fn test_pack_index_find_missing() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("pack");
        let objects = make_test_objects();

        let (_, idx_path) = PackFile::create(&pack_dir, &objects).unwrap();
        let index = PackIndex::load(&idx_path).unwrap();

        let missing = Hash::from_hex(&"f".repeat(64)).unwrap();
        assert!(index.find(&missing).is_none());
    }

    #[test]
    fn test_pack_create_empty_fails() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("pack");
        let result = PackFile::create(&pack_dir, &[]);
        assert!(matches!(result, Err(PackError::EmptyPack)));
    }

    #[test]
    fn test_pack_list_packs() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("pack");

        assert_eq!(PackFile::list_packs(&pack_dir).unwrap().len(), 0);

        let objects = make_test_objects();
        PackFile::create(&pack_dir, &objects).unwrap();

        let packs = PackFile::list_packs(&pack_dir).unwrap();
        assert_eq!(packs.len(), 1);
        assert!(packs[0].to_str().unwrap().ends_with(".pack"));
    }

    #[test]
    fn test_pack_cache() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("pack");
        let objects = make_test_objects();

        PackFile::create(&pack_dir, &objects).unwrap();

        let cache = PackCache::load_all(&pack_dir).unwrap();
        assert_eq!(cache.pack_count(), 1);
        assert_eq!(cache.object_count(), 3);

        let all_hashes = cache.all_hashes();
        assert_eq!(all_hashes.len(), 3);

        for (hash, _data) in &objects {
            let (pack_path, offset) = cache.find(hash).unwrap();
            assert!(pack_path.exists());
            assert!(offset > 0);
        }
    }

    #[test]
    fn test_pack_cache_missing() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("pack");
        let objects = make_test_objects();

        PackFile::create(&pack_dir, &objects).unwrap();

        let cache = PackCache::load_all(&pack_dir).unwrap();
        let missing = Hash::from_hex(&"a".repeat(64)).unwrap();
        assert!(cache.find(&missing).is_none());
    }

    #[test]
    fn test_pack_invalid_magic() {
        let dir = tempfile::tempdir().unwrap();
        let bad_idx = dir.path().join("bad.idx");
        fs::write(&bad_idx, b"XXXX").unwrap();

        let result = PackIndex::load(&bad_idx);
        assert!(matches!(result, Err(PackError::InvalidIndexMagic(_))));
    }

    #[test]
    fn test_pack_single_object() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("pack");
        let data = b"single object".to_vec();
        let hash = hasher::hash_bytes(&data);

        let (pack_path, idx_path) = PackFile::create(&pack_dir, &[(hash, data.clone())]).unwrap();

        let index = PackIndex::load(&idx_path).unwrap();
        assert_eq!(index.len(), 1);

        let retrieved = PackFile::read_blob(&pack_path, &index, &hash).unwrap();
        assert_eq!(data, retrieved);
    }
}
