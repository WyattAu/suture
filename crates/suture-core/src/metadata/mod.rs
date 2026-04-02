//! Metadata — persistent storage and global configuration.
//!
//! Uses SQLite in WAL mode for concurrent read access. The metadata store
//! is the persistent backing for the in-memory PatchDag.

pub mod global_config;

use crate::patch::types::{Patch, PatchId, TouchSet};
use rusqlite::{Connection, params};
use std::path::Path;
use suture_common::{BranchName, FileStatus, Hash, RepoPath};
use thiserror::Error;

/// Metadata store errors.
#[derive(Error, Debug)]
pub enum MetaError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("patch not found: {0}")]
    PatchNotFound(String),

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("corrupt metadata: {0}")]
    Corrupt(String),

    #[error("migration failed: {0}")]
    MigrationFailed(String),

    #[error("{0}")]
    Custom(String),
}

/// The SQLite metadata store.
pub struct MetadataStore {
    conn: Connection,
}

/// Current schema version.
const SCHEMA_VERSION: i32 = 1;

impl MetadataStore {
    /// Open or create a metadata database at the given path.
    pub fn open(path: &Path) -> Result<Self, MetaError> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrency
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory metadata database (for testing).
    pub fn open_in_memory() -> Result<Self, MetaError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Get a reference to the underlying SQLite connection.
    ///
    /// Used internally for direct queries that don't have dedicated methods.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run schema migrations.
    fn migrate(&mut self) -> Result<(), MetaError> {
        // Create schema_version table
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;

        let current_version: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version < 1 {
            self.conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS patches (
                    id TEXT PRIMARY KEY,
                    parent_ids TEXT NOT NULL,
                    operation_type TEXT NOT NULL,
                    touch_set TEXT NOT NULL,
                    target_path TEXT,
                    payload BLOB,
                    timestamp INTEGER NOT NULL,
                    author TEXT NOT NULL,
                    message TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS edges (
                    parent_id TEXT NOT NULL,
                    child_id TEXT NOT NULL,
                    PRIMARY KEY (parent_id, child_id)
                );

                CREATE TABLE IF NOT EXISTS branches (
                    name TEXT PRIMARY KEY,
                    target_patch_id TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE IF NOT EXISTS working_set (
                    path TEXT PRIMARY KEY,
                    patch_id TEXT,
                    status TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS config (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_edges_parent ON edges(parent_id);
                CREATE INDEX IF NOT EXISTS idx_edges_child ON edges(child_id);
                CREATE INDEX IF NOT EXISTS idx_branches_target ON branches(target_patch_id);

                CREATE TABLE IF NOT EXISTS public_keys (
                    author TEXT PRIMARY KEY,
                    public_key BLOB NOT NULL
                );

                CREATE TABLE IF NOT EXISTS signatures (
                    patch_id TEXT PRIMARY KEY,
                    signature BLOB NOT NULL
                );
                ",
            )?;

            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (?)",
                params![SCHEMA_VERSION],
            )?;
        }

        Ok(())
    }

    /// Store a patch in the metadata database.
    pub fn store_patch(&self, patch: &Patch) -> Result<(), MetaError> {
        let parent_ids_json = serde_json::to_string(&patch.parent_ids)
            .map_err(|e| MetaError::Corrupt(e.to_string()))?;
        let touch_set_json = serde_json::to_string(&patch.touch_set.iter().collect::<Vec<_>>())
            .map_err(|e| MetaError::Corrupt(e.to_string()))?;

        self.conn.execute(
            "INSERT OR REPLACE INTO patches (id, parent_ids, operation_type, touch_set, target_path, payload, timestamp, author, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                patch.id.to_hex(),
                parent_ids_json,
                patch.operation_type.to_string(),
                touch_set_json,
                patch.target_path.as_deref(),
                &patch.payload,
                patch.timestamp as i64,
                &patch.author,
                &patch.message,
            ],
        )?;

        // Store edges
        for parent_id in &patch.parent_ids {
            self.conn.execute(
                "INSERT OR IGNORE INTO edges (parent_id, child_id) VALUES (?1, ?2)",
                params![parent_id.to_hex(), patch.id.to_hex()],
            )?;
        }

        Ok(())
    }

    /// Retrieve a patch by ID.
    pub fn get_patch(&self, id: &PatchId) -> Result<Patch, MetaError> {
        let hex = id.to_hex();
        self.conn
            .query_row(
                "SELECT id, parent_ids, operation_type, touch_set, target_path, payload, timestamp, author, message
                 FROM patches WHERE id = ?1",
                params![hex],
                |row| {
                    let parent_ids_json: String = row.get(1)?;
                    let op_type_str: String = row.get(2)?;
                    let touch_set_json: String = row.get(3)?;
                    let target_path: Option<String> = row.get(4)?;
                    let payload: Vec<u8> = row.get(5)?;
                    let timestamp: i64 = row.get(6)?;
                    let author: String = row.get(7)?;
                    let message: String = row.get(8)?;

                    let parent_ids: Vec<PatchId> = serde_json::from_str(&parent_ids_json)
                        .unwrap_or_default();
                    let touch_addrs: Vec<String> = serde_json::from_str(&touch_set_json)
                        .unwrap_or_default();
                    let touch_set = TouchSet::from_addrs(touch_addrs);

                    let op_type = match op_type_str.as_str() {
                        "create" => crate::patch::types::OperationType::Create,
                        "delete" => crate::patch::types::OperationType::Delete,
                        "modify" => crate::patch::types::OperationType::Modify,
                        "move" => crate::patch::types::OperationType::Move,
                        "metadata" => crate::patch::types::OperationType::Metadata,
                        "merge" => crate::patch::types::OperationType::Merge,
                        "identity" => crate::patch::types::OperationType::Identity,
                        _ => crate::patch::types::OperationType::Modify,
                    };

                    Ok(Patch {
                        id: *id,
                        parent_ids,
                        operation_type: op_type,
                        touch_set,
                        target_path,
                        payload,
                        timestamp: timestamp as u64,
                        author,
                        message,
                    })
                },
            )
            .map_err(|_| MetaError::PatchNotFound(hex))
    }

    /// Store a branch pointer.
    pub fn set_branch(&self, name: &BranchName, target: &PatchId) -> Result<(), MetaError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO branches (name, target_patch_id) VALUES (?1, ?2)",
            params![name.as_str(), target.to_hex()],
        )?;
        Ok(())
    }

    /// Get a branch target.
    pub fn get_branch(&self, name: &BranchName) -> Result<PatchId, MetaError> {
        let hex: String = self
            .conn
            .query_row(
                "SELECT target_patch_id FROM branches WHERE name = ?1",
                params![name.as_str()],
                |row| row.get(0),
            )
            .map_err(|_| MetaError::BranchNotFound(name.as_str().to_string()))?;

        PatchId::from_hex(&hex).map_err(|e| MetaError::Corrupt(e.to_string()))
    }

    /// List all branches.
    pub fn list_branches(&self) -> Result<Vec<(String, PatchId)>, MetaError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, target_patch_id FROM branches ORDER BY name")?;

        let branches = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let target_hex: String = row.get(1)?;
                Ok((name, target_hex))
            })?
            .filter_map(|r| {
                r.ok()
                    .and_then(|(name, hex)| Hash::from_hex(&hex).ok().map(|id| (name, id)))
            })
            .collect();

        Ok(branches)
    }

    /// Store a DAG edge.
    pub fn store_edge(&self, parent: &PatchId, child: &PatchId) -> Result<(), MetaError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO edges (parent_id, child_id) VALUES (?1, ?2)",
            params![parent.to_hex(), child.to_hex()],
        )?;
        Ok(())
    }

    /// Get parent and child IDs for a patch.
    pub fn get_edges(&self, patch_id: &PatchId) -> Result<(Vec<PatchId>, Vec<PatchId>), MetaError> {
        let hex = patch_id.to_hex();

        let parents: Vec<PatchId> = {
            let mut stmt = self
                .conn
                .prepare("SELECT parent_id FROM edges WHERE child_id = ?1")?;
            let rows = stmt.query_map(params![hex], |row| row.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok().and_then(|h| Hash::from_hex(&h).ok()))
                .collect()
        };

        let children: Vec<PatchId> = {
            let mut stmt = self
                .conn
                .prepare("SELECT child_id FROM edges WHERE parent_id = ?1")?;
            let rows = stmt.query_map(params![hex], |row| row.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok().and_then(|h| Hash::from_hex(&h).ok()))
                .collect()
        };

        Ok((parents, children))
    }

    /// Add a file to the working set.
    pub fn working_set_add(&self, path: &RepoPath, status: FileStatus) -> Result<(), MetaError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO working_set (path, status) VALUES (?1, ?2)",
            params![path.as_str(), format!("{:?}", status).to_lowercase()],
        )?;
        Ok(())
    }

    /// Remove a file from the working set.
    pub fn working_set_remove(&self, path: &RepoPath) -> Result<(), MetaError> {
        self.conn.execute(
            "DELETE FROM working_set WHERE path = ?1",
            params![path.as_str()],
        )?;
        Ok(())
    }

    /// Get the working set.
    pub fn working_set(&self) -> Result<Vec<(String, FileStatus)>, MetaError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, status FROM working_set ORDER BY path")?;

        let entries = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let status_str: String = row.get(1)?;
                let status = match status_str.as_str() {
                    "added" => FileStatus::Added,
                    "modified" => FileStatus::Modified,
                    "deleted" => FileStatus::Deleted,
                    "clean" => FileStatus::Clean,
                    _ => FileStatus::Untracked,
                };
                Ok((path, status))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// Store a configuration value.
    pub fn set_config(&self, key: &str, value: &str) -> Result<(), MetaError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// List all config key-value pairs.
    pub fn list_config(&self) -> Result<Vec<(String, String)>, MetaError> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM config ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let Ok(pair) = row else { continue };
            result.push(pair);
        }
        Ok(result)
    }

    /// Delete a config key.
    pub fn delete_config(&self, key: &str) -> Result<(), MetaError> {
        self.conn
            .execute("DELETE FROM config WHERE key = ?", [key])?;
        Ok(())
    }

    /// Get a configuration value.
    pub fn get_config(&self, key: &str) -> Result<Option<String>, MetaError> {
        let result = self.conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MetaError::Database(e)),
        }
    }

    /// Get the number of patches stored.
    pub fn patch_count(&self) -> Result<i64, MetaError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM patches", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn store_public_key(&self, author: &str, public_key_bytes: &[u8]) -> Result<(), MetaError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO public_keys (author, public_key) VALUES (?1, ?2)",
                params![author, public_key_bytes],
            )
            .map_err(|e| MetaError::Custom(e.to_string()))?;
        Ok(())
    }

    pub fn get_public_key(&self, author: &str) -> Result<Option<Vec<u8>>, MetaError> {
        let mut stmt = self
            .conn
            .prepare("SELECT public_key FROM public_keys WHERE author = ?1")
            .map_err(|e| MetaError::Custom(e.to_string()))?;
        let result = stmt.query_row(params![author], |row| row.get::<_, Vec<u8>>(0));
        match result {
            Ok(bytes) => Ok(Some(bytes)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MetaError::Custom(e.to_string())),
        }
    }

    pub fn store_signature(&self, patch_id: &str, signature_bytes: &[u8]) -> Result<(), MetaError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO signatures (patch_id, signature) VALUES (?1, ?2)",
                params![patch_id, signature_bytes],
            )
            .map_err(|e| MetaError::Custom(e.to_string()))?;
        Ok(())
    }

    pub fn get_signature(&self, patch_id: &str) -> Result<Option<Vec<u8>>, MetaError> {
        let mut stmt = self
            .conn
            .prepare("SELECT signature FROM signatures WHERE patch_id = ?1")
            .map_err(|e| MetaError::Custom(e.to_string()))?;
        let result = stmt.query_row(params![patch_id], |row| row.get::<_, Vec<u8>>(0));
        match result {
            Ok(bytes) => Ok(Some(bytes)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MetaError::Custom(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::types::{OperationType, Patch, TouchSet};

    fn make_test_patch(addr: &str) -> Patch {
        Patch::new(
            OperationType::Modify,
            TouchSet::single(addr),
            Some(format!("file_{}", addr)),
            vec![1, 2, 3],
            vec![],
            "test".to_string(),
            format!("edit {}", addr),
        )
    }

    #[test]
    fn test_open_in_memory() {
        let store = MetadataStore::open_in_memory().unwrap();
        assert_eq!(store.patch_count().unwrap(), 0);
    }

    #[test]
    fn test_store_and_get_patch() {
        let store = MetadataStore::open_in_memory().unwrap();
        let patch = make_test_patch("A1");
        let id = patch.id;

        store.store_patch(&patch).unwrap();
        let retrieved = store.get_patch(&id).unwrap();

        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.author, "test");
        assert_eq!(retrieved.payload, vec![1, 2, 3]);
    }

    #[test]
    fn test_store_and_get_branch() {
        let store = MetadataStore::open_in_memory().unwrap();
        let patch = make_test_patch("root");
        store.store_patch(&patch).unwrap();

        let main = BranchName::new("main").unwrap();
        store.set_branch(&main, &patch.id).unwrap();

        let target = store.get_branch(&main).unwrap();
        assert_eq!(target, patch.id);
    }

    #[test]
    fn test_list_branches() {
        let store = MetadataStore::open_in_memory().unwrap();
        let patch = make_test_patch("root");
        store.store_patch(&patch).unwrap();

        store
            .set_branch(&BranchName::new("main").unwrap(), &patch.id)
            .unwrap();
        store
            .set_branch(&BranchName::new("dev").unwrap(), &patch.id)
            .unwrap();

        let branches = store.list_branches().unwrap();
        assert_eq!(branches.len(), 2);
    }

    #[test]
    fn test_working_set() {
        let store = MetadataStore::open_in_memory().unwrap();

        let path = RepoPath::new("src/main.rs").unwrap();
        store.working_set_add(&path, FileStatus::Added).unwrap();

        let ws = store.working_set().unwrap();
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].0, "src/main.rs");
        assert_eq!(ws[0].1, FileStatus::Added);

        store.working_set_remove(&path).unwrap();
        let ws = store.working_set().unwrap();
        assert!(ws.is_empty());
    }

    #[test]
    fn test_config() {
        let store = MetadataStore::open_in_memory().unwrap();

        assert!(store.get_config("key").unwrap().is_none());

        store.set_config("key", "value").unwrap();
        assert_eq!(store.get_config("key").unwrap(), Some("value".to_string()));

        store.set_config("key", "updated").unwrap();
        assert_eq!(
            store.get_config("key").unwrap(),
            Some("updated".to_string())
        );
    }

    #[test]
    fn test_edges() {
        let store = MetadataStore::open_in_memory().unwrap();
        let parent = make_test_patch("parent");
        let child = make_test_patch("child");
        store.store_patch(&parent).unwrap();
        store.store_patch(&child).unwrap();

        store.store_edge(&parent.id, &child.id).unwrap();

        let (parents, _children) = store.get_edges(&child.id).unwrap();
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0], parent.id);

        let (_, children) = store.get_edges(&parent.id).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], child.id);
    }
}
