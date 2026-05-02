use std::sync::Arc;

use rusqlite::Connection;

pub trait BlobBackend: Send + Sync {
    fn store_blob(&self, repo_id: &str, hash_hex: &str, data: &[u8]) -> Result<(), String>;
    fn get_blob(&self, repo_id: &str, hash_hex: &str) -> Result<Option<Vec<u8>>, String>;
    fn has_blob(&self, repo_id: &str, hash_hex: &str) -> Result<bool, String>;
    fn delete_blob(&self, repo_id: &str, hash_hex: &str) -> Result<(), String>;
    fn list_blobs(&self, repo_id: &str) -> Result<Vec<String>, String>;
}

pub struct SqliteBlobBackend {
    conn: Arc<std::sync::Mutex<Connection>>,
}

impl SqliteBlobBackend {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(std::sync::Mutex::new(conn)),
        }
    }
}

impl BlobBackend for SqliteBlobBackend {
    fn store_blob(&self, repo_id: &str, hash_hex: &str, data: &[u8]) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute(
            "INSERT OR REPLACE INTO blobs (repo_id, blob_hash, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![repo_id, hash_hex, data],
        )
        .map_err(|e| format!("store blob: {e}"))?;
        Ok(())
    }

    fn get_blob(&self, repo_id: &str, hash_hex: &str) -> Result<Option<Vec<u8>>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let result = conn.query_row(
            "SELECT data FROM blobs WHERE repo_id = ?1 AND blob_hash = ?2",
            rusqlite::params![repo_id, hash_hex],
            |row| row.get::<_, Vec<u8>>(0),
        );
        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("get blob: {e}")),
        }
    }

    fn has_blob(&self, repo_id: &str, hash_hex: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM blobs WHERE repo_id = ?1 AND blob_hash = ?2",
                rusqlite::params![repo_id, hash_hex],
                |row| row.get(0),
            )
            .map_err(|e| format!("has blob: {e}"))?;
        Ok(count > 0)
    }

    fn delete_blob(&self, repo_id: &str, hash_hex: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute(
            "DELETE FROM blobs WHERE repo_id = ?1 AND blob_hash = ?2",
            rusqlite::params![repo_id, hash_hex],
        )
        .map_err(|e| format!("delete blob: {e}"))?;
        Ok(())
    }

    fn list_blobs(&self, repo_id: &str) -> Result<Vec<String>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT blob_hash FROM blobs WHERE repo_id = ?1")
            .map_err(|e| format!("list blobs: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params![repo_id], |row| row.get::<_, String>(0))
            .map_err(|e| format!("list blobs: {e}"))?;
        let mut hashes = Vec::new();
        for row in rows {
            hashes.push(row.map_err(|e| format!("list blobs: {e}"))?);
        }
        Ok(hashes)
    }
}

#[cfg(feature = "s3-backend")]
pub mod s3_adapter {
    use super::BlobBackend;
    use suture_common::Hash;
    use suture_s3::{S3BlobStore, S3Config};

    pub struct S3BlobBackendAdapter {
        store: S3BlobStore,
    }

    impl S3BlobBackendAdapter {
        pub fn new(config: S3Config) -> Self {
            Self {
                store: S3BlobStore::new(config),
            }
        }
    }

    impl BlobBackend for S3BlobBackendAdapter {
        fn store_blob(&self, _repo_id: &str, hash_hex: &str, data: &[u8]) -> Result<(), String> {
            let hash = Hash::from_hex(hash_hex).map_err(|e| format!("invalid hash: {e}"))?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(self.store.put_blob(&hash, data))
                .map_err(|e| format!("s3 put: {e}"))
        }

        fn get_blob(&self, _repo_id: &str, hash_hex: &str) -> Result<Option<Vec<u8>>, String> {
            let hash = Hash::from_hex(hash_hex).map_err(|e| format!("invalid hash: {e}"))?;
            let rt = tokio::runtime::Handle::current();
            match rt.block_on(self.store.get_blob(&hash)) {
                Ok(data) => Ok(Some(data)),
                Err(suture_s3::S3Error::NotFound(_)) => Ok(None),
                Err(e) => Err(format!("s3 get: {e}")),
            }
        }

        fn has_blob(&self, _repo_id: &str, hash_hex: &str) -> Result<bool, String> {
            let hash = Hash::from_hex(hash_hex).map_err(|e| format!("invalid hash: {e}"))?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(self.store.has_blob(&hash))
                .map_err(|e| format!("s3 has: {e}"))
        }

        fn delete_blob(&self, _repo_id: &str, hash_hex: &str) -> Result<(), String> {
            let hash = Hash::from_hex(hash_hex).map_err(|e| format!("invalid hash: {e}"))?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(self.store.delete_blob(&hash))
                .map_err(|e| format!("s3 delete: {e}"))
        }

        fn list_blobs(&self, _repo_id: &str) -> Result<Vec<String>, String> {
            let rt = tokio::runtime::Handle::current();
            let hashes = rt
                .block_on(self.store.list_blobs())
                .map_err(|e| format!("s3 list: {e}"))?;
            Ok(hashes.iter().map(|h| h.to_hex()).collect())
        }
    }
}

#[cfg(feature = "s3-backend")]
pub use s3_adapter::S3BlobBackendAdapter;

pub struct BlobBackendConfig {
    pub backend_type: String,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_region: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_prefix: String,
}

impl Default for BlobBackendConfig {
    fn default() -> Self {
        Self {
            backend_type: "sqlite".to_owned(),
            s3_endpoint: String::new(),
            s3_bucket: String::new(),
            s3_region: "us-east-1".to_owned(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            s3_prefix: "suture/blobs/".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS blobs (
                repo_id TEXT NOT NULL,
                blob_hash TEXT NOT NULL,
                data BLOB NOT NULL,
                PRIMARY KEY (repo_id, blob_hash)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_sqlite_backend_store_retrieve() {
        let conn = setup_db();
        let backend = SqliteBlobBackend::new(conn);

        backend
            .store_blob("repo1", &"a".repeat(64), b"hello world")
            .unwrap();

        let data = backend.get_blob("repo1", &"a".repeat(64)).unwrap();
        assert_eq!(data, Some(b"hello world".to_vec()));

        let missing = backend.get_blob("repo1", &"b".repeat(64)).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_sqlite_backend_has_delete() {
        let conn = setup_db();
        let backend = SqliteBlobBackend::new(conn);

        assert!(!backend.has_blob("repo1", &"a".repeat(64)).unwrap());

        backend
            .store_blob("repo1", &"a".repeat(64), b"data")
            .unwrap();
        assert!(backend.has_blob("repo1", &"a".repeat(64)).unwrap());

        backend.delete_blob("repo1", &"a".repeat(64)).unwrap();
        assert!(!backend.has_blob("repo1", &"a".repeat(64)).unwrap());
    }

    #[test]
    fn test_sqlite_backend_list() {
        let conn = setup_db();
        let backend = SqliteBlobBackend::new(conn);

        backend
            .store_blob("repo1", &"a".repeat(64), b"one")
            .unwrap();
        backend
            .store_blob("repo1", &"b".repeat(64), b"two")
            .unwrap();
        backend
            .store_blob("repo2", &"c".repeat(64), b"three")
            .unwrap();

        let list = backend.list_blobs("repo1").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"a".repeat(64)));
        assert!(list.contains(&"b".repeat(64)));

        let list2 = backend.list_blobs("repo2").unwrap();
        assert_eq!(list2.len(), 1);
        assert!(list2.contains(&"c".repeat(64)));
    }

    #[cfg(feature = "s3-backend")]
    #[test]
    fn test_s3_adapter_constructs() {
        let config = suture_s3::S3Config {
            endpoint: "http://localhost:9000".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            access_key: "key".to_string(),
            secret_key: "secret".to_string(),
            prefix: "test/".to_string(),
            force_path_style: true,
        };
        let _adapter = S3BlobBackendAdapter::new(config);
    }
}
