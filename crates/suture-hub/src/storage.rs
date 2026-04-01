//! SQLite-backed persistent storage for the Suture Hub.
//!
//! Stores repositories, patches, branches, blobs, and authorized public keys
//! in a single SQLite database. This replaces the in-memory HashMap approach.

use rusqlite::{params, Connection};
use std::path::Path;
use thiserror::Error;

use crate::types::{BlobRef, BranchProto, HashProto, PatchProto};

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("repo not found: {0}")]
    RepoNotFound(String),

    #[error("base64 error: {0}")]
    Base64(String),

    #[error("{0}")]
    Custom(String),
}

/// Persistent SQLite storage for the hub.
pub struct HubStorage {
    conn: Connection,
}

/// Mirror row from DB: (repo_name, upstream_url, upstream_repo, last_sync, status)
type MirrorRow = (String, String, String, Option<i64>, String);

/// Mirror list row from DB: (id, repo_name, upstream_url, upstream_repo, last_sync, status)
type MirrorListRow = (i64, String, String, String, Option<i64>, String);

impl HubStorage {
    /// Open or create the hub database at the given path.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS repos (
                repo_id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS patches (
                repo_id TEXT NOT NULL,
                patch_id TEXT NOT NULL,
                operation_type TEXT NOT NULL,
                touch_set TEXT NOT NULL,
                target_path TEXT,
                payload TEXT NOT NULL,
                parent_ids TEXT NOT NULL,
                author TEXT NOT NULL,
                message TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                PRIMARY KEY (repo_id, patch_id)
            );

            CREATE TABLE IF NOT EXISTS branches (
                repo_id TEXT NOT NULL,
                name TEXT NOT NULL,
                target_patch_id TEXT NOT NULL,
                PRIMARY KEY (repo_id, name)
            );

            CREATE TABLE IF NOT EXISTS blobs (
                repo_id TEXT NOT NULL,
                blob_hash TEXT NOT NULL,
                data BLOB NOT NULL,
                PRIMARY KEY (repo_id, blob_hash)
            );

            CREATE TABLE IF NOT EXISTS authorized_keys (
                author TEXT PRIMARY KEY,
                public_key BLOB NOT NULL,
                added_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS tokens (
                token TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                description TEXT
            );

            CREATE TABLE IF NOT EXISTS mirrors (
                id INTEGER PRIMARY KEY,
                repo_name TEXT NOT NULL,
                upstream_url TEXT NOT NULL,
                upstream_repo TEXT NOT NULL,
                last_sync INTEGER,
                status TEXT DEFAULT 'idle'
            );

            CREATE INDEX IF NOT EXISTS idx_patches_repo ON patches(repo_id);
            CREATE INDEX IF NOT EXISTS idx_branches_repo ON branches(repo_id);
            CREATE INDEX IF NOT EXISTS idx_blobs_repo ON blobs(repo_id);
            CREATE INDEX IF NOT EXISTS idx_mirrors_repo ON mirrors(repo_name);
            ",
        )?;
        Ok(())
    }

    // === Repos ===

    /// Ensure a repo exists. Returns true if it was newly created.
    pub fn ensure_repo(&self, repo_id: &str) -> Result<bool, StorageError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO repos (repo_id) VALUES (?1)",
            params![repo_id],
        )?;
        Ok(self.conn.changes() > 0)
    }

    /// List all repo IDs.
    pub fn list_repos(&self) -> Result<Vec<String>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT repo_id FROM repos ORDER BY repo_id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    /// Check if a repo exists.
    pub fn repo_exists(&self, repo_id: &str) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM repos WHERE repo_id = ?1",
            params![repo_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // === Patches ===

    /// Store a patch if it doesn't already exist. Returns true if newly inserted.
    pub fn insert_patch(&self, repo_id: &str, patch: &PatchProto) -> Result<bool, StorageError> {
        let id_hex = hash_to_hex(&patch.id);
        let parent_ids_json = serde_json::to_string(
            &patch
                .parent_ids
                .iter()
                .map(|h| h.value.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();
        let touch_set_json = serde_json::to_string(&patch.touch_set).unwrap_or_default();

        self.conn.execute(
            "INSERT OR IGNORE INTO patches (repo_id, patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                repo_id,
                id_hex,
                patch.operation_type,
                touch_set_json,
                patch.target_path,
                patch.payload,
                parent_ids_json,
                patch.author,
                patch.message,
                patch.timestamp as i64,
            ],
        )?;
        Ok(self.conn.changes() > 0)
    }

    /// Get a patch by ID within a repo.
    pub fn get_patch(
        &self,
        repo_id: &str,
        patch_id_hex: &str,
    ) -> Result<Option<PatchProto>, StorageError> {
        let result = self.conn.query_row(
            "SELECT patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp
             FROM patches WHERE repo_id = ?1 AND patch_id = ?2",
            params![repo_id, patch_id_hex],
            |row| {
                let id_hex: String = row.get(0)?;
                let operation_type: String = row.get(1)?;
                let touch_set_json: String = row.get(2)?;
                let target_path: Option<String> = row.get(3)?;
                let payload: String = row.get(4)?;
                let parent_ids_json: String = row.get(5)?;
                let author: String = row.get(6)?;
                let message: String = row.get(7)?;
                let timestamp: i64 = row.get(8)?;

                let touch_set: Vec<String> =
                    serde_json::from_str(&touch_set_json).unwrap_or_default();
                let parent_ids: Vec<String> =
                    serde_json::from_str(&parent_ids_json).unwrap_or_default();

                Ok(PatchProto {
                    id: HashProto { value: id_hex },
                    operation_type,
                    touch_set,
                    target_path,
                    payload,
                    parent_ids: parent_ids
                        .into_iter()
                        .map(|h| HashProto { value: h })
                        .collect(),
                    author,
                    message,
                    timestamp: timestamp as u64,
                })
            },
        );

        match result {
            Ok(patch) => Ok(Some(patch)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Get all patches for a repo.
    pub fn get_all_patches(&self, repo_id: &str) -> Result<Vec<PatchProto>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp
             FROM patches WHERE repo_id = ?1",
        )?;

        let rows = stmt.query_map(params![repo_id], |row| {
            let id_hex: String = row.get(0)?;
            let operation_type: String = row.get(1)?;
            let touch_set_json: String = row.get(2)?;
            let target_path: Option<String> = row.get(3)?;
            let payload: String = row.get(4)?;
            let parent_ids_json: String = row.get(5)?;
            let author: String = row.get(6)?;
            let message: String = row.get(7)?;
            let timestamp: i64 = row.get(8)?;

            let touch_set: Vec<String> = serde_json::from_str(&touch_set_json).unwrap_or_default();
            let parent_ids: Vec<String> =
                serde_json::from_str(&parent_ids_json).unwrap_or_default();

            Ok(PatchProto {
                id: HashProto { value: id_hex },
                operation_type,
                touch_set,
                target_path,
                payload,
                parent_ids: parent_ids
                    .into_iter()
                    .map(|h| HashProto { value: h })
                    .collect(),
                author,
                message,
                timestamp: timestamp as u64,
            })
        })?;

        let mut patches = Vec::new();
        for row in rows {
            patches.push(row?);
        }
        Ok(patches)
    }

    /// Count patches in a repo.
    pub fn patch_count(&self, repo_id: &str) -> Result<u64, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM patches WHERE repo_id = ?1",
            params![repo_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    // === Branches ===

    /// Set a branch pointer.
    pub fn set_branch(
        &self,
        repo_id: &str,
        name: &str,
        target_patch_id: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO branches (repo_id, name, target_patch_id) VALUES (?1, ?2, ?3)",
            params![repo_id, name, target_patch_id],
        )?;
        Ok(())
    }

    /// Get all branches for a repo.
    pub fn get_branches(&self, repo_id: &str) -> Result<Vec<BranchProto>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT name, target_patch_id FROM branches WHERE repo_id = ?1 ORDER BY name",
        )?;

        let rows = stmt.query_map(params![repo_id], |row| {
            let name: String = row.get(0)?;
            let target_hex: String = row.get(1)?;
            Ok((name, target_hex))
        })?;

        let mut branches = Vec::new();
        for row in rows {
            let (name, target_hex) = row?;
            branches.push(BranchProto {
                name,
                target_id: HashProto { value: target_hex },
            });
        }
        Ok(branches)
    }

    // === Blobs ===

    /// Store a blob. Overwrites if exists (content-addressed, idempotent).
    pub fn store_blob(
        &self,
        repo_id: &str,
        hash_hex: &str,
        data: &[u8],
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO blobs (repo_id, blob_hash, data) VALUES (?1, ?2, ?3)",
            params![repo_id, hash_hex, data],
        )?;
        Ok(())
    }

    /// Get a blob by hash.
    pub fn get_blob(&self, repo_id: &str, hash_hex: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let result = self.conn.query_row(
            "SELECT data FROM blobs WHERE repo_id = ?1 AND blob_hash = ?2",
            params![repo_id, hash_hex],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Get all blobs for a repo.
    pub fn get_all_blobs(&self, repo_id: &str) -> Result<Vec<BlobRef>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT blob_hash, data FROM blobs WHERE repo_id = ?1")?;

        let rows = stmt.query_map(params![repo_id], |row| {
            let hash_hex: String = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            Ok((hash_hex, data))
        })?;

        let mut blobs = Vec::new();
        for row in rows {
            let (hash_hex, data) = row?;
            blobs.push(BlobRef {
                hash: HashProto { value: hash_hex },
                data: base64_encode(&data),
            });
        }
        Ok(blobs)
    }

    // === Authorized Keys ===

    /// Add an authorized public key for an author.
    pub fn add_authorized_key(
        &self,
        author: &str,
        public_key_bytes: &[u8],
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO authorized_keys (author, public_key) VALUES (?1, ?2)",
            params![author, public_key_bytes],
        )?;
        Ok(())
    }

    /// Get the public key for an author.
    pub fn get_authorized_key(&self, author: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let result = self.conn.query_row(
            "SELECT public_key FROM authorized_keys WHERE author = ?1",
            params![author],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(bytes) => Ok(Some(bytes)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Check if any authorized keys exist (for auth-required vs auth-optional mode).
    pub fn has_authorized_keys(&self) -> Result<bool, StorageError> {
        let count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM authorized_keys", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    // === Tokens ===

    pub fn store_token(
        &self,
        token: &str,
        created_at: u64,
        description: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO tokens (token, created_at, description) VALUES (?1, ?2, ?3)",
            params![token, created_at as i64, description],
        )?;
        Ok(())
    }

    pub fn verify_token(&self, token: &str) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tokens WHERE token = ?1",
            params![token],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn has_tokens(&self) -> Result<bool, StorageError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    // === Mirrors ===

    pub fn add_mirror(
        &self,
        repo_name: &str,
        upstream_url: &str,
        upstream_repo: &str,
    ) -> Result<i64, StorageError> {
        self.conn.execute(
            "INSERT INTO mirrors (repo_name, upstream_url, upstream_repo, last_sync, status)
             VALUES (?1, ?2, ?3, NULL, 'idle')",
            params![repo_name, upstream_url, upstream_repo],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_mirror(&self, mirror_id: i64) -> Result<Option<MirrorRow>, StorageError> {
        let result = self.conn.query_row(
            "SELECT repo_name, upstream_url, upstream_repo, last_sync, status FROM mirrors WHERE id = ?1",
            params![mirror_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        );
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn update_mirror_status(
        &self,
        mirror_id: i64,
        status: &str,
        last_sync: Option<i64>,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE mirrors SET status = ?1, last_sync = COALESCE(?2, last_sync) WHERE id = ?3",
            params![status, last_sync, mirror_id],
        )?;
        Ok(())
    }

    pub fn list_mirrors(&self) -> Result<Vec<MirrorListRow>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, repo_name, upstream_url, upstream_repo, last_sync, status FROM mirrors ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        let mut mirrors = Vec::new();
        for row in rows {
            mirrors.push(row?);
        }
        Ok(mirrors)
    }

    pub fn get_mirror_by_repo(&self, repo_name: &str) -> Result<Option<i64>, StorageError> {
        let result = self.conn.query_row(
            "SELECT id FROM mirrors WHERE repo_name = ?1",
            params![repo_name],
            |row| row.get::<_, i64>(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn hash_to_hex(h: &HashProto) -> String {
    h.value.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_hash_proto(hex: &str) -> HashProto {
        HashProto {
            value: hex.to_string(),
        }
    }

    fn make_patch(id_hex: &str, op: &str, parents: &[&str], author: &str) -> PatchProto {
        PatchProto {
            id: make_hash_proto(id_hex),
            operation_type: op.to_string(),
            touch_set: vec![format!("file_{id_hex}")],
            target_path: Some(format!("file_{id_hex}")),
            payload: String::new(),
            parent_ids: parents.iter().map(|p| make_hash_proto(p)).collect(),
            author: author.to_string(),
            message: format!("patch {id_hex}"),
            timestamp: 0,
        }
    }

    #[allow(dead_code)]
    fn make_branch(name: &str, target: &str) -> BranchProto {
        BranchProto {
            name: name.to_string(),
            target_id: make_hash_proto(target),
        }
    }

    #[test]
    fn test_persistence_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("hub.db");

        // Write
        let store = HubStorage::open(&db_path).unwrap();
        store.ensure_repo("test-repo").unwrap();
        let patch = make_patch(&"a".repeat(64), "Create", &[], "alice");
        store.insert_patch("test-repo", &patch).unwrap();
        store
            .set_branch("test-repo", "main", &"a".repeat(64))
            .unwrap();
        store
            .store_blob("test-repo", &"deadbeef".repeat(8), b"hello")
            .unwrap();
        drop(store);

        // Read back
        let store2 = HubStorage::open(&db_path).unwrap();
        assert!(store2.repo_exists("test-repo").unwrap());
        let all_patches = store2.get_all_patches("test-repo").unwrap();
        assert_eq!(all_patches.len(), 1);
        let branches = store2.get_branches("test-repo").unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "main");
        let blob = store2
            .get_blob("test-repo", &"deadbeef".repeat(8))
            .unwrap()
            .unwrap();
        assert_eq!(blob, b"hello");
    }

    #[test]
    fn test_duplicate_patch_ignored() {
        let store = HubStorage::open_in_memory().unwrap();
        store.ensure_repo("repo").unwrap();
        let patch = make_patch(&"a".repeat(64), "Create", &[], "alice");

        assert!(store.insert_patch("repo", &patch).unwrap());
        assert!(!store.insert_patch("repo", &patch).unwrap());
        assert_eq!(store.patch_count("repo").unwrap(), 1);
    }

    #[test]
    fn test_authorized_keys() {
        let store = HubStorage::open_in_memory().unwrap();
        assert!(!store.has_authorized_keys().unwrap());

        let key = [0u8; 32];
        store.add_authorized_key("alice", &key).unwrap();
        assert!(store.has_authorized_keys().unwrap());

        let retrieved = store.get_authorized_key("alice").unwrap().unwrap();
        assert_eq!(retrieved, key);

        assert!(store.get_authorized_key("bob").unwrap().is_none());
    }

    #[test]
    fn test_list_repos() {
        let store = HubStorage::open_in_memory().unwrap();
        store.ensure_repo("repo-1").unwrap();
        store.ensure_repo("repo-2").unwrap();
        store.ensure_repo("repo-1").unwrap(); // duplicate

        let repos = store.list_repos().unwrap();
        assert_eq!(repos.len(), 2);
    }
}
