//! Hub-specific types. Re-exports shared protocol types and adds mirror types.

// Re-export all shared protocol types
pub use suture_protocol::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub api_token: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub display_name: String,
    pub role: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub error: Option<String>,
    pub user: Option<UserInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct ListUsersResponse {
    pub success: bool,
    pub error: Option<String>,
    pub users: Vec<UserInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct GetUserResponse {
    pub success: bool,
    pub error: Option<String>,
    pub user: Option<UserInfo>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateRoleRequest {
    pub role: String,
}

#[derive(Debug, serde::Serialize)]
pub struct UpdateRoleResponse {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct DeleteUserResponse {
    pub success: bool,
    pub error: Option<String>,
}

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
    #[serde(default)]
    pub mirror_id: i64,
    pub local_repo: Option<String>,
    pub remote_url: Option<String>,
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

#[derive(Debug, serde::Deserialize)]
pub struct AddPeerRequest {
    pub peer_url: String,
    pub role: String,
}

#[derive(Debug, serde::Serialize)]
pub struct AddPeerResponse {
    pub success: bool,
    pub peer_id: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct RemovePeerResponse {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ListPeersResponse {
    pub peers: Vec<crate::storage::ReplicationPeer>,
}

#[derive(Debug, serde::Serialize)]
pub struct ReplicationStatusResponse {
    pub status: crate::storage::ReplicationStatus,
}

#[derive(Debug, serde::Serialize)]
pub struct SyncResponse {
    pub success: bool,
    pub applied: usize,
    pub error: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateRepoRequest {
    pub repo_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
    pub target: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub token: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchParams {
    pub q: String,
}
