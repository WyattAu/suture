//! SQLite-backed persistent storage for the Suture Hub.
//!
//! Stores repositories, patches, branches, blobs, and authorized public keys
//! in a single SQLite database. This replaces the in-memory HashMap approach.

use sha2::Digest;

use rusqlite::{Connection, params};
use std::path::Path;
use thiserror::Error;

use crate::types::{
    BlobRef, BranchProto, CodeSearchResult, HashProto, Issue, IssueComment, Organization,
    PatchProto, PrReview, PullRequestRecord, Release, Team, UserInfo, WikiPage,
};
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

    #[error("lock poisoned: {0}")]
    PoisonedLock(String),

    #[error("webhook not found: {0}")]
    WebhookNotFound(String),

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

fn issue_from_row(row: &rusqlite::Row) -> Result<Issue, rusqlite::Error> {
    let labels_json: String = row.get(7)?;
    let labels: Vec<String> = serde_json::from_str(&labels_json).unwrap_or_default();
    Ok(Issue {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        title: row.get(2)?,
        body: row.get(3)?,
        status: row.get(4)?,
        author: row.get(5)?,
        assignee: row.get(6)?,
        labels,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        closed_at: row.get(10)?,
    })
}

impl HubStorage {
    /// Get a reference to the inner connection mutex (for backup/restore).
    pub fn conn(&self) -> &std::sync::Mutex<Connection> {
        &self.conn
    }

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
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size = -65536; PRAGMA mmap_size = 268435456;")?;
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
        let conn = self
            .conn
            .get_mut()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
                expires_at INTEGER NOT NULL,
                scopes TEXT NOT NULL DEFAULT 'read,write'
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
            CREATE INDEX IF NOT EXISTS idx_patches_repo_ts ON patches(repo_id, timestamp, patch_id);
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

            CREATE TABLE IF NOT EXISTS sso_providers (
                provider_name TEXT PRIMARY KEY,
                config_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                actor TEXT NOT NULL DEFAULT '',
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL DEFAULT '',
                resource_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'success',
                details TEXT NOT NULL DEFAULT '',
                request_id TEXT NOT NULL DEFAULT '',
                client_ip TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log(actor);
            CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action);

            CREATE TABLE IF NOT EXISTS sso_states (
                state TEXT PRIMARY KEY,
                provider_name TEXT NOT NULL,
                nonce TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sso_states_created ON sso_states(created_at);

            CREATE TABLE IF NOT EXISTS sso_user_mappings (
                provider_name TEXT NOT NULL,
                provider_sub TEXT NOT NULL,
                username TEXT NOT NULL,
                linked_at INTEGER NOT NULL,
                PRIMARY KEY (provider_name, provider_sub),
                FOREIGN KEY (username) REFERENCES users(username)
            );

            CREATE TABLE IF NOT EXISTS issues (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_id TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'open',
                author TEXT NOT NULL,
                assignee TEXT,
                labels TEXT NOT NULL DEFAULT '[]',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                closed_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS issue_comments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                issue_id INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
                author TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_issues_repo_id ON issues(repo_id);
            CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(repo_id, status);

            CREATE TABLE IF NOT EXISTS pull_requests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_id TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'open',
                author TEXT NOT NULL,
                source_branch TEXT NOT NULL,
                target_branch TEXT NOT NULL DEFAULT 'main',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                merged_at INTEGER,
                merged_by TEXT
            );

            CREATE TABLE IF NOT EXISTS pr_reviews (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pr_id INTEGER NOT NULL REFERENCES pull_requests(id) ON DELETE CASCADE,
                reviewer TEXT NOT NULL,
                verdict TEXT NOT NULL,
                body TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_pr_repo_id ON pull_requests(repo_id);
            CREATE INDEX IF NOT EXISTS idx_pr_status ON pull_requests(repo_id, status);

            CREATE TABLE IF NOT EXISTS blob_trigrams (
                trigram TEXT NOT NULL,
                blob_hash TEXT NOT NULL,
                repo_id TEXT NOT NULL,
                PRIMARY KEY (trigram, blob_hash)
            );

            CREATE TABLE IF NOT EXISTS blob_metadata (
                hash TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                size INTEGER NOT NULL,
                is_text INTEGER NOT NULL DEFAULT 0,
                path TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_trigrams_trigram ON blob_trigrams(trigram);

            CREATE TABLE IF NOT EXISTS organizations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS teams (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                org_id INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                permission TEXT NOT NULL DEFAULT 'read',
                created_at INTEGER NOT NULL,
                UNIQUE(org_id, name)
            );

            CREATE TABLE IF NOT EXISTS team_members (
                team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
                username TEXT NOT NULL,
                PRIMARY KEY (team_id, username)
            );

            CREATE TABLE IF NOT EXISTS team_repos (
                team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
                repo_id TEXT NOT NULL,
                PRIMARY KEY (team_id, repo_id)
            );

            CREATE TABLE IF NOT EXISTS fork_relationships (
                fork_repo_id TEXT NOT NULL,
                parent_repo_id TEXT NOT NULL,
                forked_at INTEGER NOT NULL,
                PRIMARY KEY (fork_repo_id, parent_repo_id)
            );

            CREATE TABLE IF NOT EXISTS wiki_pages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_id TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                author TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(repo_id, title)
            );

            CREATE TABLE IF NOT EXISTS wiki_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                page_id INTEGER NOT NULL REFERENCES wiki_pages(id) ON DELETE CASCADE,
                content TEXT NOT NULL,
                author TEXT NOT NULL,
                edited_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS releases (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL DEFAULT '',
                author TEXT NOT NULL,
                prerelease INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                UNIQUE(repo_id, tag)
            );

            CREATE TABLE IF NOT EXISTS release_assets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                release_id INTEGER NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                size INTEGER NOT NULL,
                hash TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
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

        let has_scopes: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('tokens') WHERE name = 'scopes'",
            [],
            |row| row.get(0),
        )?;

        if !has_scopes {
            conn.execute_batch(
                "ALTER TABLE tokens ADD COLUMN scopes TEXT NOT NULL DEFAULT 'read,write';",
            )?;
        }

        let _ = conn.execute_batch(
            "ALTER TABLE repos ADD COLUMN visibility TEXT NOT NULL DEFAULT 'private'",
        );

        Ok(())
    }

    // === Repos ===

    /// Ensure a repo exists. Returns true if it was newly created.
    pub fn ensure_repo(&self, repo_id: &str) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT repo_id FROM repos ORDER BY repo_id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    pub fn repo_count(&self) -> Result<u64, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM repos", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    pub fn total_patch_count(&self) -> Result<u64, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM patches", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    pub fn total_blob_stats(&self) -> Result<(u64, u64), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM blobs", [], |row| row.get(0))?;
        let size: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(data)), 0) FROM blobs",
            [],
            |row| row.get(0),
        )?;
        Ok((count as u64, size as u64))
    }

    pub fn user_count(&self) -> Result<u64, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// Check if a repo exists.
    pub fn repo_exists(&self, repo_id: &str) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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

    /// Store multiple patches in a single transaction. Returns the number of newly inserted patches.
    pub fn insert_patches_batch(
        &self,
        repo_id: &str,
        patches: &[&PatchProto],
    ) -> Result<usize, StorageError> {
        if patches.is_empty() {
            return Ok(0);
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO patches (repo_id, patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for patch in patches {
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
                stmt.execute(params![
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
                ])?;
            }
            drop(stmt);
        }
        let total_inserted = tx.changes() as usize;
        tx.commit()?;
        Ok(total_inserted)
    }

    /// Store multiple patches in a single transaction and return indices of already-existing patches.
    pub fn insert_patches_batch_with_existing(
        &self,
        repo_id: &str,
        patches: &[PatchProto],
    ) -> Result<Vec<usize>, StorageError> {
        if patches.is_empty() {
            return Ok(vec![]);
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let tx = conn.unchecked_transaction()?;
        let mut existing_indices = Vec::new();
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO patches (repo_id, patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for (i, patch) in patches.iter().enumerate() {
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
                stmt.execute(params![
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
                ])?;
                if tx.changes() == 0 {
                    existing_indices.push(i);
                }
            }
            drop(stmt);
        }
        tx.commit()?;
        Ok(existing_indices)
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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

    /// Get patches for a repo with pagination support.
    /// Returns at most `limit` patches starting from `offset`, ordered by timestamp.
    pub fn get_all_patches(
        &self,
        repo_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<PatchProto>, StorageError> {
        let effective_limit = limit.min(self.max_page_size).max(1);
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp
             FROM patches WHERE repo_id = ?1 ORDER BY timestamp ASC, patch_id ASC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(
            params![repo_id, effective_limit as i64, offset as i64],
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
        )?;

        let mut patches = Vec::new();
        for row in rows {
            patches.push(row?);
        }
        Ok(patches)
    }

    /// Get all patches for a repo without pagination limit.
    /// Used internally by push/pull handlers that need the full patch set.
    /// Prefer `get_all_patches()` with pagination for user-facing APIs.
    pub fn get_all_patches_unbounded(
        &self,
        repo_id: &str,
    ) -> Result<Vec<PatchProto>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp
             FROM patches WHERE repo_id = ?1 ORDER BY timestamp ASC, patch_id ASC",
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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

        let placeholders: Vec<String> = hashes.iter().map(|_| "?".to_owned()).collect();
        let sql = format!(
            "SELECT blob_hash, data FROM blobs WHERE repo_id = ?1 AND blob_hash IN ({})",
            placeholders.join(",")
        );

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(repo_id.to_owned()));
        for h in hashes {
            params_vec.push(Box::new(h.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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

    /// Check if `ancestor_id` is an ancestor of `descendant_id` using a recursive CTE.
    /// Replaces the old N+1 per-hop approach with a single SQL query.
    pub fn is_ancestor(
        &self,
        repo_id: &str,
        ancestor_id: &str,
        descendant_id: &str,
    ) -> Result<bool, StorageError> {
        if ancestor_id == descendant_id {
            return Ok(true);
        }

        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;

        // SQLite recursive CTEs require parent_id to be a scalar column,
        // but our schema stores parent_ids as a JSON array. Use batched
        // application-level BFS instead.
        drop(conn);

        // Batch approach: load all reachable patches in batches
        self.is_ancestor_batched(repo_id, ancestor_id, descendant_id)
    }

    /// Batched ancestor check: loads patches in chunks to minimize SQL round-trips.
    fn is_ancestor_batched(
        &self,
        repo_id: &str,
        ancestor_id: &str,
        descendant_id: &str,
    ) -> Result<bool, StorageError> {
        let mut visited = std::collections::HashSet::new();
        let mut frontier = vec![descendant_id.to_owned()];

        while !frontier.is_empty() {
            // Deduplicate frontier
            frontier.sort();
            frontier.dedup();
            frontier.retain(|id| visited.insert(id.clone()));

            if frontier.is_empty() {
                break;
            }

            // Load all patches in this batch in a single query
            let patches = self.get_patches_batch(repo_id, &frontier)?;
            frontier.clear();

            for patch in patches.values() {
                for parent in &patch.parent_ids {
                    let parent_hex = &parent.value;
                    if parent_hex == ancestor_id {
                        return Ok(true);
                    }
                    if !visited.contains(parent_hex) {
                        frontier.push(parent_hex.clone());
                    }
                }
            }

            // Safety: limit traversal depth to prevent pathological cases
            if visited.len() > 100_000 {
                return Ok(false);
            }
        }
        Ok(false)
    }

    /// Fetch multiple patches by ID in a single SQL query.
    /// Returns a HashMap keyed by patch_id for O(1) lookup.
    fn get_patches_batch(
        &self,
        repo_id: &str,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, PatchProto>, StorageError> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;

        // Build a query with parameterized IN clause
        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect();
        let sql = format!(
            "SELECT patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp
             FROM patches WHERE repo_id = ?1 AND patch_id IN ({})",
            placeholders.join(", ")
        );

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(repo_id.to_owned())];
        for id in ids {
            params.push(Box::new(id.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
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

            Ok((
                id_hex.clone(),
                PatchProto {
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
                },
            ))
        })?;

        let mut result = std::collections::HashMap::with_capacity(ids.len());
        for row in rows {
            let (id_hex, patch) = row?;
            result.insert(id_hex, patch);
        }
        Ok(result)
    }

    // === Branch Protection ===

    pub fn protect_branch(&self, repo_id: &str, branch_name: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM branch_protection WHERE repo_id = ?1 AND branch_name = ?2",
            params![repo_id, branch_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // === Issues ===

    pub fn create_issue(
        &self,
        repo_id: &str,
        title: &str,
        body: &str,
        author: &str,
        labels: &[String],
    ) -> Result<Issue, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let labels_json = serde_json::to_string(labels).unwrap_or_else(|_| "[]".to_string());
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO issues (repo_id, title, body, status, author, labels, created_at, updated_at) VALUES (?1, ?2, ?3, 'open', ?4, ?5, ?6, ?6)",
            params![repo_id, title, body, author, labels_json, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Issue {
            id,
            repo_id: repo_id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            status: "open".to_string(),
            author: author.to_string(),
            assignee: None,
            labels: labels.to_vec(),
            created_at: now,
            updated_at: now,
            closed_at: None,
        })
    }

    pub fn list_issues(
        &self,
        repo_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<Issue>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let sql = if status.is_some() {
            "SELECT id, repo_id, title, body, status, author, assignee, labels, created_at, updated_at, closed_at FROM issues WHERE repo_id = ?1 AND status = ?2 ORDER BY updated_at DESC"
        } else {
            "SELECT id, repo_id, title, body, status, author, assignee, labels, created_at, updated_at, closed_at FROM issues WHERE repo_id = ?1 ORDER BY updated_at DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows: Result<Vec<Issue>, rusqlite::Error> = if let Some(s) = status {
            stmt.query_map(params![repo_id, s], issue_from_row)?
                .collect()
        } else {
            stmt.query_map(params![repo_id], issue_from_row)?.collect()
        };
        rows.map_err(StorageError::Database)
    }

    pub fn get_issue(&self, issue_id: i64) -> Result<Option<Issue>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, repo_id, title, body, status, author, assignee, labels, created_at, updated_at, closed_at FROM issues WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![issue_id], issue_from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn update_issue_status(&self, issue_id: i64, status: &str) -> Result<(), StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let closed_at = if status == "closed" { Some(now) } else { None };
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "UPDATE issues SET status = ?1, updated_at = ?2, closed_at = ?3 WHERE id = ?4",
            params![status, now, closed_at, issue_id],
        )?;
        Ok(())
    }

    pub fn add_issue_comment(
        &self,
        issue_id: i64,
        author: &str,
        body: &str,
    ) -> Result<IssueComment, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO issue_comments (issue_id, author, body, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![issue_id, author, body, now],
        )?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE issues SET updated_at = ?1 WHERE id = ?2",
            params![now, issue_id],
        )?;
        Ok(IssueComment {
            id,
            issue_id,
            author: author.to_string(),
            body: body.to_string(),
            created_at: now,
        })
    }

    pub fn list_issue_comments(&self, issue_id: i64) -> Result<Vec<IssueComment>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, issue_id, author, body, created_at FROM issue_comments WHERE issue_id = ?1 ORDER BY created_at ASC")?;
        let rows = stmt.query_map(params![issue_id], |row| {
            Ok(IssueComment {
                id: row.get(0)?,
                issue_id: row.get(1)?,
                author: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    // === Pull Requests ===

    pub fn create_pull_request(
        &self,
        repo_id: &str,
        title: &str,
        body: &str,
        author: &str,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<PullRequestRecord, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO pull_requests (repo_id, title, body, status, author, source_branch, target_branch, created_at, updated_at) VALUES (?1, ?2, ?3, 'open', ?4, ?5, ?6, ?7, ?7)",
            params![repo_id, title, body, author, source_branch, target_branch, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(PullRequestRecord {
            id,
            repo_id: repo_id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            status: "open".to_string(),
            author: author.to_string(),
            source_branch: source_branch.to_string(),
            target_branch: target_branch.to_string(),
            created_at: now,
            updated_at: now,
            merged_at: None,
            merged_by: None,
        })
    }

    pub fn list_pull_requests(
        &self,
        repo_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<PullRequestRecord>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let sql = if status.is_some() {
            "SELECT id, repo_id, title, body, status, author, source_branch, target_branch, created_at, updated_at, merged_at, merged_by FROM pull_requests WHERE repo_id = ?1 AND status = ?2 ORDER BY updated_at DESC"
        } else {
            "SELECT id, repo_id, title, body, status, author, source_branch, target_branch, created_at, updated_at, merged_at, merged_by FROM pull_requests WHERE repo_id = ?1 ORDER BY updated_at DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows: Result<Vec<PullRequestRecord>, rusqlite::Error> = if let Some(s) = status {
            stmt.query_map(params![repo_id, s], |row| {
                Ok(PullRequestRecord {
                    id: row.get(0)?,
                    repo_id: row.get(1)?,
                    title: row.get(2)?,
                    body: row.get(3)?,
                    status: row.get(4)?,
                    author: row.get(5)?,
                    source_branch: row.get(6)?,
                    target_branch: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    merged_at: row.get(10)?,
                    merged_by: row.get(11)?,
                })
            })?
            .collect()
        } else {
            stmt.query_map(params![repo_id], |row| {
                Ok(PullRequestRecord {
                    id: row.get(0)?,
                    repo_id: row.get(1)?,
                    title: row.get(2)?,
                    body: row.get(3)?,
                    status: row.get(4)?,
                    author: row.get(5)?,
                    source_branch: row.get(6)?,
                    target_branch: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    merged_at: row.get(10)?,
                    merged_by: row.get(11)?,
                })
            })?
            .collect()
        };
        rows.map_err(StorageError::Database)
    }

    pub fn get_pull_request(&self, pr_id: i64) -> Result<Option<PullRequestRecord>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, repo_id, title, body, status, author, source_branch, target_branch, created_at, updated_at, merged_at, merged_by FROM pull_requests WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![pr_id], |row| {
            Ok(PullRequestRecord {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                status: row.get(4)?,
                author: row.get(5)?,
                source_branch: row.get(6)?,
                target_branch: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
                merged_at: row.get(10)?,
                merged_by: row.get(11)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn merge_pull_request(&self, pr_id: i64, merged_by: &str) -> Result<(), StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "UPDATE pull_requests SET status = 'merged', merged_at = ?1, merged_by = ?2, updated_at = ?1 WHERE id = ?3",
            params![now, merged_by, pr_id],
        )?;
        Ok(())
    }

    pub fn close_pull_request(&self, pr_id: i64) -> Result<(), StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "UPDATE pull_requests SET status = 'closed', updated_at = ?1 WHERE id = ?2",
            params![now, pr_id],
        )?;
        Ok(())
    }

    pub fn add_pr_review(
        &self,
        pr_id: i64,
        reviewer: &str,
        verdict: &str,
        body: &str,
    ) -> Result<PrReview, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO pr_reviews (pr_id, reviewer, verdict, body, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![pr_id, reviewer, verdict, body, now],
        )?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE pull_requests SET updated_at = ?1 WHERE id = ?2",
            params![now, pr_id],
        )?;
        Ok(PrReview {
            id,
            pr_id,
            reviewer: reviewer.to_string(),
            verdict: verdict.to_string(),
            body: body.to_string(),
            created_at: now,
        })
    }

    pub fn list_pr_reviews(&self, pr_id: i64) -> Result<Vec<PrReview>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, pr_id, reviewer, verdict, body, created_at FROM pr_reviews WHERE pr_id = ?1 ORDER BY created_at ASC")?;
        let rows = stmt.query_map(params![pr_id], |row| {
            Ok(PrReview {
                id: row.get(0)?,
                pr_id: row.get(1)?,
                reviewer: row.get(2)?,
                verdict: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    // === Code Search (Trigram Index) ===

    pub fn index_blob(
        &self,
        hash: &str,
        repo_id: &str,
        data: &[u8],
        path: Option<&str>,
    ) -> Result<(), StorageError> {
        let is_text = data.iter().all(|&b| b < 128);
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO blob_metadata (hash, repo_id, size, is_text, path) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![hash, repo_id, data.len() as i64, is_text as i32, path],
        )?;
        if is_text && data.len() >= 3 {
            let mut seen = std::collections::HashSet::new();
            for window in data.windows(3) {
                let tg = std::str::from_utf8(window).unwrap_or("");
                if !tg.is_empty() && seen.insert(tg.to_string()) {
                    conn.execute(
                        "INSERT OR IGNORE INTO blob_trigrams (trigram, blob_hash, repo_id) VALUES (?1, ?2, ?3)",
                        params![tg, hash, repo_id],
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn search_code(
        &self,
        repo_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<CodeSearchResult>, StorageError> {
        if query.len() < 3 {
            return Ok(vec![]);
        }
        let trigrams: Vec<String> = query
            .as_bytes()
            .windows(3)
            .filter_map(|w| std::str::from_utf8(w).ok())
            .map(|s| s.to_string())
            .collect();
        if trigrams.is_empty() {
            return Ok(vec![]);
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let placeholders: Vec<String> = trigrams
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect();
        let sql = format!(
            "SELECT blob_hash, repo_id, COUNT(DISTINCT trigram) as match_count \
             FROM blob_trigrams WHERE repo_id = ?1 AND trigram IN ({}) \
             GROUP BY blob_hash ORDER BY match_count DESC LIMIT {}",
            placeholders.join(","),
            limit
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(repo_id.to_string())];
        for tg in &trigrams {
            params.push(Box::new(tg.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as usize,
            ))
        })?;
        let mut results = Vec::new();
        for row in rows {
            let (blob_hash, rid, match_count) = row?;
            let path: Option<String> = conn
                .query_row(
                    "SELECT path FROM blob_metadata WHERE hash = ?1",
                    params![blob_hash],
                    |r| r.get(0),
                )
                .unwrap_or(None);
            let snippet: String = conn
                .query_row(
                    "SELECT substr(data, 1, 200) FROM blobs WHERE blob_hash = ?1 AND repo_id = ?2",
                    params![blob_hash, rid],
                    |r| {
                        let d: Vec<u8> = r.get(0)?;
                        Ok(String::from_utf8_lossy(&d).to_string())
                    },
                )
                .unwrap_or_default();
            results.push(CodeSearchResult {
                blob_hash,
                repo_id: rid,
                path,
                match_count,
                snippet,
            });
        }
        Ok(results)
    }

    // === Organizations ===

    pub fn create_org(
        &self,
        name: &str,
        display_name: &str,
        description: &str,
    ) -> Result<Organization, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("INSERT INTO organizations (name, display_name, description, created_at) VALUES (?1, ?2, ?3, ?4)", params![name, display_name, description, now])?;
        let id = conn.last_insert_rowid();
        Ok(Organization {
            id,
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            created_at: now,
        })
    }

    pub fn list_orgs(&self) -> Result<Vec<Organization>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, name, display_name, description, created_at FROM organizations ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(Organization {
                id: row.get(0)?,
                name: row.get(1)?,
                display_name: row.get(2)?,
                description: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    pub fn get_org(&self, id: i64) -> Result<Option<Organization>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row("SELECT id, name, display_name, description, created_at FROM organizations WHERE id = ?1", params![id], |row| {
            Ok(Organization { id: row.get(0)?, name: row.get(1)?, display_name: row.get(2)?, description: row.get(3)?, created_at: row.get(4)? })
        });
        match result {
            Ok(org) => Ok(Some(org)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn delete_org(&self, id: i64) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("DELETE FROM organizations WHERE id = ?1", params![id])?;
        Ok(())
    }

    // === Teams ===

    pub fn create_team(
        &self,
        org_id: i64,
        name: &str,
        permission: &str,
    ) -> Result<Team, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO teams (org_id, name, permission, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![org_id, name, permission, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Team {
            id,
            org_id,
            name: name.to_string(),
            permission: permission.to_string(),
            created_at: now,
        })
    }

    pub fn list_teams(&self, org_id: i64) -> Result<Vec<Team>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, org_id, name, permission, created_at FROM teams WHERE org_id = ?1 ORDER BY name")?;
        let rows = stmt.query_map(params![org_id], |row| {
            Ok(Team {
                id: row.get(0)?,
                org_id: row.get(1)?,
                name: row.get(2)?,
                permission: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    pub fn delete_team(&self, id: i64) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("DELETE FROM teams WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_team_member(&self, team_id: i64, username: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT OR IGNORE INTO team_members (team_id, username) VALUES (?1, ?2)",
            params![team_id, username],
        )?;
        Ok(())
    }

    pub fn remove_team_member(&self, team_id: i64, username: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "DELETE FROM team_members WHERE team_id = ?1 AND username = ?2",
            params![team_id, username],
        )?;
        Ok(())
    }

    pub fn list_team_members(&self, team_id: i64) -> Result<Vec<String>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT username FROM team_members WHERE team_id = ?1")?;
        let rows = stmt.query_map(params![team_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    pub fn add_team_repo(&self, team_id: i64, repo_id: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT OR IGNORE INTO team_repos (team_id, repo_id) VALUES (?1, ?2)",
            params![team_id, repo_id],
        )?;
        Ok(())
    }

    pub fn remove_team_repo(&self, team_id: i64, repo_id: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "DELETE FROM team_repos WHERE team_id = ?1 AND repo_id = ?2",
            params![team_id, repo_id],
        )?;
        Ok(())
    }

    pub fn list_team_repos(&self, team_id: i64) -> Result<Vec<String>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT repo_id FROM team_repos WHERE team_id = ?1")?;
        let rows = stmt.query_map(params![team_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    // === Repo Visibility ===

    pub fn update_repo_visibility(
        &self,
        repo_id: &str,
        visibility: &str,
    ) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "UPDATE repos SET visibility = ?1 WHERE repo_id = ?2",
            params![visibility, repo_id],
        )?;
        Ok(())
    }

    pub fn get_repo_visibility(&self, repo_id: &str) -> Result<String, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row(
            "SELECT visibility FROM repos WHERE repo_id = ?1",
            params![repo_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(v) => Ok(v),
            Err(_) => Ok("private".to_string()),
        }
    }

    // === Forks ===

    pub fn create_fork(
        &self,
        fork_repo_id: &str,
        parent_repo_id: &str,
    ) -> Result<(), StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("INSERT OR IGNORE INTO fork_relationships (fork_repo_id, parent_repo_id, forked_at) VALUES (?1, ?2, ?3)", params![fork_repo_id, parent_repo_id, now])?;
        Ok(())
    }

    pub fn get_fork_parent(&self, repo_id: &str) -> Result<Option<String>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row(
            "SELECT parent_repo_id FROM fork_relationships WHERE fork_repo_id = ?1",
            params![repo_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn list_forks(&self, parent_repo_id: &str) -> Result<Vec<String>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt =
            conn.prepare("SELECT fork_repo_id FROM fork_relationships WHERE parent_repo_id = ?1")?;
        let rows = stmt.query_map(params![parent_repo_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    // === Wiki ===

    pub fn upsert_wiki_page(
        &self,
        repo_id: &str,
        title: &str,
        content: &str,
        author: &str,
    ) -> Result<WikiPage, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM wiki_pages WHERE repo_id = ?1 AND title = ?2",
                params![repo_id, title],
                |row| row.get(0),
            )
            .ok();
        if let Some(page_id) = existing_id {
            conn.execute("INSERT INTO wiki_history (page_id, content, author, edited_at) VALUES (?1, ?2, ?3, ?4)",
                params![page_id, content, author, now])?;
            conn.execute(
                "UPDATE wiki_pages SET content = ?1, author = ?2, updated_at = ?3 WHERE id = ?4",
                params![content, author, now, page_id],
            )?;
            Ok(WikiPage {
                id: page_id,
                repo_id: repo_id.to_string(),
                title: title.to_string(),
                content: content.to_string(),
                author: author.to_string(),
                updated_at: now,
            })
        } else {
            conn.execute("INSERT INTO wiki_pages (repo_id, title, content, author, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![repo_id, title, content, author, now])?;
            let id = conn.last_insert_rowid();
            Ok(WikiPage {
                id,
                repo_id: repo_id.to_string(),
                title: title.to_string(),
                content: content.to_string(),
                author: author.to_string(),
                updated_at: now,
            })
        }
    }

    pub fn get_wiki_page(
        &self,
        repo_id: &str,
        title: &str,
    ) -> Result<Option<WikiPage>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, repo_id, title, content, author, updated_at FROM wiki_pages WHERE repo_id = ?1 AND title = ?2",
            params![repo_id, title],
            |row| Ok(WikiPage { id: row.get(0)?, repo_id: row.get(1)?, title: row.get(2)?, content: row.get(3)?, author: row.get(4)?, updated_at: row.get(5)? }),
        );
        match result {
            Ok(page) => Ok(Some(page)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn list_wiki_pages(&self, repo_id: &str) -> Result<Vec<WikiPage>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, repo_id, title, content, author, updated_at FROM wiki_pages WHERE repo_id = ?1 ORDER BY title")?;
        let rows = stmt.query_map(params![repo_id], |row| {
            Ok(WikiPage {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                author: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    pub fn get_wiki_history(
        &self,
        repo_id: &str,
        title: &str,
    ) -> Result<Vec<(i64, String, String, i64)>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let page_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM wiki_pages WHERE repo_id = ?1 AND title = ?2",
                params![repo_id, title],
                |row| row.get(0),
            )
            .ok();
        let page_id = match page_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };
        let mut stmt = conn.prepare("SELECT id, content, author, edited_at FROM wiki_history WHERE page_id = ?1 ORDER BY edited_at DESC")?;
        let rows = stmt.query_map(params![page_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    // === Releases ===

    pub fn create_release(
        &self,
        repo_id: &str,
        tag: &str,
        title: &str,
        body: &str,
        author: &str,
        prerelease: bool,
    ) -> Result<Release, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("INSERT INTO releases (repo_id, tag, title, body, author, prerelease, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![repo_id, tag, title, body, author, prerelease as i32, now])?;
        let id = conn.last_insert_rowid();
        Ok(Release {
            id,
            repo_id: repo_id.to_string(),
            tag: tag.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            author: author.to_string(),
            prerelease,
            created_at: now,
        })
    }

    pub fn list_releases(&self, repo_id: &str) -> Result<Vec<Release>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, repo_id, tag, title, body, author, prerelease, created_at FROM releases WHERE repo_id = ?1 ORDER BY created_at DESC")?;
        let rows = stmt.query_map(params![repo_id], |row| {
            Ok(Release {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                tag: row.get(2)?,
                title: row.get(3)?,
                body: row.get(4)?,
                author: row.get(5)?,
                prerelease: row.get::<_, i32>(6)? != 0,
                created_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    pub fn get_release(&self, id: i64) -> Result<Option<Release>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row("SELECT id, repo_id, tag, title, body, author, prerelease, created_at FROM releases WHERE id = ?1", params![id], |row| {
            Ok(Release { id: row.get(0)?, repo_id: row.get(1)?, tag: row.get(2)?, title: row.get(3)?, body: row.get(4)?, author: row.get(5)?, prerelease: row.get::<_, i32>(6)? != 0, created_at: row.get(7)? })
        });
        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn delete_release(&self, id: i64) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("DELETE FROM releases WHERE id = ?1", params![id])?;
        Ok(())
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
        scopes: &str,
    ) -> Result<(), StorageError> {
        let token_hash = format!("{:x}", sha2::Sha256::digest(token.as_bytes()));
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO tokens (token, created_at, description, expires_at, scopes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![token_hash, created_at as i64, description, expires_at, scopes],
        )?;
        Ok(())
    }

    pub fn verify_token(&self, token: &str) -> Result<bool, StorageError> {
        let token_hash = format!("{:x}", sha2::Sha256::digest(token.as_bytes()));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tokens WHERE token = ?1 AND expires_at > ?2",
            params![token_hash, now],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn get_token_scopes(&self, token: &str) -> Result<Vec<String>, StorageError> {
        let token_hash = format!("{:x}", sha2::Sha256::digest(token.as_bytes()));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let scopes_str: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT scopes FROM tokens WHERE token = ?1 AND expires_at > ?2",
            params![token_hash, now],
            |row| row.get(0),
        );
        match scopes_str {
            Ok(s) => Ok(s
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(vec![]),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub fn has_tokens(&self) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    pub fn has_users(&self) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
        let token_hash = format!("{:x}", sha2::Sha256::digest(api_token.as_bytes()));
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO users (username, display_name, role, api_token, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![username, display_name, role, token_hash, created_at],
        )?;
        Ok(())
    }

    pub fn get_user(&self, username: &str) -> Result<Option<UserInfo>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
        let token_hash = format!("{:x}", sha2::Sha256::digest(token.as_bytes()));
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row(
            "SELECT username, display_name, role, api_token, created_at FROM users WHERE api_token = ?1",
            params![token_hash],
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("DELETE FROM users WHERE username = ?1", params![username])?;
        Ok(())
    }

    pub fn delete_repo(&self, repo_id: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("DELETE FROM mirrors WHERE id = ?1", params![mirror_id])?;
        Ok(())
    }

    pub fn search_repos(&self, query: &str) -> Result<Vec<String>, StorageError> {
        let pattern = format!("%{query}%");
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
        // BFS to discover all reachable patch IDs, then load in batch
        let mut visited = std::collections::HashSet::new();
        let mut frontier = vec![tip_id.to_owned()];

        while !frontier.is_empty() {
            frontier.sort();
            frontier.dedup();
            frontier.retain(|id| visited.insert(id.clone()));
            if frontier.is_empty() {
                break;
            }

            let patches = self.get_patches_batch(repo_id, &frontier)?;
            frontier.clear();

            for patch in patches.values() {
                for parent in &patch.parent_ids {
                    if !visited.contains(&parent.value) {
                        frontier.push(parent.value.clone());
                    }
                }
            }

            if visited.len() > 100_000 {
                break;
            }
        }

        // Single batch load of all discovered patches
        let all_ids: Vec<String> = visited.into_iter().collect();
        let patches_map = self.get_patches_batch(repo_id, &all_ids)?;

        // Sort deterministically
        let mut patches: Vec<PatchProto> = patches_map.into_values().collect();
        patches.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then_with(|| a.id.value.cmp(&b.id.value))
        });
        Ok(patches)
    }

    pub fn get_tree_at_branch(
        &self,
        repo_id: &str,
        branch: &str,
    ) -> Result<Vec<crate::types::TreeEntry>, StorageError> {
        use crate::types::TreeEntry;

        let Some(tip_id) = self.get_branch_target(repo_id, branch)? else {
            return Ok(Vec::new());
        };

        let mut patches = self.get_patches_at(repo_id, &tip_id)?;
        patches.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then_with(|| a.id.value.cmp(&b.id.value))
        });

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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute("DELETE FROM replication_peers WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_replication_peers(&self) -> Result<Vec<ReplicationPeer>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO webhooks (id, repo_id, url, events, secret, created_at, active) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                webhook.id,
                webhook.repo_id,
                webhook.url,
                events_json,
                webhook.secret,
                webhook.created_at as i64,
                i32::from(webhook.active),
            ],
        )?;
        Ok(())
    }

    pub fn list_webhooks(&self, repo_id: &str) -> Result<Vec<Webhook>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
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
            rusqlite::Error::QueryReturnedNoRows => StorageError::WebhookNotFound(id.to_owned()),
            e => StorageError::Database(e),
        })
    }

    pub fn delete_webhook(&self, id: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let changes = conn.execute("DELETE FROM webhooks WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(StorageError::WebhookNotFound(id.to_owned()));
        }
        Ok(())
    }

    // === SSO / OIDC Configuration ===

    /// Store an OIDC provider configuration.
    pub fn set_oidc_config(&self, config: &crate::sso::OidcConfig) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let json = serde_json::to_string(config).map_err(|e| {
            StorageError::PoisonedLock(format!("failed to serialize OIDC config: {e}"))
        })?;
        conn.execute(
            "INSERT OR REPLACE INTO sso_providers (provider_name, config_json, updated_at) VALUES (?1, ?2, datetime('now'))",
            params![config.provider_name, json],
        )?;
        Ok(())
    }

    /// Get an OIDC provider configuration by name.
    pub fn get_oidc_config(
        &self,
        provider_name: &str,
    ) -> Result<Option<crate::sso::OidcConfig>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row(
            "SELECT config_json FROM sso_providers WHERE provider_name = ?1",
            params![provider_name],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(json) => {
                let config: crate::sso::OidcConfig = serde_json::from_str(&json).map_err(|e| {
                    StorageError::PoisonedLock(format!("failed to deserialize OIDC config: {e}"))
                })?;
                Ok(Some(config))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// List all configured OIDC providers.
    pub fn list_oidc_configs(&self) -> Result<Vec<crate::sso::OidcConfig>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let mut stmt =
            conn.prepare("SELECT config_json FROM sso_providers ORDER BY provider_name")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut configs = Vec::new();
        for row in rows {
            let json = row?;
            let config: crate::sso::OidcConfig = serde_json::from_str(&json).map_err(|e| {
                StorageError::PoisonedLock(format!("failed to deserialize OIDC config: {e}"))
            })?;
            configs.push(config);
        }
        Ok(configs)
    }

    /// Delete an OIDC provider configuration.
    pub fn delete_oidc_config(&self, provider_name: &str) -> Result<bool, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let affected = conn.execute(
            "DELETE FROM sso_providers WHERE provider_name = ?1",
            params![provider_name],
        )?;
        Ok(affected > 0)
    }

    // === SSO State Management ===

    /// Store an SSO authorization state for CSRF validation.
    ///
    /// Returns `Err` if the state already exists (unlikely collision).
    pub fn store_sso_state(
        &self,
        state: &str,
        provider_name: &str,
        nonce: &str,
    ) -> Result<(), StorageError> {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO sso_states (state, provider_name, nonce, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![state, provider_name, nonce, created_at],
        )?;
        Ok(())
    }

    /// Consume an SSO authorization state.
    ///
    /// Returns the stored provider name and nonce if the state is valid.
    /// The state is deleted after retrieval (one-time use).
    /// Returns `None` if the state does not exist or has expired (10 minutes).
    pub fn consume_sso_state(&self, state: &str) -> Result<Option<(String, String)>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row(
            "SELECT provider_name, nonce, created_at FROM sso_states WHERE state = ?1",
            params![state],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        );
        match result {
            Ok((provider_name, nonce, created_at)) => {
                // Delete the state (one-time use).
                if let Err(e) =
                    conn.execute("DELETE FROM sso_states WHERE state = ?1", params![state])
                {
                    tracing::warn!("failed to clean up SSO state: {e}");
                }
                // Check expiry (10 minutes).
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let max_age = 10 * 60; // 10 minutes
                if now - created_at > max_age {
                    return Ok(None);
                }
                Ok(Some((provider_name, nonce)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Clean up expired SSO states older than 10 minutes.
    pub fn cleanup_expired_sso_states(&self) -> Result<usize, StorageError> {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            - (10 * 60);
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let affected = conn.execute(
            "DELETE FROM sso_states WHERE created_at < ?1",
            params![cutoff],
        )?;
        Ok(affected)
    }

    /// Look up a local username by SSO provider + subject.
    pub fn get_sso_user_mapping(
        &self,
        provider_name: &str,
        provider_sub: &str,
    ) -> Result<Option<String>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let result = conn.query_row(
            "SELECT username FROM sso_user_mappings WHERE provider_name = ?1 AND provider_sub = ?2",
            params![provider_name, provider_sub],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(username) => Ok(Some(username)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Create or update a user from SSO authentication.
    ///
    /// If a user already exists with the same username, updates their display name.
    /// If the user doesn't exist, creates a new one with the "member" role.
    /// Also creates/updates the SSO user mapping.
    pub fn upsert_sso_user(
        &self,
        provider_name: &str,
        provider_sub: &str,
        username: &str,
        display_name: &str,
    ) -> Result<String, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Upsert the user (create if not exists, update display name if exists).
        conn.execute(
            "INSERT INTO users (username, display_name, role, api_token, created_at) VALUES (?1, ?2, 'member', NULL, ?3)
             ON CONFLICT(username) DO UPDATE SET display_name = ?2",
            params![username, display_name, created_at],
        )?;

        // Upsert the SSO mapping.
        conn.execute(
            "INSERT INTO sso_user_mappings (provider_name, provider_sub, username, linked_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(provider_name, provider_sub) DO UPDATE SET username = ?3, linked_at = ?4",
            params![provider_name, provider_sub, username, created_at],
        )?;

        Ok(username.to_owned())
    }

    /// Look up a local user by SSO provider + email.
    ///
    /// Falls back to searching by username if the email matches a username.
    pub fn get_user_by_email(&self, email: &str) -> Result<Option<UserInfo>, StorageError> {
        self.get_user(email)
    }

    /// Update a user's API token.
    pub fn update_user_token(&self, username: &str, token_hash: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "UPDATE users SET api_token = ?1 WHERE username = ?2",
            params![token_hash, username],
        )?;
        Ok(())
    }

    // === Audit Logging ===

    /// Write an audit log entry.
    #[allow(clippy::too_many_arguments)]
    pub fn write_audit_entry(
        &self,
        actor: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        status: &str,
        details: &str,
        request_id: &str,
        client_ip: &str,
    ) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        conn.execute(
            "INSERT INTO audit_log (actor, action, resource_type, resource_id, status, details, request_id, client_ip) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![actor, action, resource_type, resource_id, status, details, request_id, client_ip],
        )?;
        Ok(())
    }

    /// Query audit log entries with optional filters.
    pub fn query_audit_log(
        &self,
        actor: Option<&str>,
        action: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditEntry>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::PoisonedLock(e.to_string()))?;
        let effective_limit: i64 = limit.clamp(1, 1000) as i64;
        let effective_offset: i64 = offset as i64;

        let mut sql = String::from(
            "SELECT id, timestamp, actor, action, resource_type, resource_id, status, details, request_id, client_ip FROM audit_log WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(a) = actor {
            sql.push_str(" AND actor = ?");
            param_values.push(Box::new(a.to_owned()));
        }
        if let Some(a) = action {
            sql.push_str(" AND action = ?");
            param_values.push(Box::new(a.to_owned()));
        }
        sql.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
        param_values.push(Box::new(effective_limit));
        param_values.push(Box::new(effective_offset));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|b| b.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(AuditEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                actor: row.get(2)?,
                action: row.get(3)?,
                resource_type: row.get(4)?,
                resource_id: row.get(5)?,
                status: row.get(6)?,
                details: row.get(7)?,
                request_id: row.get(8)?,
                client_ip: row.get(9)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
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

/// A single audit log entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub status: String,
    pub details: String,
    pub request_id: String,
    pub client_ip: String,
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

    #[cfg(test)]
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
        let all_patches = store2.get_all_patches("test-repo", 0, 10000).unwrap();
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
