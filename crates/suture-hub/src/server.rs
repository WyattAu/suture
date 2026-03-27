use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::storage::HubStorage;
use crate::types::*;

pub struct SutureHubServer {
    storage: Arc<Mutex<HubStorage>>,
}

impl Default for SutureHubServer {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl SutureHubServer {
    /// Create a new in-memory hub (for testing).
    pub fn new() -> Self {
        Self::new_in_memory()
    }

    /// Create a new in-memory hub.
    pub fn new_in_memory() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HubStorage::open_in_memory().unwrap())),
        }
    }

    /// Create a hub backed by a SQLite database file.
    pub fn with_db(path: &std::path::Path) -> Result<Self, crate::storage::StorageError> {
        Ok(Self {
            storage: Arc::new(Mutex::new(HubStorage::open(path)?)),
        })
    }

    /// Get a reference to the underlying storage (for key management, etc.).
    pub fn storage(&self) -> &Arc<Mutex<HubStorage>> {
        &self.storage
    }

    /// Add an authorized public key for an author.
    pub async fn add_authorized_key(
        &self,
        author: &str,
        public_key_bytes: &[u8],
    ) -> Result<(), crate::storage::StorageError> {
        let store = self.storage.lock().await;
        store.add_authorized_key(author, public_key_bytes)
    }

    pub async fn handle_push(
        &self,
        req: PushRequest,
    ) -> Result<PushResponse, (StatusCode, PushResponse)> {
        // Verify signature if provided and auth is configured
        if let Some(ref sig_bytes) = req.signature {
            let store = self.storage.lock().await;
            if let Err(e) = verify_push_signature(&store, &req, sig_bytes) {
                return Err((
                    StatusCode::FORBIDDEN,
                    PushResponse {
                        success: false,
                        error: Some(format!("authentication failed: {e}")),
                        existing_patches: vec![],
                    },
                ));
            }
        } else {
            // If auth keys are configured, require signature
            let store = self.storage.lock().await;
            if store.has_authorized_keys().unwrap_or(false) {
                return Err((
                    StatusCode::FORBIDDEN,
                    PushResponse {
                        success: false,
                        error: Some("authentication required: no signature provided".to_string()),
                        existing_patches: vec![],
                    },
                ));
            }
        }

        let store = self.storage.lock().await;
        store.ensure_repo(&req.repo_id).unwrap();

        let mut existing_patches = Vec::new();

        // Store blobs
        for blob in &req.blobs {
            let hex = hash_to_hex(&blob.hash);
            let data = match base64_decode(&blob.data) {
                Ok(d) => d,
                Err(e) => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        PushResponse {
                            success: false,
                            error: Some(format!("invalid base64 in blob: {e}")),
                            existing_patches: vec![],
                        },
                    ));
                }
            };
            store.store_blob(&req.repo_id, &hex, &data).unwrap();
        }

        // Store patches
        for patch in &req.patches {
            let inserted = store.insert_patch(&req.repo_id, patch).unwrap();
            if !inserted {
                existing_patches.push(patch.id.clone());
            }
        }

        // Store branches
        for branch in &req.branches {
            let target_hex = hash_to_hex(&branch.target_id);
            store
                .set_branch(&req.repo_id, &branch.name, &target_hex)
                .unwrap();
        }

        Ok(PushResponse {
            success: true,
            error: None,
            existing_patches,
        })
    }

    pub async fn handle_pull(&self, req: PullRequest) -> PullResponse {
        let store = self.storage.lock().await;

        if !store.repo_exists(&req.repo_id).unwrap_or(false) {
            return PullResponse {
                success: false,
                error: Some(format!("repo not found: {}", req.repo_id)),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
            };
        }

        let all_patches = store.get_all_patches(&req.repo_id).unwrap_or_default();
        let client_ancestors = collect_ancestors(&all_patches, &req.known_branches);
        let new_patches = collect_new_patches(&all_patches, &client_ancestors);

        let branches = store.get_branches(&req.repo_id).unwrap_or_default();
        let blobs = store.get_all_blobs(&req.repo_id).unwrap_or_default();

        PullResponse {
            success: true,
            error: None,
            patches: new_patches,
            branches,
            blobs,
        }
    }

    pub async fn handle_list_repos(&self) -> ListReposResponse {
        let store = self.storage.lock().await;
        ListReposResponse {
            repo_ids: store.list_repos().unwrap_or_default(),
        }
    }

    pub async fn handle_repo_info(&self, repo_id: &str) -> RepoInfoResponse {
        let store = self.storage.lock().await;

        if !store.repo_exists(repo_id).unwrap_or(false) {
            return RepoInfoResponse {
                repo_id: repo_id.to_string(),
                patch_count: 0,
                branches: vec![],
                success: false,
                error: Some(format!("repo not found: {repo_id}")),
            };
        }

        let patch_count = store.patch_count(repo_id).unwrap_or(0);
        let branches = store.get_branches(repo_id).unwrap_or_default();

        RepoInfoResponse {
            repo_id: repo_id.to_string(),
            patch_count,
            branches,
            success: true,
            error: None,
        }
    }
}

/// Verify the Ed25519 signature on a push request.
fn verify_push_signature(
    store: &HubStorage,
    req: &PushRequest,
    sig_bytes: &[u8],
) -> Result<(), String> {
    if sig_bytes.len() != 64 {
        return Err("signature must be 64 bytes".to_string());
    }
    let signature = Signature::from_bytes(
        sig_bytes
            .try_into()
            .map_err(|_| "invalid signature length")?,
    );

    // Build canonical bytes from the request (without signature)
    let canonical = canonical_push_bytes(req);

    // Try to find a matching authorized key
    // Check all authors mentioned in the patches
    let mut authors: HashSet<&str> = HashSet::new();
    for patch in &req.patches {
        authors.insert(&patch.author);
    }
    // Also include the repo_id's implicit "owner" — but we primarily check patch authors

    for author in &authors {
        if let Some(pub_key_bytes) = store.get_authorized_key(author).unwrap_or(None) {
            if pub_key_bytes.len() != 32 {
                continue;
            }
            let pub_key_array: [u8; 32] = pub_key_bytes
                .try_into()
                .map_err(|_| "invalid public key length")?;
            let verifying_key = VerifyingKey::from_bytes(&pub_key_array)
                .map_err(|e| format!("invalid public key: {e}"))?;
            match verifying_key.verify(&canonical, &signature) {
                Ok(()) => return Ok(()),
                Err(_) => continue, // try next author
            }
        }
    }

    Err("no matching authorized key found for signature".to_string())
}

/// Build canonical bytes for push request signing.
/// Format: repo_id \0 patch_count \0 (each patch: id \0 op \0 author \0 msg \0 timestamp \0) ... branch_count \0 (each: name \0 target \0) ...
fn canonical_push_bytes(req: &PushRequest) -> Vec<u8> {
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

fn collect_ancestors(
    all_patches: &[PatchProto],
    known_branches: &[BranchProto],
) -> HashSet<String> {
    let patch_map: std::collections::HashMap<String, &PatchProto> = all_patches
        .iter()
        .map(|p| (hash_to_hex(&p.id), p))
        .collect();

    let mut ancestors = HashSet::new();
    let mut stack: Vec<String> = known_branches
        .iter()
        .filter_map(|b| {
            let hex = hash_to_hex(&b.target_id);
            if patch_map.contains_key(&hex) {
                Some(hex)
            } else {
                None
            }
        })
        .collect();

    while let Some(id_hex) = stack.pop() {
        if ancestors.insert(id_hex.clone())
            && let Some(patch) = patch_map.get(&id_hex)
        {
            for parent in &patch.parent_ids {
                let parent_hex = hash_to_hex(parent);
                if !ancestors.contains(&parent_hex) {
                    stack.push(parent_hex);
                }
            }
        }
    }

    ancestors
}

fn collect_new_patches(
    all_patches: &[PatchProto],
    client_ancestors: &HashSet<String>,
) -> Vec<PatchProto> {
    let patch_map: std::collections::HashMap<String, &PatchProto> = all_patches
        .iter()
        .map(|p| (hash_to_hex(&p.id), p))
        .collect();

    // Collect branch tips from all patches' parent relationships
    // We need to know which patches are "reachable" from any chain
    let mut reachable: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = all_patches
        .iter()
        .map(|p| hash_to_hex(&p.id))
        .collect();

    while let Some(id_hex) = stack.pop() {
        if reachable.insert(id_hex.clone())
            && let Some(patch) = patch_map.get(&id_hex)
        {
            for parent in &patch.parent_ids {
                let parent_hex = hash_to_hex(parent);
                if !reachable.contains(&parent_hex) {
                    stack.push(parent_hex);
                }
            }
        }
    }

    // New patches = reachable but not in client_ancestors
    let mut new_ids: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = reachable
        .into_iter()
        .filter(|id| !client_ancestors.contains(id))
        .collect();

    while let Some(id_hex) = stack.pop() {
        if new_ids.insert(id_hex.clone())
            && let Some(patch) = patch_map.get(&id_hex)
        {
            for parent in &patch.parent_ids {
                let parent_hex = hash_to_hex(parent);
                if !client_ancestors.contains(&parent_hex)
                    && !new_ids.contains(&parent_hex)
                {
                    stack.push(parent_hex);
                }
            }
        }
    }

    let mut result: Vec<PatchProto> = new_ids
        .into_iter()
        .filter_map(|id| patch_map.get(&id).map(|p| (*p).clone()))
        .collect();

    topological_sort(&mut result);
    result
}

fn topological_sort(patches: &mut Vec<PatchProto>) {
    let index_map: std::collections::HashMap<String, usize> = patches
        .iter()
        .enumerate()
        .map(|(i, p)| (hash_to_hex(&p.id), i))
        .collect();

    let n = patches.len();
    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);

    for i in 0..n {
        if !visited[i] {
            dfs(i, patches, &index_map, &mut visited, &mut order);
        }
    }

    let sorted: Vec<PatchProto> = order
        .into_iter()
        .map(|i| patches[i].clone())
        .collect();
    *patches = sorted;
}

fn dfs(
    idx: usize,
    patches: &[PatchProto],
    index_map: &std::collections::HashMap<String, usize>,
    visited: &mut [bool],
    order: &mut Vec<usize>,
) {
    visited[idx] = true;
    let patch = &patches[idx];
    for parent in &patch.parent_ids {
        let parent_hex = hash_to_hex(parent);
        if let Some(&parent_idx) = index_map.get(&parent_hex)
            && !visited[parent_idx]
        {
            dfs(parent_idx, patches, index_map, visited, order);
        }
    }
    order.push(idx);
}

pub async fn push_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(req): Json<PushRequest>,
) -> (StatusCode, Json<PushResponse>) {
    match hub.handle_push(req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)),
        Err((status, resp)) => (status, Json(resp)),
    }
}

pub async fn pull_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(req): Json<PullRequest>,
) -> Json<PullResponse> {
    Json(hub.handle_pull(req).await)
}

pub async fn list_repos_handler(
    State(hub): State<Arc<SutureHubServer>>,
) -> Json<ListReposResponse> {
    Json(hub.handle_list_repos().await)
}

pub async fn repo_info_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path(repo_id): Path<String>,
) -> (StatusCode, Json<RepoInfoResponse>) {
    let resp = hub.handle_repo_info(&repo_id).await;
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (status, Json(resp))
}

#[cfg(test)]
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| e.to_string())
}

pub async fn run_server(hub: SutureHubServer, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let app = axum::Router::new()
        .route("/push", axum::routing::post(push_handler))
        .route("/pull", axum::routing::post(pull_handler))
        .route("/repos", axum::routing::get(list_repos_handler))
        .route("/repo/{repo_id}", axum::routing::get(repo_info_handler))
        .with_state(Arc::new(hub));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Suture Hub listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;

    pub fn make_hash_proto(hex: &str) -> HashProto {
        HashProto {
            value: hex.to_string(),
        }
    }

    pub fn make_patch(
        id_hex: &str,
        op: &str,
        parents: &[&str],
        author: &str,
    ) -> PatchProto {
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

    pub fn make_branch(name: &str, target: &str) -> BranchProto {
        BranchProto {
            name: name.to_string(),
            target_id: make_hash_proto(target),
        }
    }

    #[tokio::test]
    async fn test_push_and_pull() {
        let hub = SutureHubServer::new();
        let a_hex = "a".repeat(64);
        let b_hex = "b".repeat(64);

        let p1 = make_patch(&a_hex, "Create", &[], "alice");
        let p2 = make_patch(&b_hex, "Modify", &[&a_hex], "alice");

        let push_req = PushRequest {
            repo_id: "test-repo".to_string(),
            patches: vec![p1.clone(), p2.clone()],
            branches: vec![make_branch("main", &b_hex)],
            blobs: vec![],
            signature: None,
        };

        let resp = hub.handle_push(push_req).await.unwrap();
        assert!(resp.success);
        assert!(resp.existing_patches.is_empty());

        let pull_req = PullRequest {
            repo_id: "test-repo".to_string(),
            known_branches: vec![],
        };

        let pull_resp = hub.handle_pull(pull_req).await;
        assert!(pull_resp.success);
        assert_eq!(pull_resp.patches.len(), 2);
        assert_eq!(hash_to_hex(&pull_resp.patches[0].id), a_hex);
        assert_eq!(hash_to_hex(&pull_resp.patches[1].id), b_hex);
        assert_eq!(pull_resp.branches.len(), 1);
    }

    #[tokio::test]
    async fn test_pull_with_known_branch() {
        let hub = SutureHubServer::new();
        let a_hex = "a".repeat(64);
        let b_hex = "b".repeat(64);
        let c_hex = "c".repeat(64);

        let p1 = make_patch(&a_hex, "Create", &[], "alice");
        let p2 = make_patch(&b_hex, "Modify", &[&a_hex], "alice");
        let p3 = make_patch(&c_hex, "Modify", &[&b_hex], "alice");

        let push = PushRequest {
            repo_id: "test-repo".to_string(),
            patches: vec![p1, p2, p3],
            branches: vec![make_branch("main", &c_hex)],
            blobs: vec![],
            signature: None,
        };
        hub.handle_push(push).await.unwrap();

        let pull = PullRequest {
            repo_id: "test-repo".to_string(),
            known_branches: vec![make_branch("main", &b_hex)],
        };
        let resp = hub.handle_pull(pull).await;
        assert!(resp.success);
        assert_eq!(resp.patches.len(), 1);
        assert_eq!(hash_to_hex(&resp.patches[0].id), c_hex);
    }

    #[tokio::test]
    async fn test_push_existing_patch() {
        let hub = SutureHubServer::new();
        let a_hex = "a".repeat(64);

        let p1 = make_patch(&a_hex, "Create", &[], "alice");

        let push1 = PushRequest {
            repo_id: "test-repo".to_string(),
            patches: vec![p1.clone()],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
        };
        let resp1 = hub.handle_push(push1).await.unwrap();
        assert!(resp1.success);
        assert!(resp1.existing_patches.is_empty());

        let push2 = PushRequest {
            repo_id: "test-repo".to_string(),
            patches: vec![p1.clone()],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
        };
        let resp2 = hub.handle_push(push2).await.unwrap();
        assert!(resp2.success);
        assert_eq!(resp2.existing_patches.len(), 1);
    }

    #[tokio::test]
    async fn test_list_repos() {
        let hub = SutureHubServer::new();

        let push = PushRequest {
            repo_id: "repo-1".to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![],
            signature: None,
        };
        hub.handle_push(push).await.unwrap();

        let push = PushRequest {
            repo_id: "repo-2".to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![],
            signature: None,
        };
        hub.handle_push(push).await.unwrap();

        let resp = hub.handle_list_repos().await;
        assert_eq!(resp.repo_ids.len(), 2);
    }

    #[tokio::test]
    async fn test_repo_info() {
        let hub = SutureHubServer::new();
        let a_hex = "a".repeat(64);

        let push = PushRequest {
            repo_id: "my-repo".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
        };
        hub.handle_push(push).await.unwrap();

        let resp = hub.handle_repo_info("my-repo").await;
        assert!(resp.success);
        assert_eq!(resp.patch_count, 1);
        assert_eq!(resp.branches.len(), 1);

        let resp = hub.handle_repo_info("nonexistent").await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_pull_nonexistent_repo() {
        let hub = SutureHubServer::new();
        let resp = hub
            .handle_pull(PullRequest {
                repo_id: "nope".to_string(),
                known_branches: vec![],
            })
            .await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_blobs_roundtrip() {
        let hub = SutureHubServer::new();
        let blob_data = b"hello world";

        let push = PushRequest {
            repo_id: "blob-repo".to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![BlobRef {
                hash: make_hash_proto(&"deadbeef".repeat(8)),
                data: base64_encode(blob_data),
            }],
            signature: None,
        };
        hub.handle_push(push).await.unwrap();

        let pull = PullRequest {
            repo_id: "blob-repo".to_string(),
            known_branches: vec![],
        };
        let resp = hub.handle_pull(pull).await;
        assert_eq!(resp.blobs.len(), 1);
        let decoded = base64_decode(&resp.blobs[0].data).unwrap();
        assert_eq!(decoded, blob_data);
    }

    #[tokio::test]
    async fn test_topological_sort() {
        let a_hex = "a".repeat(64);
        let b_hex = "b".repeat(64);
        let c_hex = "c".repeat(64);

        let mut patches = vec![
            make_patch(&c_hex, "Modify", &[&b_hex], "alice"),
            make_patch(&a_hex, "Create", &[], "alice"),
            make_patch(&b_hex, "Modify", &[&a_hex], "alice"),
        ];

        topological_sort(&mut patches);

        assert_eq!(hash_to_hex(&patches[0].id), a_hex);
        assert_eq!(hash_to_hex(&patches[1].id), b_hex);
        assert_eq!(hash_to_hex(&patches[2].id), c_hex);
    }

    #[tokio::test]
    async fn test_auth_required_when_keys_exist() {
        let hub = SutureHubServer::new();

        // Add an authorized key
        let keypair = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
        hub.add_authorized_key("alice", &keypair.verifying_key().to_bytes())
            .await
            .unwrap();

        // Push without signature should fail
        let push = PushRequest {
            repo_id: "auth-test".to_string(),
            patches: vec![make_patch(&"a".repeat(64), "Create", &[], "alice")],
            branches: vec![make_branch("main", &"a".repeat(64))],
            blobs: vec![],
            signature: None,
        };
        let resp = hub.handle_push(push).await;
        assert!(resp.is_err());
        let (status, body) = resp.unwrap_err();
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!body.success);
    }

    #[tokio::test]
    async fn test_auth_succeeds_with_valid_signature() {
        let hub = SutureHubServer::new();

        // Generate keypair and register public key
        let keypair = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
        hub.add_authorized_key("alice", &keypair.verifying_key().to_bytes())
            .await
            .unwrap();

        // Build push request
        let a_hex = "a".repeat(64);
        let push_req = PushRequest {
            repo_id: "auth-test-2".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
        };

        // Sign it
        let canonical = canonical_push_bytes(&push_req);
        let signature = keypair.sign(&canonical);

        let mut signed_req = push_req;
        signed_req.signature = Some(signature.to_bytes().to_vec());

        // Push should succeed
        let resp = hub.handle_push(signed_req).await;
        assert!(resp.is_ok());
        assert!(resp.unwrap().success);
    }

    #[tokio::test]
    async fn test_no_auth_when_no_keys_configured() {
        let hub = SutureHubServer::new();

        // No keys configured — push without signature should succeed
        let push = PushRequest {
            repo_id: "no-auth-test".to_string(),
            patches: vec![make_patch(&"a".repeat(64), "Create", &[], "alice")],
            branches: vec![make_branch("main", &"a".repeat(64))],
            blobs: vec![],
            signature: None,
        };
        let resp = hub.handle_push(push).await;
        assert!(resp.is_ok());
        assert!(resp.unwrap().success);
    }
}
