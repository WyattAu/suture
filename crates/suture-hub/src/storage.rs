//! SQLite-backed persistent storage for the Suture Hub.
//!
//! Stores repositories, patches, branches, blobs, and authorized public keys
//! in a single SQLite database. This replaces the in-memory HashMap approach.

use rusqlite::{params, Connection};
use std::path::Path;
use thiserror::Error;

use crate::types::{BlobRef, BranchProto, HashProto, PatchProto, UserInfo};
use crate::webhooks::Webhook;

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

    #[error("blob exceeds maximum allowed size of {0} bytes")]
    BlobTooLarge(usize),
}

/// Persistent SQLite storage for the hub.
///
/// The SQLite connection is wrapped in a `std::sync::Mutex` to make this type
/// `Send + Sync` without requiring `unsafe impl`. All methods acquire the
/// mutex lock internally. The outer synchronization (tokio::sync::RwLock in
/// server.rs) provides async-compatible locking; this inner mutex satisfies
/// the Rust type system's thread-safety requirements.
pub struct HubStorage {
    conn: std::sync::Mutex<Connection>,
    max_blob_size: usize,
    max_page_size: usize,
}

/// Mirror row from DB: (repo_name, upstream_url, upstream_repo, last_sync, status)
type MirrorRow = (String, String, String, Option<i64>, String);

/// Mirror list row from DB: (id, repo_name, upstream_url, upstream_repo, last_sync, status)
type MirrorListRow = (i64, String, String, String, Option<i64>, String);

impl HubStorage {
    /// Open or create the hub database at the given path.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Self::open_with_limits(path, 50 * 1024 * 1024, 10000)
    }

    /// Open or create the hub database with custom limits.
    pub fn open_with_limits(
        path: &Path,
        max_blob_size: usize,
        max_page_size: usize,
    ) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let mut store = Self {
            conn: std::sync::Mutex::new(conn),
            max_blob_size,
            max_page_size,
        };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        Self::open_in_memory_with_limits(50 * 1024 * 1024, 10000)
    }

    /// Open an in-memory database with custom limits.
    pub fn open_in_memory_with_limits(
        max_blob_size: usize,
        max_page_size: usize,
    ) -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let mut store = Self {
            conn: std::sync::Mutex::new(conn),
            max_blob_size,
            max_page_size,
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), StorageError> {
        let conn = self.conn.get_mut().expect("not shared yet");
        conn.execute_batch(
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
                description TEXT,
                expires_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS branch_protection (
                repo_id TEXT NOT NULL,
                branch_name TEXT NOT NULL,
                PRIMARY KEY (repo_id, branch_name)
            );

            CREATE TABLE IF NOT EXISTS mirrors (
                id INTEGER PRIMARY KEY,
                repo_name TEXT NOT NULL,
                upstream_url TEXT NOT NULL,
                upstream_repo TEXT NOT NULL,
                last_sync INTEGER,
                status TEXT DEFAULT 'idle'
            );

            CREATE TABLE IF NOT EXISTS users (
                username TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'member',
                api_token TEXT UNIQUE,
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_patches_repo ON patches(repo_id);
            CREATE INDEX IF NOT EXISTS idx_branches_repo ON branches(repo_id);
            CREATE INDEX IF NOT EXISTS idx_blobs_repo ON blobs(repo_id);
            CREATE INDEX IF NOT EXISTS idx_mirrors_repo ON mirrors(repo_name);

            CREATE TABLE IF NOT EXISTS replication_peers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                peer_url TEXT NOT NULL UNIQUE,
                role TEXT NOT NULL DEFAULT 'follower',
                last_sync_seq INTEGER DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'active',
                added_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS replication_log (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                operation TEXT NOT NULL,
                table_name TEXT NOT NULL,
                row_id TEXT NOT NULL,
                data TEXT,
                timestamp INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS webhooks (
                id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                url TEXT NOT NULL,
                events TEXT NOT NULL,
                secret TEXT,
                created_at INTEGER NOT NULL,
                active INTEGER NOT NULL DEFAULT 1,
                FOREIGN KEY (repo_id) REFERENCES repos(repo_id)
            );

            CREATE INDEX IF NOT EXISTS idx_webhooks_repo ON webhooks(repo_id);
            ",
        )?;

        let has_expires: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('tokens') WHERE name = 'expires_at'",
            [],
            |row| row.get(0),
        )?;

        if !has_expires {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let default_expiry = now + (30 * 24 * 60 * 60);
            conn.execute_batch(
                "ALTER TABLE tokens ADD COLUMN expires_at INTEGER NOT NULL DEFAULT 0;",
            )?;
            conn.execute(
                "UPDATE tokens SET expires_at = ?1 WHERE expires_at = 0",
                params![default_expiry],
            )?;
        }

        Ok(())
    }

    // === Repos ===

    /// Ensure a repo exists. Returns true if it was newly created.
    pub fn ensure_repo(&self, repo_id: &str) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT OR IGNORE INTO repos (repo_id) VALUES (?1)",
            params![repo_id],
        )?;
        Ok(conn.changes() > 0)
    }

    /// List all repo IDs.
    pub fn list_repos(&self) -> Result<Vec<String>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare("SELECT repo_id FROM repos ORDER BY repo_id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    /// Check if a repo exists.
    pub fn repo_exists(&self, repo_id: &str) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let count: i64 = conn.query_row(
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
        let touch_set_json = serde_json::to_string(&patch.touch_set).unwrap_or_default();
        let parent_ids_json = serde_json::to_string(
            &patch
                .parent_ids
                .iter()
                .map(|h| &h.value)
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();

        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
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
        Ok(conn.changes() > 0)
    }

    /// Get a patch by ID within a repo.
    pub fn get_patch(
        &self,
        repo_id: &str,
        patch_id_hex: &str,
    ) -> Result<Option<PatchProto>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
            "SELECT patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp
             FROM patches WHERE repo_id = ?1 LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![repo_id, self.max_page_size as i64], |row| {
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let count: i64 = conn.query_row(
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT OR REPLACE INTO branches (repo_id, name, target_patch_id) VALUES (?1, ?2, ?3)",
            params![repo_id, name, target_patch_id],
        )?;
        Ok(())
    }

    /// Get all branches for a repo.
    pub fn get_branches(&self, repo_id: &str) -> Result<Vec<BranchProto>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
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
        if data.len() > self.max_blob_size {
            return Err(StorageError::BlobTooLarge(self.max_blob_size));
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT OR REPLACE INTO blobs (repo_id, blob_hash, data) VALUES (?1, ?2, ?3)",
            params![repo_id, hash_hex, data],
        )?;
        Ok(())
    }

    pub fn delete_blob(&self, repo_id: &str, hash_hex: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "DELETE FROM blobs WHERE repo_id = ?1 AND blob_hash = ?2",
            params![repo_id, hash_hex],
        )?;
        Ok(())
    }

    /// Get a blob by hash.
    pub fn get_blob(&self, repo_id: &str, hash_hex: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
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

    /// Get all blobs for a repo. Blobs exceeding `max_blob_size` are returned
    /// with empty data and `truncated: true`.
    pub fn get_all_blobs(&self, repo_id: &str) -> Result<Vec<BlobRef>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare("SELECT blob_hash, data FROM blobs WHERE repo_id = ?1")?;

        let rows = stmt.query_map(params![repo_id], |row| {
            let hash_hex: String = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            Ok((hash_hex, data))
        })?;

        let max_blob_size = self.max_blob_size;
        let mut blobs = Vec::new();
        for row in rows {
            let (hash_hex, data) = row?;
            let (data_b64, truncated) = if data.len() > max_blob_size {
                (String::new(), true)
            } else {
                (base64_encode(&data), false)
            };
            blobs.push(BlobRef {
                hash: HashProto { value: hash_hex },
                data: data_b64,
                truncated,
            });
        }
        Ok(blobs)
    }

    /// Get specific blobs by hash set.
    pub fn get_blobs(
        &self,
        repo_id: &str,
        hashes: &std::collections::HashSet<String>,
    ) -> Result<Vec<BlobRef>, StorageError> {
        if hashes.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: Vec<String> = hashes.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT blob_hash, data FROM blobs WHERE repo_id = ?1 AND blob_hash IN ({})",
            placeholders.join(",")
        );

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(repo_id.to_string()));
        for h in hashes {
            params_vec.push(Box::new(h.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let hash_hex: String = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            Ok((hash_hex, data))
        })?;

        let mut blobs = Vec::new();
        for row in rows {
            let (hash_hex, data) = row?;
            let (data_b64, truncated) = if data.len() > self.max_blob_size {
                (String::new(), true)
            } else {
                (base64_encode(&data), false)
            };
            blobs.push(BlobRef {
                hash: HashProto { value: hash_hex },
                data: data_b64,
                truncated,
            });
        }
        Ok(blobs)
    }

    /// Get the target patch ID for a specific branch, if it exists.
    pub fn get_branch_target(
        &self,
        repo_id: &str,
        branch_name: &str,
    ) -> Result<Option<String>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
            "SELECT target_patch_id FROM branches WHERE repo_id = ?1 AND name = ?2",
            params![repo_id, branch_name],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(hex) => Ok(Some(hex)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Check if `ancestor_id` is an ancestor of `descendant_id` by walking the parent chain.
    pub fn is_ancestor(
        &self,
        repo_id: &str,
        ancestor_id: &str,
        descendant_id: &str,
    ) -> Result<bool, StorageError> {
        if ancestor_id == descendant_id {
            return Ok(true);
        }

        let mut current = descendant_id.to_string();
        let mut visited = std::collections::HashSet::new();

        while !visited.contains(&current) {
            visited.insert(current.clone());
            let patch = match self.get_patch(repo_id, &current)? {
                Some(p) => p,
                None => return Ok(false),
            };
            if patch.parent_ids.is_empty() {
                return Ok(false);
            }
            for parent in &patch.parent_ids {
                let parent_hex = hash_to_hex(parent);
                if parent_hex == ancestor_id {
                    return Ok(true);
                }
                if !visited.contains(&parent_hex) {
                    current = parent_hex;
                }
            }
        }
        Ok(false)
    }

    // === Branch Protection ===

    pub fn protect_branch(&self, repo_id: &str, branch_name: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT OR IGNORE INTO branch_protection (repo_id, branch_name) VALUES (?1, ?2)",
            params![repo_id, branch_name],
        )?;
        Ok(())
    }

    pub fn unprotect_branch(&self, repo_id: &str, branch_name: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "DELETE FROM branch_protection WHERE repo_id = ?1 AND branch_name = ?2",
            params![repo_id, branch_name],
        )?;
        Ok(())
    }

    pub fn is_branch_protected(
        &self,
        repo_id: &str,
        branch_name: &str,
    ) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM branch_protection WHERE repo_id = ?1 AND branch_name = ?2",
            params![repo_id, branch_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // === Authorized Keys ===

    /// Add an authorized public key for an author.
    pub fn add_authorized_key(
        &self,
        author: &str,
        public_key_bytes: &[u8],
    ) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT OR REPLACE INTO authorized_keys (author, public_key) VALUES (?1, ?2)",
            params![author, public_key_bytes],
        )?;
        Ok(())
    }

    /// Get the public key for an author.
    pub fn get_authorized_key(&self, author: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM authorized_keys", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    // === Tokens ===

    pub fn store_token(
        &self,
        token: &str,
        created_at: u64,
        description: &str,
        expires_at: i64,
    ) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT INTO tokens (token, created_at, description, expires_at) VALUES (?1, ?2, ?3, ?4)",
            params![token, created_at as i64, description, expires_at],
        )?;
        Ok(())
    }

    pub fn verify_token(&self, token: &str) -> Result<bool, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tokens WHERE token = ?1 AND expires_at > ?2",
            params![token, now],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn has_tokens(&self) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    pub fn has_users(&self) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    // === Mirrors ===

    pub fn add_mirror(
        &self,
        repo_name: &str,
        upstream_url: &str,
        upstream_repo: &str,
    ) -> Result<i64, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT INTO mirrors (repo_name, upstream_url, upstream_repo, last_sync, status)
             VALUES (?1, ?2, ?3, NULL, 'idle')",
            params![repo_name, upstream_url, upstream_repo],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_mirror(&self, mirror_id: i64) -> Result<Option<MirrorRow>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "UPDATE mirrors SET status = ?1, last_sync = COALESCE(?2, last_sync) WHERE id = ?3",
            params![status, last_sync, mirror_id],
        )?;
        Ok(())
    }

    pub fn list_mirrors(&self) -> Result<Vec<MirrorListRow>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
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

    // === Users ===

    pub fn create_user(
        &self,
        username: &str,
        display_name: &str,
        role: &str,
        api_token: &str,
    ) -> Result<(), StorageError> {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT INTO users (username, display_name, role, api_token, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![username, display_name, role, api_token, created_at],
        )?;
        Ok(())
    }

    pub fn get_user(&self, username: &str) -> Result<Option<UserInfo>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
            "SELECT username, display_name, role, api_token, created_at FROM users WHERE username = ?1",
            params![username],
            |row| {
                Ok(UserInfo {
                    username: row.get(0)?,
                    display_name: row.get(1)?,
                    role: row.get(2)?,
                    api_token: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        );
        match result {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn get_user_by_token(&self, token: &str) -> Result<Option<UserInfo>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
            "SELECT username, display_name, role, api_token, created_at FROM users WHERE api_token = ?1",
            params![token],
            |row| {
                Ok(UserInfo {
                    username: row.get(0)?,
                    display_name: row.get(1)?,
                    role: row.get(2)?,
                    api_token: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        );
        match result {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn list_users(&self) -> Result<Vec<UserInfo>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
            "SELECT username, display_name, role, api_token, created_at FROM users ORDER BY username",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(UserInfo {
                username: row.get(0)?,
                display_name: row.get(1)?,
                role: row.get(2)?,
                api_token: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut users = Vec::new();
        for row in rows {
            users.push(row?);
        }
        Ok(users)
    }

    pub fn update_user_role(&self, username: &str, role: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "UPDATE users SET role = ?1 WHERE username = ?2",
            params![role, username],
        )?;
        Ok(())
    }

    pub fn delete_user(&self, username: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute("DELETE FROM users WHERE username = ?1", params![username])?;
        Ok(())
    }

    pub fn delete_repo(&self, repo_id: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute("DELETE FROM patches WHERE repo_id = ?1", params![repo_id])?;
        conn.execute("DELETE FROM branches WHERE repo_id = ?1", params![repo_id])?;
        conn.execute("DELETE FROM blobs WHERE repo_id = ?1", params![repo_id])?;
        conn.execute(
            "DELETE FROM branch_protection WHERE repo_id = ?1",
            params![repo_id],
        )?;
        conn.execute("DELETE FROM repos WHERE repo_id = ?1", params![repo_id])?;
        Ok(())
    }

    pub fn delete_branch(&self, repo_id: &str, branch_name: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "DELETE FROM branches WHERE repo_id = ?1 AND name = ?2",
            params![repo_id, branch_name],
        )?;
        conn.execute(
            "DELETE FROM branch_protection WHERE repo_id = ?1 AND branch_name = ?2",
            params![repo_id, branch_name],
        )?;
        Ok(())
    }

    pub fn delete_mirror(&self, mirror_id: i64) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute("DELETE FROM mirrors WHERE id = ?1", params![mirror_id])?;
        Ok(())
    }

    pub fn search_repos(&self, query: &str) -> Result<Vec<String>, StorageError> {
        let pattern = format!("%{query}%");
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt =
            conn.prepare("SELECT repo_id FROM repos WHERE repo_id LIKE ?1 ORDER BY repo_id")?;
        let rows = stmt.query_map(params![pattern], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    pub fn get_patches_at(
        &self,
        repo_id: &str,
        tip_id: &str,
    ) -> Result<Vec<PatchProto>, StorageError> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(tip_id.to_string());
        visited.insert(tip_id.to_string());

        while let Some(current_id) = queue.pop_front() {
            if let Some(patch) = self.get_patch(repo_id, &current_id)? {
                for parent in &patch.parent_ids {
                    if !visited.contains(&parent.value) {
                        visited.insert(parent.value.clone());
                        queue.push_back(parent.value.clone());
                    }
                }
            }
        }

        let mut patches = Vec::new();
        for id in &visited {
            if let Some(patch) = self.get_patch(repo_id, id)? {
                patches.push(patch);
            }
        }
        Ok(patches)
    }

    pub fn get_tree_at_branch(
        &self,
        repo_id: &str,
        branch: &str,
    ) -> Result<Vec<crate::types::TreeEntry>, StorageError> {
        use crate::types::TreeEntry;

        let tip_id = match self.get_branch_target(repo_id, branch)? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let mut patches = self.get_patches_at(repo_id, &tip_id)?;
        patches.sort_by_key(|p| p.timestamp);

        let mut tree: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        for patch in &patches {
            let path = match &patch.target_path {
                Some(p) => p.clone(),
                None => continue,
            };
            match patch.operation_type.as_str() {
                "Create" | "Modify" => {
                    tree.insert(path, patch.payload.clone());
                }
                "Delete" => {
                    tree.remove(&path);
                }
                _ => {}
            }
        }

        let mut entries: Vec<TreeEntry> = tree
            .into_iter()
            .map(|(path, content_hash)| TreeEntry { path, content_hash })
            .collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    pub fn search_patches(
        &self,
        repo_id: &str,
        query: &str,
    ) -> Result<Vec<PatchProto>, StorageError> {
        let pattern = format!("%{query}%");
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
            "SELECT patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp
             FROM patches WHERE repo_id = ?1 AND (author LIKE ?2 OR message LIKE ?3) LIMIT 50",
        )?;
        let rows = stmt.query_map(params![repo_id, pattern, pattern], |row| {
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

    // === Replication ===

    pub fn add_replication_peer(&self, peer_url: &str, role: &str) -> Result<i64, StorageError> {
        let added_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT INTO replication_peers (peer_url, role, last_sync_seq, status, added_at) VALUES (?1, ?2, 0, 'active', ?3)",
            params![peer_url, role, added_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn remove_replication_peer(&self, id: i64) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "DELETE FROM replication_peers WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn list_replication_peers(&self) -> Result<Vec<ReplicationPeer>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
            "SELECT id, peer_url, role, last_sync_seq, status, added_at FROM replication_peers ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ReplicationPeer {
                id: row.get(0)?,
                peer_url: row.get(1)?,
                role: row.get(2)?,
                last_sync_seq: row.get(3)?,
                status: row.get(4)?,
                added_at: row.get(5)?,
            })
        })?;
        let mut peers = Vec::new();
        for row in rows {
            peers.push(row?);
        }
        Ok(peers)
    }

    pub fn update_peer_sync_seq(&self, peer_id: i64, seq: i64) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "UPDATE replication_peers SET last_sync_seq = ?1 WHERE id = ?2",
            params![seq, peer_id],
        )?;
        Ok(())
    }

    pub fn get_replication_peer(&self, id: i64) -> Result<Option<ReplicationPeer>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let result = conn.query_row(
            "SELECT id, peer_url, role, last_sync_seq, status, added_at FROM replication_peers WHERE id = ?1",
            params![id],
            |row| {
                Ok(ReplicationPeer {
                    id: row.get(0)?,
                    peer_url: row.get(1)?,
                    role: row.get(2)?,
                    last_sync_seq: row.get(3)?,
                    status: row.get(4)?,
                    added_at: row.get(5)?,
                })
            },
        );
        match result {
            Ok(peer) => Ok(Some(peer)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn log_operation(
        &self,
        operation: &str,
        table_name: &str,
        row_id: &str,
        data: Option<&str>,
    ) -> Result<i64, StorageError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT INTO replication_log (operation, table_name, row_id, data, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![operation, table_name, row_id, data, timestamp],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_replication_log(
        &self,
        since_seq: i64,
    ) -> Result<Vec<ReplicationEntry>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
            "SELECT seq, operation, table_name, row_id, data, timestamp FROM replication_log WHERE seq > ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![since_seq], |row| {
            Ok(ReplicationEntry {
                seq: row.get(0)?,
                operation: row.get(1)?,
                table_name: row.get(2)?,
                row_id: row.get(3)?,
                data: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn apply_replication_entries(
        &self,
        entries: &[ReplicationEntry],
    ) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        for entry in entries {
            conn.execute(
                "INSERT OR IGNORE INTO replication_log (seq, operation, table_name, row_id, data, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![entry.seq, entry.operation, entry.table_name, entry.row_id, entry.data, entry.timestamp],
            )?;
        }
        Ok(())
    }

    pub fn get_replication_status(&self) -> Result<ReplicationStatus, StorageError> {
        let peers = self.list_replication_peers()?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let current_seq: i64 = conn.query_row(
            "SELECT COALESCE(MAX(seq), 0) FROM replication_log",
            [],
            |row| row.get(0),
        )?;
        Ok(ReplicationStatus {
            current_seq,
            peer_count: peers.len(),
            peers,
        })
    }

    // === Webhooks ===

    pub fn create_webhook(&self, webhook: &Webhook) -> Result<(), StorageError> {
        let events_json = serde_json::to_string(&webhook.events).unwrap_or_default();
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.execute(
            "INSERT INTO webhooks (id, repo_id, url, events, secret, created_at, active) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                webhook.id,
                webhook.repo_id,
                webhook.url,
                events_json,
                webhook.secret,
                webhook.created_at as i64,
                webhook.active as i32,
            ],
        )?;
        Ok(())
    }

    pub fn list_webhooks(&self, repo_id: &str) -> Result<Vec<Webhook>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
            "SELECT id, repo_id, url, events, secret, created_at, active FROM webhooks WHERE repo_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![repo_id], |row| {
            let events_json: String = row.get(3)?;
            let events: Vec<String> = serde_json::from_str(&events_json).unwrap_or_default();
            Ok(Webhook {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                url: row.get(2)?,
                events,
                secret: row.get(4)?,
                created_at: row.get::<_, i64>(5)? as u64,
                active: row.get::<_, i32>(6)? != 0,
            })
        })?;
        let mut webhooks = Vec::new();
        for row in rows {
            webhooks.push(row?);
        }
        Ok(webhooks)
    }

    pub fn get_webhook(&self, id: &str) -> Result<Webhook, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        conn.query_row(
            "SELECT id, repo_id, url, events, secret, created_at, active FROM webhooks WHERE id = ?1",
            params![id],
            |row| {
                let events_json: String = row.get(3)?;
                let events: Vec<String> = serde_json::from_str(&events_json).unwrap_or_default();
                Ok(Webhook {
                    id: row.get(0)?,
                    repo_id: row.get(1)?,
                    url: row.get(2)?,
                    events,
                    secret: row.get(4)?,
                    created_at: row.get::<_, i64>(5)? as u64,
                    active: row.get::<_, i32>(6)? != 0,
                })
            },
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StorageError::Custom(format!("webhook not found: {id}")),
            e => StorageError::Database(e),
        })
    }

    pub fn delete_webhook(&self, id: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Custom(format!("lock poisoned: {e}")))?;
        let changes = conn.execute("DELETE FROM webhooks WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(StorageError::Custom(format!("webhook not found: {id}")));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplicationPeer {
    pub id: i64,
    pub peer_url: String,
    pub role: String,
    pub last_sync_seq: i64,
    pub status: String,
    pub added_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplicationEntry {
    pub seq: i64,
    pub operation: String,
    pub table_name: String,
    pub row_id: String,
    pub data: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplicationStatus {
    pub current_seq: i64,
    pub peer_count: usize,
    pub peers: Vec<ReplicationPeer>,
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

    #[test]
    fn test_webhook_crud() {
        let store = HubStorage::open_in_memory().unwrap();
        store.ensure_repo("test-repo").unwrap();

        let webhook = Webhook {
            id: "wh-1".to_string(),
            repo_id: "test-repo".to_string(),
            url: "https://example.com/hook".to_string(),
            events: vec!["push".to_string(), "branch.create".to_string()],
            secret: Some("secret123".to_string()),
            created_at: 1000,
            active: true,
        };
        store.create_webhook(&webhook).unwrap();

        let listed = store.list_webhooks("test-repo").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "wh-1");
        assert_eq!(listed[0].events.len(), 2);

        let fetched = store.get_webhook("wh-1").unwrap();
        assert_eq!(fetched.url, "https://example.com/hook");
        assert_eq!(fetched.secret, Some("secret123".to_string()));
        assert!(fetched.active);

        assert!(store.list_webhooks("other-repo").unwrap().is_empty());

        store.delete_webhook("wh-1").unwrap();
        assert!(store.list_webhooks("test-repo").unwrap().is_empty());
    }
}
