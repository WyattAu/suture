//! Hub-specific types. Re-exports shared protocol types and adds mirror types.

// Re-export all shared protocol types
pub use suture_protocol::*;

/// Mirror-specific types (not part of the wire protocol).

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MirrorSetupRequest {
    pub repo_name: String,
    pub upstream_url: String,
    pub upstream_repo: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MirrorSetupResponse {
    pub success: bool,
    pub error: Option<String>,
    pub mirror_id: Option<i64>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MirrorSyncRequest {
    pub mirror_id: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MirrorSyncResponse {
    pub success: bool,
    pub error: Option<String>,
    pub patches_synced: u64,
    pub branches_synced: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MirrorStatusRequest {
    pub mirror_id: Option<i64>,
    pub repo_name: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MirrorStatusEntry {
    pub mirror_id: i64,
    pub repo_name: String,
    pub upstream_url: String,
    pub upstream_repo: String,
    pub last_sync: Option<u64>,
    pub status: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MirrorStatusResponse {
    pub success: bool,
    pub error: Option<String>,
    pub mirrors: Vec<MirrorStatusEntry>,
}
