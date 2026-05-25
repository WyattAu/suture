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
    pub error_code: Option<HubErrorCode>,
    pub user: Option<UserInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct ListUsersResponse {
    pub success: bool,
    pub error: Option<String>,
    pub error_code: Option<HubErrorCode>,
    pub users: Vec<UserInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct GetUserResponse {
    pub success: bool,
    pub error: Option<String>,
    pub error_code: Option<HubErrorCode>,
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
    pub error_code: Option<HubErrorCode>,
}

#[derive(Debug, serde::Serialize)]
pub struct DeleteUserResponse {
    pub success: bool,
    pub error: Option<String>,
    pub error_code: Option<HubErrorCode>,
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
    pub error_code: Option<HubErrorCode>,
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
    pub error_code: Option<HubErrorCode>,
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
    pub error_code: Option<HubErrorCode>,
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
    pub error_code: Option<HubErrorCode>,
}

#[derive(Debug, serde::Serialize)]
pub struct RemovePeerResponse {
    pub success: bool,
    pub error: Option<String>,
    pub error_code: Option<HubErrorCode>,
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
    pub error_code: Option<HubErrorCode>,
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BatchPatchRequest {
    pub repo_id: String,
    pub patches: Vec<PatchProto>,
    pub branches: Vec<BranchProto>,
    pub blobs: Vec<BlobRef>,
    pub signature: Option<Vec<u8>>,
    pub force: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TreeEntry {
    pub path: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Issue {
    pub id: i64,
    pub repo_id: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub author: String,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IssueComment {
    pub id: i64,
    pub issue_id: i64,
    pub author: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateIssueRequest {
    pub repo_id: String,
    pub title: String,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListIssuesResponse {
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PullRequestRecord {
    pub id: i64,
    pub repo_id: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub author: String,
    pub source_branch: String,
    pub target_branch: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub merged_at: Option<i64>,
    pub merged_by: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrReview {
    pub id: i64,
    pub pr_id: i64,
    pub reviewer: String,
    pub verdict: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreatePullRequestRequest {
    pub repo_id: String,
    pub title: String,
    pub body: Option<String>,
    pub source_branch: String,
    pub target_branch: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateReviewRequest {
    pub verdict: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeSearchResult {
    pub blob_hash: String,
    pub repo_id: String,
    pub path: Option<String>,
    pub match_count: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Organization {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Team {
    pub id: i64,
    pub org_id: i64,
    pub name: String,
    pub permission: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateTeamRequest {
    pub org_id: i64,
    pub name: String,
    pub permission: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddTeamMemberRequest {
    pub username: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ForkRepoRequest {
    pub source_repo_id: String,
    pub target_repo_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateVisibilityRequest {
    pub visibility: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WikiPage {
    pub id: i64,
    pub repo_id: String,
    pub title: String,
    pub content: String,
    pub author: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateWikiPageRequest {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Release {
    pub id: i64,
    pub repo_id: String,
    pub tag: String,
    pub title: String,
    pub body: String,
    pub author: String,
    pub prerelease: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateReleaseRequest {
    pub repo_id: String,
    pub tag: String,
    pub title: String,
    pub body: Option<String>,
    pub prerelease: Option<bool>,
}
