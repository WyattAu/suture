use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub client_version: u32,
    pub client_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub server_version: u32,
    pub server_name: String,
    pub compatible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AuthMethod {
    None,
    Signature {
        public_key: String,
        signature: String,
    },
    Token(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    pub method: AuthMethod,
    pub timestamp: u64,
}

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
    /// Client's known state of branches at time of push.
    /// Used for fast-forward validation on the hub.
    /// Optional for backward compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_branches: Option<Vec<BranchProto>>,
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
    /// Limit the number of patches returned from each branch tip.
    /// None = full history, Some(n) = last n patches per branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u32>,
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

/// Build canonical bytes for push request signing.
/// Format: repo_id \0 patch_count \0 (each patch: id \0 op \0 author \0 msg \0 timestamp \0) ... branch_count \0 (each: name \0 target \0) ...
pub fn canonical_push_bytes(req: &PushRequest) -> Vec<u8> {
    let mut buf = Vec::new();

    buf.extend_from_slice(req.repo_id.as_bytes());
    buf.push(0);

    buf.extend_from_slice(&(req.patches.len() as u64).to_le_bytes());
    for patch in &req.patches {
        buf.extend_from_slice(patch.id.value.as_bytes());
        buf.push(0);
        buf.extend_from_slice(patch.operation_type.as_bytes());
        buf.push(0);
        buf.extend_from_slice(patch.author.as_bytes());
        buf.push(0);
        buf.extend_from_slice(patch.message.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&patch.timestamp.to_le_bytes());
        buf.push(0);
    }

    buf.extend_from_slice(&(req.branches.len() as u64).to_le_bytes());
    for branch in &req.branches {
        buf.extend_from_slice(branch.name.as_bytes());
        buf.push(0);
        buf.extend_from_slice(branch.target_id.value.as_bytes());
        buf.push(0);
    }

    buf
}
