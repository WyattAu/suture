// SPDX-License-Identifier: MIT OR Apache-2.0
//! Suture Protocol — wire format for client-server communication.
//!
//! Defines the request/response types used by the Suture Hub for
//! push, pull, authentication, and repository management operations.
//! All types are serializable via `serde` for JSON transport.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const PROTOCOL_VERSION_V2: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HubErrorCode {
    AuthFailed,
    RateLimited,
    BlobTooLarge,
    RepoNotFound,
    Conflict,
    PatchNotFound,
    BranchNotFound,
    InsufficientPermissions,
    InternalError,
    UserNotFound,
    UserAlreadyExists,
    InvalidRequest,
    MirrorNotFound,
    MirrorSyncFailed,
    PeerNotFound,
    ReplicationFailed,
    TagNotFound,
}

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
    #[serde(default)]
    pub truncated: bool,
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
    /// If true, skip fast-forward validation on push.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub success: bool,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<HubErrorCode>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<HubErrorCode>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<HubErrorCode>,
}

#[must_use]
pub fn hash_to_hex(h: &HashProto) -> String {
    h.value.clone()
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    zstd::encode_all(data, 3).map_err(|e| format!("zstd compression failed: {e}"))
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    zstd::decode_all(data).map_err(|e| format!("zstd decompression failed: {e}"))
}

#[must_use]
pub fn hex_to_hash(hex: &str) -> HashProto {
    HashProto {
        value: hex.to_owned(),
    }
}

/// Build canonical bytes for push request signing.
/// Format: repo_id \0 patch_count \0 (each patch: id \0 op \0 author \0 msg \0 timestamp \0) ... branch_count \0 (each: name \0 target \0) ...
#[must_use]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeltaEncoding {
    BinaryPatch,
    FullBlob,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlobDelta {
    pub base_hash: HashProto,
    pub target_hash: HashProto,
    pub encoding: DeltaEncoding,
    pub delta_data: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub supports_delta: bool,
    pub supports_compression: bool,
    pub max_blob_size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub supports_delta: bool,
    pub supports_compression: bool,
    pub max_blob_size: u64,
    pub protocol_versions: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullRequestV2 {
    pub repo_id: String,
    pub known_branches: Vec<BranchProto>,
    pub max_depth: Option<u32>,
    pub known_blob_hashes: Vec<HashProto>,
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullResponseV2 {
    pub success: bool,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<HubErrorCode>,
    pub patches: Vec<PatchProto>,
    pub branches: Vec<BranchProto>,
    pub blobs: Vec<BlobRef>,
    pub deltas: Vec<BlobDelta>,
    pub protocol_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushRequestV2 {
    pub repo_id: String,
    pub patches: Vec<PatchProto>,
    pub branches: Vec<BranchProto>,
    pub blobs: Vec<BlobRef>,
    pub deltas: Vec<BlobDelta>,
    pub signature: Option<Vec<u8>>,
    pub known_branches: Option<Vec<BranchProto>>,
    pub force: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeRequestV2 {
    pub client_version: u32,
    pub client_name: String,
    pub capabilities: ClientCapabilities,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeResponseV2 {
    pub server_version: u32,
    pub server_name: String,
    pub compatible: bool,
    pub server_capabilities: ServerCapabilities,
}

// === LFS Protocol Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfsBatchRequest {
    pub repo_id: String,
    pub objects: Vec<LfsObjectRef>,
    pub operation: LfsOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LfsOperation {
    Upload,
    Download,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfsObjectRef {
    pub oid: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfsBatchResponse {
    pub objects: Vec<LfsObjectAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfsObjectAction {
    pub oid: String,
    pub size: u64,
    pub action: LfsAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LfsAction {
    None,
    Upload,
    Download,
    Error,
}

#[must_use]
pub fn compute_delta(base: &[u8], target: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let prefix_len = base
        .iter()
        .zip(target.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let max_suffix_base = base.len().saturating_sub(prefix_len);
    let max_suffix_target = target.len().saturating_sub(prefix_len);
    let suffix_len = base[prefix_len..]
        .iter()
        .rev()
        .zip(target[prefix_len..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(max_suffix_base)
        .min(max_suffix_target);

    let changed_start = prefix_len;
    let changed_end_target = target.len().saturating_sub(suffix_len);
    let changed = &target[changed_start..changed_end_target];

    if changed.len() < target.len() {
        let mut delta = Vec::new();
        delta.push(0x01);
        delta.extend_from_slice(&(prefix_len as u64).to_le_bytes());
        delta.extend_from_slice(&(suffix_len as u64).to_le_bytes());
        delta.extend_from_slice(&(target.len() as u64).to_le_bytes());
        delta.extend_from_slice(changed);
        (base.to_vec(), delta)
    } else {
        let mut full = vec![0x00];
        full.extend_from_slice(target);
        (base.to_vec(), full)
    }
}

#[must_use]
pub fn apply_delta(base: &[u8], delta: &[u8]) -> Vec<u8> {
    if delta.is_empty() {
        return Vec::new();
    }
    match delta[0] {
        0x00 => delta[1..].to_vec(),
        0x01 => {
            if delta.len() < 25 {
                return delta.to_vec();
            }
            let prefix_len = u64::from_le_bytes(delta[1..9].try_into().unwrap_or([0; 8])) as usize;
            let suffix_len = u64::from_le_bytes(delta[9..17].try_into().unwrap_or([0; 8])) as usize;
            let total_len = u64::from_le_bytes(delta[17..25].try_into().unwrap_or([0; 8])) as usize;
            let changed = &delta[25..];

            let mut result = Vec::with_capacity(total_len);
            result.extend_from_slice(&base[..prefix_len.min(base.len())]);
            result.extend_from_slice(changed);
            result.extend_from_slice(&base[base.len().saturating_sub(suffix_len)..]);
            result
        }
        _ => delta.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de>>(val: &T) -> T {
        let json = serde_json::to_string(val).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    fn make_hash(hex: &str) -> HashProto {
        HashProto {
            value: hex.to_string(),
        }
    }

    fn make_patch(id: &str, op: &str, parents: &[&str]) -> PatchProto {
        PatchProto {
            id: make_hash(id),
            operation_type: op.to_string(),
            touch_set: vec![format!("file_{id}")],
            target_path: Some(format!("file_{id}")),
            payload: String::new(),
            parent_ids: parents.iter().map(|p| make_hash(p)).collect(),
            author: "alice".to_string(),
            message: format!("patch {id}"),
            timestamp: 1000,
        }
    }

    fn make_branch(name: &str, target: &str) -> BranchProto {
        BranchProto {
            name: name.to_string(),
            target_id: make_hash(target),
        }
    }

    #[test]
    fn test_handshake_roundtrip() {
        let req = HandshakeRequest {
            client_version: 1,
            client_name: "test".to_string(),
        };
        let rt: HandshakeRequest = roundtrip(&req);
        assert_eq!(rt.client_version, 1);
        assert_eq!(rt.client_name, "test");

        let resp = HandshakeResponse {
            server_version: 1,
            server_name: "hub".to_string(),
            compatible: true,
        };
        let rt: HandshakeResponse = roundtrip(&resp);
        assert!(rt.compatible);
    }

    #[test]
    fn test_auth_method_roundtrip() {
        let methods = vec![
            AuthMethod::None,
            AuthMethod::Signature {
                public_key: "pk".to_string(),
                signature: "sig".to_string(),
            },
            AuthMethod::Token("tok".to_string()),
        ];
        for m in &methods {
            let rt: AuthMethod = roundtrip(m);
            match (m, &rt) {
                (AuthMethod::None, AuthMethod::None) => {}
                (
                    AuthMethod::Signature {
                        public_key: a,
                        signature: b,
                    },
                    AuthMethod::Signature {
                        public_key: c,
                        signature: d,
                    },
                ) => {
                    assert_eq!(a, c);
                    assert_eq!(b, d);
                }
                (AuthMethod::Token(a), AuthMethod::Token(b)) => assert_eq!(a, b),
                _ => panic!("auth method mismatch"),
            }
        }
    }

    #[test]
    fn test_patch_proto_roundtrip() {
        let p = make_patch("a".repeat(64).as_str(), "Create", &[]);
        let rt: PatchProto = roundtrip(&p);
        assert_eq!(rt.operation_type, "Create");
        assert_eq!(rt.touch_set.len(), 1);
        assert!(rt.target_path.is_some());
        assert!(rt.parent_ids.is_empty());
        assert_eq!(rt.author, "alice");
    }

    #[test]
    fn test_patch_proto_with_parents() {
        let parent = "b".repeat(64);
        let p = make_patch("a".repeat(64).as_str(), "Modify", &[&parent]);
        let rt: PatchProto = roundtrip(&p);
        assert_eq!(rt.parent_ids.len(), 1);
        assert_eq!(hash_to_hex(&rt.parent_ids[0]), parent);
    }

    #[test]
    fn test_push_request_roundtrip() {
        let req = PushRequest {
            repo_id: "my-repo".to_string(),
            patches: vec![make_patch("a".repeat(64).as_str(), "Create", &[])],
            branches: vec![make_branch("main", "a".repeat(64).as_str())],
            blobs: vec![BlobRef {
                hash: make_hash("deadbeef"),
                data: "aGVsbG8=".to_string(),
                truncated: false,
            }],
            signature: Some(vec![1u8; 64]),
            known_branches: Some(vec![make_branch("main", "prev".repeat(32).as_str())]),
            force: true,
        };
        let rt: PushRequest = roundtrip(&req);
        assert_eq!(rt.repo_id, "my-repo");
        assert_eq!(rt.patches.len(), 1);
        assert_eq!(rt.branches.len(), 1);
        assert_eq!(rt.blobs.len(), 1);
        assert!(rt.signature.is_some());
        assert!(rt.known_branches.is_some());
        assert!(rt.force);
    }

    #[test]
    fn test_push_request_defaults() {
        let req = PushRequest {
            repo_id: "r".to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        let rt: PushRequest = serde_json::from_str(&json).unwrap();
        assert!(rt.signature.is_none());
        assert!(rt.known_branches.is_none());
        assert!(!rt.force);
    }

    #[test]
    fn test_pull_request_roundtrip() {
        let req = PullRequest {
            repo_id: "r".to_string(),
            known_branches: vec![make_branch("main", "a".repeat(32).as_str())],
            max_depth: Some(10),
        };
        let rt: PullRequest = roundtrip(&req);
        assert_eq!(rt.max_depth, Some(10));

        let req2 = PullRequest {
            repo_id: "r".to_string(),
            known_branches: vec![],
            max_depth: None,
        };
        let rt2: PullRequest = roundtrip(&req2);
        assert!(rt2.max_depth.is_none());
    }

    #[test]
    fn test_pull_response_roundtrip() {
        let resp = PullResponse {
            success: true,
            error: None,
            error_code: None,
            patches: vec![make_patch("a".repeat(64).as_str(), "Create", &[])],
            branches: vec![make_branch("main", "a".repeat(64).as_str())],
            blobs: vec![BlobRef {
                hash: make_hash("abc"),
                data: "dGVzdA==".to_string(),
                truncated: false,
            }],
        };

        let rt: PullResponse = roundtrip(&resp);
        assert!(rt.success);
        assert_eq!(rt.patches.len(), 1);
        assert_eq!(rt.blobs.len(), 1);
    }

    #[test]
    fn test_pull_response_error() {
        let resp = PullResponse {
            success: false,
            error: Some("not found".to_string()),
            error_code: None,
            patches: vec![],
            branches: vec![],
            blobs: vec![],
        };
        let rt: PullResponse = roundtrip(&resp);
        assert!(!rt.success);
        assert_eq!(rt.error, Some("not found".to_string()));
    }

    #[test]
    fn test_blob_ref_roundtrip() {
        let blob = BlobRef {
            hash: make_hash("cafebabe"),
            data: "SGVsbG8gV29ybGQ=".to_string(),
            truncated: false,
        };
        let rt: BlobRef = roundtrip(&blob);
        assert_eq!(rt.data, "SGVsbG8gV29ybGQ=");
    }

    #[test]
    fn test_hash_helpers() {
        let h = hex_to_hash("abcdef1234");
        assert_eq!(hash_to_hex(&h), "abcdef1234");
    }

    #[test]
    fn test_canonical_push_bytes_deterministic() {
        let req = PushRequest {
            repo_id: "test".to_string(),
            patches: vec![make_patch("a".repeat(64).as_str(), "Create", &[])],
            branches: vec![make_branch("main", "a".repeat(64).as_str())],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        };
        let b1 = canonical_push_bytes(&req);
        let b2 = canonical_push_bytes(&req);
        assert_eq!(b1, b2);
    }

    #[test]
    fn test_canonical_push_bytes_different_repos() {
        let make_req = |repo: &str| PushRequest {
            repo_id: repo.to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        };
        let b1 = canonical_push_bytes(&make_req("repo-a"));
        let b2 = canonical_push_bytes(&make_req("repo-b"));
        assert_ne!(b1, b2);
    }

    #[test]
    fn test_repo_info_response_roundtrip() {
        let resp = RepoInfoResponse {
            repo_id: "my-repo".to_string(),
            patch_count: 42,
            branches: vec![make_branch("main", "a".repeat(32).as_str())],
            success: true,
            error: None,
            error_code: None,
        };
        let rt: RepoInfoResponse = roundtrip(&resp);
        assert_eq!(rt.patch_count, 42);
        assert!(rt.success);

        let err = RepoInfoResponse {
            repo_id: "x".to_string(),
            patch_count: 0,
            branches: vec![],
            success: false,
            error: Some("not found".to_string()),
            error_code: None,
        };
        let rt2: RepoInfoResponse = roundtrip(&err);
        assert!(!rt2.success);
        assert_eq!(rt2.error, Some("not found".to_string()));
    }

    #[test]
    fn test_list_repos_response_roundtrip() {
        let resp = ListReposResponse {
            repo_ids: vec!["a".to_string(), "b".to_string()],
        };
        let rt: ListReposResponse = roundtrip(&resp);
        assert_eq!(rt.repo_ids, vec!["a", "b"]);
    }

    #[test]
    fn test_push_response_roundtrip() {
        let resp = PushResponse {
            success: true,
            error: None,
            error_code: None,
            existing_patches: vec![make_hash("abc"), make_hash("def")],
        };
        let rt: PushResponse = roundtrip(&resp);
        assert_eq!(rt.existing_patches.len(), 2);
    }

    #[test]
    fn test_delta_roundtrip() {
        let base = b"Hello, World!";
        let target = b"Hello, Rust!";
        let (_base_copy, delta) = compute_delta(base, target);
        let result = apply_delta(base, &delta);
        assert_eq!(result, target);
    }

    #[test]
    fn test_delta_no_change() {
        let base = b"identical data here";
        let target = b"identical data here";
        let (_base_copy, delta) = compute_delta(base, target);
        assert!(delta.len() < target.len() + 25);
        let result = apply_delta(base, &delta);
        assert_eq!(result, target);
    }

    #[test]
    fn test_delta_completely_different() {
        let base = b"AAAA";
        let target = b"BBBB";
        let (_base_copy, delta) = compute_delta(base, target);
        let result = apply_delta(base, &delta);
        assert_eq!(result, target);
    }

    #[test]
    fn test_pull_request_v2_roundtrip() {
        let req = PullRequestV2 {
            repo_id: "my-repo".to_string(),
            known_branches: vec![make_branch("main", "a".repeat(32).as_str())],
            max_depth: Some(10),
            known_blob_hashes: vec![make_hash("deadbeef")],
            capabilities: ClientCapabilities {
                supports_delta: true,
                supports_compression: true,
                max_blob_size: 1024 * 1024,
            },
        };
        let rt: PullRequestV2 = roundtrip(&req);
        assert_eq!(rt.repo_id, "my-repo");
        assert_eq!(rt.max_depth, Some(10));
        assert!(rt.capabilities.supports_delta);
        assert_eq!(rt.known_blob_hashes.len(), 1);
    }

    #[test]
    fn test_handshake_v2_roundtrip() {
        let req = HandshakeRequestV2 {
            client_version: 2,
            client_name: "suture-cli".to_string(),
            capabilities: ClientCapabilities {
                supports_delta: true,
                supports_compression: false,
                max_blob_size: 512 * 1024,
            },
        };
        let rt: HandshakeRequestV2 = roundtrip(&req);
        assert_eq!(rt.client_version, 2);
        assert!(rt.capabilities.supports_delta);
        assert!(!rt.capabilities.supports_compression);

        let resp = HandshakeResponseV2 {
            server_version: 2,
            server_name: "suture-hub".to_string(),
            compatible: true,
            server_capabilities: ServerCapabilities {
                supports_delta: true,
                supports_compression: true,
                max_blob_size: 10 * 1024 * 1024,
                protocol_versions: vec![1, 2],
            },
        };
        let rt: HandshakeResponseV2 = roundtrip(&resp);
        assert!(rt.compatible);
        assert_eq!(rt.server_capabilities.protocol_versions, vec![1, 2]);
    }

    #[test]
    fn test_client_capabilities_roundtrip() {
        let caps = ClientCapabilities {
            supports_delta: false,
            supports_compression: true,
            max_blob_size: 999,
        };
        let rt: ClientCapabilities = roundtrip(&caps);
        assert!(!rt.supports_delta);
        assert!(rt.supports_compression);
        assert_eq!(rt.max_blob_size, 999);
    }

    #[test]
    fn test_blob_delta_roundtrip() {
        let delta = BlobDelta {
            base_hash: make_hash("aaa"),
            target_hash: make_hash("bbb"),
            encoding: DeltaEncoding::BinaryPatch,
            delta_data: "ZGF0YQ==".to_string(),
        };
        let rt: BlobDelta = roundtrip(&delta);
        assert_eq!(hash_to_hex(&rt.base_hash), "aaa");
        assert_eq!(hash_to_hex(&rt.target_hash), "bbb");
        assert!(matches!(rt.encoding, DeltaEncoding::BinaryPatch));
        assert_eq!(rt.delta_data, "ZGF0YQ==");

        let full = BlobDelta {
            base_hash: make_hash("aaa"),
            target_hash: make_hash("bbb"),
            encoding: DeltaEncoding::FullBlob,
            delta_data: "Ynl0ZXM=".to_string(),
        };
        let rt: BlobDelta = roundtrip(&full);
        assert!(matches!(rt.encoding, DeltaEncoding::FullBlob));
    }

    fn assert_delta_roundtrip(base: &[u8], target: &[u8]) {
        let (_base_copy, delta) = compute_delta(base, target);
        let result = apply_delta(base, &delta);
        assert_eq!(
            result,
            target,
            "delta roundtrip failed: base_len={}, target_len={}",
            base.len(),
            target.len()
        );
    }

    #[test]
    fn test_delta_small_1_to_10_bytes() {
        for len in 1..=10u32 {
            let base: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();
            let target: Vec<u8> = (0..len).map(|i| (i * 13 + 3) as u8).collect();
            assert_delta_roundtrip(&base, &target);
        }
    }

    #[test]
    fn test_delta_medium_100_to_1000_bytes() {
        for len in [100, 200, 500, 1000] {
            let base: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
            let mut target = base.clone();
            for i in len / 3..len * 2 / 3 {
                target[i] = target[i].wrapping_add(1);
            }
            assert_delta_roundtrip(&base, &target);
        }
    }

    #[test]
    fn test_delta_large_10kb_plus() {
        for len in [10_240, 50_000, 100_000] {
            let base: Vec<u8> = (0..len).map(|i| ((i * 7 + 13) % 251) as u8).collect();
            let mut target = base.clone();
            target[len / 2] = 0xFF;
            assert_delta_roundtrip(&base, &target);
        }
    }

    #[test]
    fn test_delta_empty_base() {
        assert_delta_roundtrip(b"", b"some target data that is reasonably long enough");
    }

    #[test]
    fn test_delta_empty_target() {
        assert_delta_roundtrip(b"some base data that is reasonably long enough too", b"");
    }

    #[test]
    fn test_delta_both_empty() {
        assert_delta_roundtrip(b"", b"");
    }

    #[test]
    fn test_delta_identical() {
        let data = b"The quick brown fox jumps over the lazy dog";
        assert_delta_roundtrip(data, data);
    }

    #[test]
    fn test_delta_base_is_prefix_of_target() {
        assert_delta_roundtrip(
            b"shared prefix data",
            b"shared prefix data and extra suffix content here",
        );
    }

    #[test]
    fn test_delta_target_is_prefix_of_base() {
        assert_delta_roundtrip(
            b"shared prefix data and extra suffix content here",
            b"shared prefix data",
        );
    }

    #[test]
    fn test_delta_completely_different_same_length() {
        let base: Vec<u8> = (0..100).map(|i| (i * 3) as u8).collect();
        let target: Vec<u8> = (0..100).map(|i| (i * 7 + 100) as u8).collect();
        assert_delta_roundtrip(&base, &target);
    }

    #[test]
    fn test_delta_completely_different_different_lengths() {
        assert_delta_roundtrip(&vec![0xAA; 50], &vec![0xBB; 200]);
    }

    #[test]
    fn test_delta_common_middle_section() {
        let middle = b"COMMON_MIDDLE_SECTION_THAT_IS_LONG_ENOUGH";
        let mut base = Vec::new();
        base.extend_from_slice(b"DIFFERENT_START_XXXXXX_");
        base.extend_from_slice(middle);
        base.extend_from_slice(b"_DIFFERENT_END_XXXXXX");

        let mut target = Vec::new();
        target.extend_from_slice(b"CHANGED_PREFIX_");
        target.extend_from_slice(middle);
        target.extend_from_slice(b"_CHANGED_SUFFIX_DATA");

        assert_delta_roundtrip(&base, &target);
    }

    #[test]
    fn test_delta_single_byte_change() {
        let base = vec![0u8; 1000];
        let mut target = base.clone();
        target[500] = 1;
        assert_delta_roundtrip(&base, &target);
    }

    #[test]
    fn test_delta_single_byte_base_and_target() {
        assert_delta_roundtrip(b"A", b"B");
    }

    #[test]
    fn test_delta_single_byte_identical() {
        assert_delta_roundtrip(b"X", b"X");
    }

    #[test]
    fn test_delta_prefix_overlap_large() {
        let prefix: Vec<u8> = (0..60u8).collect();
        let mut base = prefix.clone();
        base.extend_from_slice(&vec![0x00; 60]);
        let mut target = prefix.clone();
        target.extend_from_slice(&(60..120u8).collect::<Vec<_>>());
        assert_delta_roundtrip(&base, &target);
    }

    #[test]
    fn test_delta_suffix_overlap_large() {
        let suffix: Vec<u8> = (60..120u8).collect();
        let mut base = vec![0x00; 60];
        base.extend_from_slice(&suffix);
        let mut target = (0..60u8).collect::<Vec<_>>();
        target.extend_from_slice(&suffix);
        assert_delta_roundtrip(&base, &target);
    }

    #[test]
    fn test_compress_decompress_empty() {
        let compressed = compress(b"").unwrap();
        assert_eq!(decompress(&compressed).unwrap(), b"");
    }

    #[test]
    fn test_compress_decompress_small_1_to_100() {
        for len in 1..=100u32 {
            let data: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
            let compressed = compress(&data).unwrap();
            assert_eq!(
                decompress(&compressed).unwrap(),
                data,
                "failed for len={len}"
            );
        }
    }

    #[test]
    fn test_compress_decompress_medium() {
        for len in [100, 500, 1000, 5000, 10_000] {
            let data: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
            let compressed = compress(&data).unwrap();
            assert_eq!(
                decompress(&compressed).unwrap(),
                data,
                "failed for len={len}"
            );
        }
    }

    #[test]
    fn test_compress_decompress_large() {
        let len = 200_000usize;
        let data: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let compressed = compress(&data).unwrap();
        assert!(
            compressed.len() < data.len(),
            "compressed should be smaller for repetitive data"
        );
        assert_eq!(decompress(&compressed).unwrap(), data);
    }

    #[test]
    fn test_compress_decompress_incompressible() {
        let mut data = Vec::with_capacity(100_000);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for i in 0..100_000 {
            use std::hash::{Hash, Hasher};
            i.hash(&mut hasher);
            data.push((hasher.finish() % 256) as u8);
            hasher = std::collections::hash_map::DefaultHasher::new();
        }
        let compressed = compress(&data).unwrap();
        assert_eq!(decompress(&compressed).unwrap(), data);
    }

    #[test]
    fn test_compress_decompress_highly_compressible() {
        let data = vec![0xAAu8; 500_000];
        let compressed = compress(&data).unwrap();
        assert!(
            compressed.len() < 100,
            "highly compressible data should be tiny"
        );
        assert_eq!(decompress(&compressed).unwrap(), data);
    }

    #[test]
    fn test_decompress_invalid_data_fails() {
        assert!(decompress(b"not valid zstd data").is_err());
    }

    #[test]
    fn test_decompress_empty_input_fails() {
        assert!(decompress(b"").is_err());
    }

    #[test]
    fn test_server_capabilities_roundtrip() {
        let caps = ServerCapabilities {
            supports_delta: true,
            supports_compression: true,
            max_blob_size: 50 * 1024 * 1024,
            protocol_versions: vec![1, 2],
        };
        let rt: ServerCapabilities = roundtrip(&caps);
        assert!(rt.supports_delta);
        assert!(rt.supports_compression);
        assert_eq!(rt.max_blob_size, 50 * 1024 * 1024);
        assert_eq!(rt.protocol_versions, vec![1, 2]);
    }

    #[test]
    fn test_capability_version_matching() {
        let server_caps = ServerCapabilities {
            supports_delta: true,
            supports_compression: false,
            max_blob_size: 1024 * 1024,
            protocol_versions: vec![1, 2],
        };
        let client_caps = ClientCapabilities {
            supports_delta: true,
            supports_compression: true,
            max_blob_size: 1024 * 1024,
        };
        assert!(server_caps.protocol_versions.contains(&PROTOCOL_VERSION));
        assert!(server_caps.supports_delta && client_caps.supports_delta);
        assert!(!(server_caps.supports_compression && client_caps.supports_compression));
        assert!(client_caps.max_blob_size <= server_caps.max_blob_size);
    }

    #[test]
    fn test_capability_version_mismatch() {
        let server_caps = ServerCapabilities {
            supports_delta: false,
            supports_compression: false,
            max_blob_size: 1024,
            protocol_versions: vec![1],
        };
        let client_caps = ClientCapabilities {
            supports_delta: true,
            supports_compression: true,
            max_blob_size: 10 * 1024 * 1024,
        };
        assert!(!server_caps.protocol_versions.contains(&PROTOCOL_VERSION_V2));
        assert!(!server_caps.supports_delta || !client_caps.supports_delta);
        assert!(client_caps.max_blob_size > server_caps.max_blob_size);
    }

    #[test]
    fn test_push_request_v2_roundtrip() {
        let req = PushRequestV2 {
            repo_id: "my-repo".to_string(),
            patches: vec![make_patch("a".repeat(64).as_str(), "Create", &[])],
            branches: vec![make_branch("main", "a".repeat(64).as_str())],
            blobs: vec![BlobRef {
                hash: make_hash("abc"),
                data: "dGVzdA==".to_string(),
                truncated: false,
            }],
            deltas: vec![BlobDelta {
                base_hash: make_hash("base"),
                target_hash: make_hash("target"),
                encoding: DeltaEncoding::BinaryPatch,
                delta_data: "ZGVsdGE=".to_string(),
            }],
            signature: None,
            known_branches: None,
            force: false,
        };
        let rt: PushRequestV2 = roundtrip(&req);
        assert_eq!(rt.repo_id, "my-repo");
        assert_eq!(rt.deltas.len(), 1);
        assert_eq!(rt.patches.len(), 1);
    }

    #[test]
    fn test_pull_response_v2_roundtrip() {
        let resp = PullResponseV2 {
            success: true,
            error: None,
            error_code: None,
            patches: vec![make_patch("a".repeat(64).as_str(), "Create", &[])],
            branches: vec![make_branch("main", "a".repeat(64).as_str())],
            blobs: vec![BlobRef {
                hash: make_hash("abc"),
                data: "dGVzdA==".to_string(),
                truncated: false,
            }],
            deltas: vec![BlobDelta {
                base_hash: make_hash("old"),
                target_hash: make_hash("new"),
                encoding: DeltaEncoding::FullBlob,
                delta_data: "ZnVsbA==".to_string(),
            }],
            protocol_version: 2,
        };
        let rt: PullResponseV2 = roundtrip(&resp);
        assert!(rt.success);
        assert_eq!(rt.protocol_version, 2);
        assert_eq!(rt.deltas.len(), 1);
        assert_eq!(rt.blobs.len(), 1);
    }

    #[test]
    fn test_protocol_versions() {
        assert_eq!(PROTOCOL_VERSION, 1);
        assert_eq!(PROTOCOL_VERSION_V2, 2);
        assert_ne!(PROTOCOL_VERSION, PROTOCOL_VERSION_V2);
    }

    #[test]
    fn test_auth_request_roundtrip() {
        let req = AuthRequest {
            method: AuthMethod::Token("secret".to_string()),
            timestamp: 12345,
        };
        let rt: AuthRequest = roundtrip(&req);
        assert_eq!(rt.timestamp, 12345);
        match rt.method {
            AuthMethod::Token(t) => assert_eq!(t, "secret"),
            _ => panic!("expected Token auth method"),
        }
    }
}
