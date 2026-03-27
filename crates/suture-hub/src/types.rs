use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HashProto {
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchProto {
    pub id: HashProto,
    pub operation_type: String,
    pub touch_set: Vec<String>,
    pub target_path: Option<String>,
    pub payload: String,
    pub parent_ids: Vec<HashProto>,
    pub author: String,
    pub message: String,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchProto {
    pub name: String,
    pub target_id: HashProto,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlobRef {
    pub hash: HashProto,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushRequest {
    pub repo_id: String,
    pub patches: Vec<PatchProto>,
    pub branches: Vec<BranchProto>,
    pub blobs: Vec<BlobRef>,
    /// Optional Ed25519 signature (64 bytes, base64-encoded).
    /// Required when the hub has authorized keys configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub success: bool,
    pub error: Option<String>,
    pub existing_patches: Vec<HashProto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullRequest {
    pub repo_id: String,
    pub known_branches: Vec<BranchProto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullResponse {
    pub success: bool,
    pub error: Option<String>,
    pub patches: Vec<PatchProto>,
    pub branches: Vec<BranchProto>,
    pub blobs: Vec<BlobRef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListReposResponse {
    pub repo_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepoInfoResponse {
    pub repo_id: String,
    pub patch_count: u64,
    pub branches: Vec<BranchProto>,
    pub success: bool,
    pub error: Option<String>,
}

pub fn hash_to_hex(h: &HashProto) -> String {
    h.value.clone()
}

pub fn hex_to_hash(hex: &str) -> HashProto {
    HashProto {
        value: hex.to_string(),
    }
}
