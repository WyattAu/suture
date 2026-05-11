use rusqlite::Connection;
use std::path::Path;
use thiserror::Error;

use crate::storage::{HubStorage, StorageError};

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("invalid backup manifest: {0}")]
    InvalidManifest(String),

    #[error("lock poisoned: {0}")]
    PoisonedLock(String),

    #[error("backup directory already exists: {0}")]
    AlreadyExists(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupManifest {
    pub version: u32,
    pub timestamp: String,
    pub repo_count: u64,
    pub patch_count: u64,
    pub blob_count: u64,
    pub suture_hub_version: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RestoreStats {
    pub repos_restored: u64,
    pub patches_restored: u64,
    pub blobs_restored: u64,
}

const BACKUP_VERSION: u32 = 1;
const MANIFEST_FILE: &str = "manifest.json";
const METADATA_DB: &str = "metadata.db";

pub fn create_backup(
    storage: &HubStorage,
    output_dir: &Path,
) -> Result<BackupManifest, BackupError> {
    if output_dir.exists() {
        return Err(BackupError::AlreadyExists(output_dir.display().to_string()));
    }
    std::fs::create_dir_all(output_dir)?;

    let backup_db_path = output_dir.join(METADATA_DB);
    let mut dst = Connection::open(&backup_db_path)?;
    dst.execute_batch("PRAGMA journal_mode=WAL;")?;

    backup_sqlite(storage, &mut dst)?;
    let manifest = build_manifest(&dst)?;

    let manifest_path = output_dir.join(MANIFEST_FILE);
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| BackupError::Io(std::io::Error::other(e)))?;
    std::fs::write(&manifest_path, manifest_json)?;

    Ok(manifest)
}

fn backup_sqlite(storage: &HubStorage, dst: &mut Connection) -> Result<(), BackupError> {
    let src_conn = storage
        .conn()
        .lock()
        .map_err(|e| BackupError::PoisonedLock(e.to_string()))?;
    let backup = rusqlite::backup::Backup::new(&src_conn, dst)?;
    backup.step(-1)?;
    Ok(())
}

fn build_manifest(conn: &Connection) -> Result<BackupManifest, BackupError> {
    let repo_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM repos", [], |row| row.get(0))
        .unwrap_or(0);
    let patch_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM patches", [], |row| row.get(0))
        .unwrap_or(0);
    let blob_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM blobs", [], |row| row.get(0))
        .unwrap_or(0);

    let now = chrono::Utc::now().to_rfc3339();

    Ok(BackupManifest {
        version: BACKUP_VERSION,
        timestamp: now,
        repo_count: repo_count as u64,
        patch_count: patch_count as u64,
        blob_count: blob_count as u64,
        suture_hub_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub fn restore_backup(
    storage: &mut HubStorage,
    backup_dir: &Path,
) -> Result<RestoreStats, BackupError> {
    let manifest_path = backup_dir.join(MANIFEST_FILE);
    if !manifest_path.exists() {
        return Err(BackupError::InvalidManifest(
            "manifest.json not found in backup directory".to_string(),
        ));
    }
    let manifest_json = std::fs::read_to_string(&manifest_path)?;
    let manifest: BackupManifest = serde_json::from_str(&manifest_json)
        .map_err(|e| BackupError::InvalidManifest(format!("invalid manifest JSON: {e}")))?;

    if manifest.version != BACKUP_VERSION {
        return Err(BackupError::InvalidManifest(format!(
            "unsupported backup version {} (expected {})",
            manifest.version, BACKUP_VERSION
        )));
    }

    let backup_db_path = backup_dir.join(METADATA_DB);
    if !backup_db_path.exists() {
        return Err(BackupError::InvalidManifest(
            "metadata.db not found in backup directory".to_string(),
        ));
    }

    let stats = restore_from_db(storage, &backup_db_path)?;

    if stats.repos_restored != manifest.repo_count {
        return Err(BackupError::InvalidManifest(format!(
            "repo count mismatch: manifest says {}, but restored {}",
            manifest.repo_count, stats.repos_restored
        )));
    }

    Ok(stats)
}

fn restore_from_db(
    storage: &mut HubStorage,
    backup_db_path: &Path,
) -> Result<RestoreStats, BackupError> {
    let backup_conn = Connection::open(backup_db_path)?;

    let repos: Vec<String> = {
        let mut stmt = backup_conn.prepare("SELECT repo_id FROM repos ORDER BY repo_id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut repos_restored = 0u64;
    let mut patches_restored = 0u64;
    let mut blobs_restored = 0u64;

    for repo_id in &repos {
        storage.ensure_repo(repo_id)?;
        repos_restored += 1;

        let mut stmt = backup_conn.prepare(
            "SELECT patch_id, operation_type, touch_set, target_path, payload, parent_ids, author, message, timestamp
             FROM patches WHERE repo_id = ?1 ORDER BY timestamp ASC, patch_id ASC",
        )?;
        let patch_rows = stmt.query_map(rusqlite::params![repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })?;

        for row in patch_rows {
            let (
                id_hex,
                operation_type,
                touch_set_json,
                target_path,
                payload,
                parent_ids_json,
                author,
                message,
                timestamp,
            ) = row?;
            let touch_set: Vec<String> = serde_json::from_str(&touch_set_json).unwrap_or_default();
            let parent_ids: Vec<String> =
                serde_json::from_str(&parent_ids_json).unwrap_or_default();

            let patch = crate::types::PatchProto {
                id: crate::types::HashProto { value: id_hex },
                operation_type,
                touch_set,
                target_path,
                payload,
                parent_ids: parent_ids
                    .into_iter()
                    .map(|h| crate::types::HashProto { value: h })
                    .collect(),
                author,
                message,
                timestamp: timestamp as u64,
            };
            if storage.insert_patch(repo_id, &patch)? {
                patches_restored += 1;
            }
        }

        let mut branch_stmt =
            backup_conn.prepare("SELECT name, target_patch_id FROM branches WHERE repo_id = ?1")?;
        let branch_rows = branch_stmt.query_map(rusqlite::params![repo_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in branch_rows {
            let (name, target_id) = row?;
            storage.set_branch(repo_id, &name, &target_id)?;
        }

        let mut blob_stmt =
            backup_conn.prepare("SELECT blob_hash, data FROM blobs WHERE repo_id = ?1")?;
        let blob_rows = blob_stmt.query_map(rusqlite::params![repo_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        for row in blob_rows {
            let (hash, data) = row?;
            storage.store_blob(repo_id, &hash, &data)?;
            blobs_restored += 1;
        }
    }

    Ok(RestoreStats {
        repos_restored,
        patches_restored,
        blobs_restored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hash_proto(hex: &str) -> crate::types::HashProto {
        crate::types::HashProto {
            value: hex.to_string(),
        }
    }

    fn make_patch(
        id_hex: &str,
        op: &str,
        parents: &[&str],
        author: &str,
    ) -> crate::types::PatchProto {
        crate::types::PatchProto {
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

    #[test]
    fn test_backup_empty_hub() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("hub.db");
        let storage = HubStorage::open(&db_path).unwrap();

        let backup_dir = dir.path().join("backup");
        let manifest = create_backup(&storage, &backup_dir).unwrap();

        assert_eq!(manifest.version, BACKUP_VERSION);
        assert_eq!(manifest.repo_count, 0);
        assert_eq!(manifest.patch_count, 0);
        assert_eq!(manifest.blob_count, 0);
        assert!(backup_dir.join(MANIFEST_FILE).exists());
        assert!(backup_dir.join(METADATA_DB).exists());
    }

    #[test]
    fn test_backup_creates_valid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("hub.db");
        let storage = HubStorage::open(&db_path).unwrap();

        storage.ensure_repo("test-repo").unwrap();
        let patch = make_patch(&"a".repeat(64), "Create", &[], "alice");
        storage.insert_patch("test-repo", &patch).unwrap();
        storage
            .set_branch("test-repo", "main", &"a".repeat(64))
            .unwrap();
        storage
            .store_blob("test-repo", &"deadbeef".repeat(8), b"hello")
            .unwrap();

        let backup_dir = dir.path().join("backup");
        let manifest = create_backup(&storage, &backup_dir).unwrap();

        assert_eq!(manifest.repo_count, 1);
        assert_eq!(manifest.patch_count, 1);
        assert_eq!(manifest.blob_count, 1);
        assert!(!manifest.timestamp.is_empty());

        let manifest_path = backup_dir.join(MANIFEST_FILE);
        let loaded: BackupManifest =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(loaded.version, manifest.version);
        assert_eq!(loaded.repo_count, manifest.repo_count);
    }

    #[test]
    fn test_backup_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("hub.db");
        let storage = HubStorage::open(&db_path).unwrap();

        let backup_dir = dir.path().join("backup");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let result = create_backup(&storage, &backup_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("already exists"), "{err}");
    }

    #[test]
    fn test_restore_round_trip() {
        let dir = tempfile::tempdir().unwrap();

        let db_path = dir.path().join("hub.db");
        let storage = HubStorage::open(&db_path).unwrap();

        storage.ensure_repo("repo-1").unwrap();
        storage.ensure_repo("repo-2").unwrap();

        let patch1 = make_patch(&"a".repeat(64), "Create", &[], "alice");
        storage.insert_patch("repo-1", &patch1).unwrap();
        storage
            .set_branch("repo-1", "main", &"a".repeat(64))
            .unwrap();
        storage
            .store_blob("repo-1", &"deadbeef".repeat(8), b"hello world")
            .unwrap();

        let patch2 = make_patch(&"b".repeat(64), "Create", &[], "bob");
        storage.insert_patch("repo-2", &patch2).unwrap();
        storage
            .store_blob("repo-2", &"cafebabe".repeat(8), b"second blob")
            .unwrap();

        let backup_dir = dir.path().join("backup");
        create_backup(&storage, &backup_dir).unwrap();
        drop(storage);

        let restore_db = dir.path().join("restored.db");
        let mut restored_storage = HubStorage::open(&restore_db).unwrap();
        let stats = restore_backup(&mut restored_storage, &backup_dir).unwrap();

        assert_eq!(stats.repos_restored, 2);
        assert_eq!(stats.patches_restored, 2);
        assert_eq!(stats.blobs_restored, 2);

        assert!(restored_storage.repo_exists("repo-1").unwrap());
        assert!(restored_storage.repo_exists("repo-2").unwrap());

        let blob1 = restored_storage
            .get_blob("repo-1", &"deadbeef".repeat(8))
            .unwrap()
            .unwrap();
        assert_eq!(blob1, b"hello world");

        let blob2 = restored_storage
            .get_blob("repo-2", &"cafebabe".repeat(8))
            .unwrap()
            .unwrap();
        assert_eq!(blob2, b"second blob");

        let branches = restored_storage.get_branches("repo-1").unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "main");

        let patches = restored_storage.get_all_patches("repo-1", 0, 100).unwrap();
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].author, "alice");
    }

    #[test]
    fn test_restore_validates_manifest_format() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backup");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let manifest_path = backup_dir.join(MANIFEST_FILE);
        std::fs::write(&manifest_path, "not json").unwrap();

        let db_path = dir.path().join("hub.db");
        let mut storage = HubStorage::open(&db_path).unwrap();
        let result = restore_backup(&mut storage, &backup_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid manifest"), "{err}");
    }

    #[test]
    fn test_restore_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backup");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let db_path = dir.path().join("hub.db");
        let mut storage = HubStorage::open(&db_path).unwrap();
        let result = restore_backup(&mut storage, &backup_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("manifest.json not found"), "{err}");
    }

    #[test]
    fn test_restore_unsupported_version() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backup");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let manifest = BackupManifest {
            version: 999,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            repo_count: 0,
            patch_count: 0,
            blob_count: 0,
            suture_hub_version: "0.0.0".to_string(),
        };
        std::fs::write(
            backup_dir.join(MANIFEST_FILE),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let db_path = dir.path().join("hub.db");
        let mut storage = HubStorage::open(&db_path).unwrap();
        let result = restore_backup(&mut storage, &backup_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unsupported backup version"), "{err}");
    }
}
