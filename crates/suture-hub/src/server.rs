use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::storage::HubStorage;
pub use crate::types::*;

pub struct SutureHubServer {
    storage: Arc<Mutex<HubStorage>>,
    no_auth: bool,
}

impl Default for SutureHubServer {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl SutureHubServer {
    pub fn new() -> Self {
        Self::new_in_memory()
    }

    pub fn new_in_memory() -> Self {
        Self {
            storage: Arc::new(Mutex::new(
                HubStorage::open_in_memory().expect("in-memory storage must open"),
            )),
            no_auth: false,
        }
    }

    pub fn with_db(path: &std::path::Path) -> Result<Self, crate::storage::StorageError> {
        Ok(Self {
            storage: Arc::new(Mutex::new(HubStorage::open(path)?)),
            no_auth: false,
        })
    }

    pub fn set_no_auth(&mut self, no_auth: bool) {
        self.no_auth = no_auth;
    }

    pub fn is_no_auth(&self) -> bool {
        self.no_auth
    }

    pub fn storage(&self) -> &Arc<Mutex<HubStorage>> {
        &self.storage
    }

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
        if let Some(ref sig_bytes) = req.signature {
            let store = self.storage.lock().await;
            if store.has_authorized_keys().unwrap_or(false)
                && let Err(e) = verify_push_signature(&store, &req, sig_bytes)
            {
                return Err((
                    StatusCode::FORBIDDEN,
                    PushResponse {
                        success: false,
                        error: Some(format!("authentication failed: {e}")),
                        existing_patches: vec![],
                    },
                ));
            }
        } else if !self.no_auth {
            let store = self.storage.lock().await;
            if store.has_authorized_keys().unwrap_or(false) || store.has_tokens().unwrap_or(false) {
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
        if let Err(e) = store.ensure_repo(&req.repo_id) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                PushResponse {
                    success: false,
                    error: Some(format!("storage error: {e}")),
                    existing_patches: vec![],
                },
            ));
        }

        let mut existing_patches = Vec::new();

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
            if let Err(e) = store.store_blob(&req.repo_id, &hex, &data) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    PushResponse {
                        success: false,
                        error: Some(format!("storage error: {e}")),
                        existing_patches: vec![],
                    },
                ));
            }
        }

        for patch in &req.patches {
            let inserted = match store.insert_patch(&req.repo_id, patch) {
                Ok(i) => i,
                Err(e) => {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        PushResponse {
                            success: false,
                            error: Some(format!("storage error: {e}")),
                            existing_patches: vec![],
                        },
                    ));
                }
            };
            if !inserted {
                existing_patches.push(patch.id.clone());
            }
        }

        for branch in &req.branches {
            let target_hex = hash_to_hex(&branch.target_id);
            if let Err(e) = store.set_branch(&req.repo_id, &branch.name, &target_hex) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    PushResponse {
                        success: false,
                        error: Some(format!("storage error: {e}")),
                        existing_patches: vec![],
                    },
                ));
            }
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

    pub async fn handle_mirror_setup(
        &self,
        req: crate::types::MirrorSetupRequest,
    ) -> crate::types::MirrorSetupResponse {
        if let Err(e) = validate_mirror_url(&req.upstream_url) {
            return crate::types::MirrorSetupResponse {
                success: false,
                error: Some(format!("invalid upstream URL: {e}")),
                mirror_id: None,
            };
        }

        let store = self.storage.lock().await;

        match store.add_mirror(&req.repo_name, &req.upstream_url, &req.upstream_repo) {
            Ok(mirror_id) => {
                if let Err(e) = store.ensure_repo(&req.repo_name) {
                    return crate::types::MirrorSetupResponse {
                        success: false,
                        error: Some(format!("failed to create repo: {e}")),
                        mirror_id: None,
                    };
                }
                crate::types::MirrorSetupResponse {
                    success: true,
                    error: None,
                    mirror_id: Some(mirror_id),
                }
            }
            Err(e) => crate::types::MirrorSetupResponse {
                success: false,
                error: Some(format!("failed to register mirror: {e}")),
                mirror_id: None,
            },
        }
    }

    pub async fn handle_mirror_sync(
        &self,
        req: crate::types::MirrorSyncRequest,
    ) -> crate::types::MirrorSyncResponse {
        let store = self.storage.lock().await;

        let mirror_info = match store.get_mirror(req.mirror_id) {
            Ok(Some(info)) => info,
            Ok(None) => {
                return crate::types::MirrorSyncResponse {
                    success: false,
                    error: Some(format!("mirror {} not found", req.mirror_id)),
                    patches_synced: 0,
                    branches_synced: 0,
                };
            }
            Err(e) => {
                return crate::types::MirrorSyncResponse {
                    success: false,
                    error: Some(format!("database error: {e}")),
                    patches_synced: 0,
                    branches_synced: 0,
                };
            }
        };

        let (local_repo, upstream_url, upstream_repo, _, _) = mirror_info;

        if let Err(e) = validate_mirror_url(&upstream_url) {
            return crate::types::MirrorSyncResponse {
                success: false,
                error: Some(format!("invalid upstream URL: {e}")),
                patches_synced: 0,
                branches_synced: 0,
            };
        }

        if let Err(e) = store.update_mirror_status(req.mirror_id, "syncing", None) {
            return crate::types::MirrorSyncResponse {
                success: false,
                error: Some(format!("failed to update status: {e}")),
                patches_synced: 0,
                branches_synced: 0,
            };
        }

        drop(store);

        let upstream_pull = crate::types::PullRequest {
            repo_id: upstream_repo,
            known_branches: vec![],
            max_depth: None,
        };

        let client = reqwest::Client::new();
        let pull_resp = match client
            .post(format!("{}/pull", upstream_url))
            .json(&upstream_pull)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let store = self.storage.lock().await;
                let _ = store.update_mirror_status(req.mirror_id, "error", None);
                return crate::types::MirrorSyncResponse {
                    success: false,
                    error: Some(format!("failed to reach upstream: {e}")),
                    patches_synced: 0,
                    branches_synced: 0,
                };
            }
        };

        let pull_result: crate::types::PullResponse = match pull_resp.json().await {
            Ok(r) => r,
            Err(e) => {
                let store = self.storage.lock().await;
                let _ = store.update_mirror_status(req.mirror_id, "error", None);
                return crate::types::MirrorSyncResponse {
                    success: false,
                    error: Some(format!("failed to parse upstream response: {e}")),
                    patches_synced: 0,
                    branches_synced: 0,
                };
            }
        };

        if !pull_result.success {
            let store = self.storage.lock().await;
            let _ = store.update_mirror_status(req.mirror_id, "error", None);
            return crate::types::MirrorSyncResponse {
                success: false,
                error: pull_result.error,
                patches_synced: 0,
                branches_synced: 0,
            };
        }

        let store = self.storage.lock().await;
        let mut patches_synced = 0u64;

        for blob in &pull_result.blobs {
            let hex = hash_to_hex(&blob.hash);
            let data = match base64_decode(&blob.data) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let _ = store.store_blob(&local_repo, &hex, &data);
        }

        for patch in &pull_result.patches {
            let inserted = store.insert_patch(&local_repo, patch).unwrap_or(false);
            if inserted {
                patches_synced += 1;
            }
        }

        let branches_synced = pull_result.branches.len() as u64;
        for branch in &pull_result.branches {
            let target_hex = hash_to_hex(&branch.target_id);
            let _ = store.set_branch(&local_repo, &branch.name, &target_hex);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let _ = store.update_mirror_status(req.mirror_id, "idle", Some(now));

        crate::types::MirrorSyncResponse {
            success: true,
            error: None,
            patches_synced,
            branches_synced,
        }
    }

    pub async fn handle_mirror_status(
        &self,
        req: crate::types::MirrorStatusRequest,
    ) -> crate::types::MirrorStatusResponse {
        let store = self.storage.lock().await;

        let mirrors = store.list_mirrors().unwrap_or_default();

        let entries: Vec<crate::types::MirrorStatusEntry> = mirrors
            .into_iter()
            .filter(|m| {
                if let Some(mid) = req.mirror_id
                    && m.0 != mid
                {
                    return false;
                }
                if let Some(ref name) = req.repo_name
                    && &m.1 != name
                {
                    return false;
                }
                true
            })
            .map(|m| crate::types::MirrorStatusEntry {
                mirror_id: m.0,
                repo_name: m.1,
                upstream_url: m.2,
                upstream_repo: m.3,
                last_sync: m.4.map(|v| v as u64),
                status: m.5,
            })
            .collect();

        crate::types::MirrorStatusResponse {
            success: true,
            error: None,
            mirrors: entries,
        }
    }
}

fn validate_mirror_url(url: &str) -> Result<(), &'static str> {
    let parsed = url::Url::parse(url).map_err(|_| "invalid URL syntax")?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("only http and https URLs are allowed"),
    }

    let host = parsed.host_str().ok_or("URL must have a host")?;

    if let Ok(addr) = host.parse::<std::net::IpAddr>() {
        match addr {
            std::net::IpAddr::V4(v4) => {
                if v4.is_loopback() || v4.is_private() || v4.is_link_local() {
                    return Err("private/internal IP addresses are not allowed");
                }
            }
            std::net::IpAddr::V6(v6) => {
                if v6.is_loopback() || v6.is_unicast_link_local() {
                    return Err("private/internal IP addresses are not allowed");
                }
            }
        }
    }

    if host == "169.254.169.254" || host == "metadata.google.internal" {
        return Err("metadata endpoints are not allowed");
    }

    Ok(())
}

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

    let canonical = canonical_push_bytes(req);

    let mut authors: HashSet<&str> = HashSet::new();
    for patch in &req.patches {
        authors.insert(&patch.author);
    }

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
                Err(_) => continue,
            }
        }
    }

    Err("no matching authorized key found for signature".to_string())
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

    let mut reachable: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = all_patches.iter().map(|p| hash_to_hex(&p.id)).collect();

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
                if !client_ancestors.contains(&parent_hex) && !new_ids.contains(&parent_hex) {
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

    let sorted: Vec<PatchProto> = order.into_iter().map(|i| patches[i].clone()).collect();
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

async fn check_auth(hub: &SutureHubServer, headers: &HeaderMap) -> Result<(), StatusCode> {
    if hub.no_auth {
        return Ok(());
    }

    let store = hub.storage.lock().await;
    let auth_keys_configured = store.has_authorized_keys().unwrap_or(false);
    let tokens_exist = store.has_tokens().unwrap_or(false);
    drop(store);

    if !auth_keys_configured && !tokens_exist {
        return Ok(());
    }

    if let Some(auth_header) = headers.get("authorization")
        && let Ok(auth_str) = auth_header.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
    {
        let store = hub.storage.lock().await;
        if store.verify_token(token).unwrap_or(false) {
            return Ok(());
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

fn generate_random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    hex::encode(bytes)
}

pub async fn push_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(req): Json<PushRequest>,
) -> (StatusCode, Json<PushResponse>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(PushResponse {
                success: false,
                error: Some("authentication failed".to_string()),
                existing_patches: vec![],
            }),
        );
    }
    match hub.handle_push(req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)),
        Err((status, resp)) => (status, Json(resp)),
    }
}

pub async fn pull_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(req): Json<PullRequest>,
) -> (StatusCode, Json<PullResponse>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(PullResponse {
                success: false,
                error: Some("authentication failed".to_string()),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
            }),
        );
    }
    let resp = hub.handle_pull(req).await;
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (status, Json(resp))
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

pub async fn handshake_handler(
    Json(req): Json<crate::types::HandshakeRequest>,
) -> Json<crate::types::HandshakeResponse> {
    let compatible = req.client_version == crate::types::PROTOCOL_VERSION;
    Json(crate::types::HandshakeResponse {
        server_version: crate::types::PROTOCOL_VERSION,
        server_name: "suture-hub".to_string(),
        compatible,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub created_at: u64,
}

pub async fn create_token_handler(
    State(hub): State<Arc<SutureHubServer>>,
) -> (StatusCode, Json<TokenResponse>) {
    let token = generate_random_token();
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let store = hub.storage.lock().await;
    if store
        .store_token(&token, created_at, "cli-generated")
        .is_err()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TokenResponse {
                token: String::new(),
                created_at: 0,
            }),
        );
    }

    (StatusCode::OK, Json(TokenResponse { token, created_at }))
}

#[derive(Debug, serde::Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
}

pub async fn verify_token_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(auth_req): Json<crate::types::AuthRequest>,
) -> Json<VerifyResponse> {
    let valid = match &auth_req.method {
        crate::types::AuthMethod::Token(token) => {
            let store = hub.storage.lock().await;
            store.verify_token(token).unwrap_or(false)
        }
        _ => false,
    };
    Json(VerifyResponse { valid })
}

pub async fn mirror_setup_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(req): Json<crate::types::MirrorSetupRequest>,
) -> (StatusCode, Json<crate::types::MirrorSetupResponse>) {
    let resp = hub.handle_mirror_setup(req).await;
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(resp))
}

pub async fn mirror_sync_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(req): Json<crate::types::MirrorSyncRequest>,
) -> (StatusCode, Json<crate::types::MirrorSyncResponse>) {
    let resp = hub.handle_mirror_sync(req).await;
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(resp))
}

pub async fn mirror_status_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(req): Json<crate::types::MirrorStatusRequest>,
) -> (StatusCode, Json<crate::types::MirrorStatusResponse>) {
    let resp = hub.handle_mirror_status(req).await;
    (StatusCode::OK, Json(resp))
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

pub async fn repo_branches_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path(repo_id): Path<String>,
) -> (StatusCode, Json<Vec<BranchProto>>) {
    let store = hub.storage.lock().await;
    let branches = store.get_branches(&repo_id).unwrap_or_default();
    (StatusCode::OK, Json(branches))
}

pub async fn repo_patches_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path(repo_id): Path<String>,
) -> (StatusCode, Json<Vec<PatchProto>>) {
    let store = hub.storage.lock().await;
    let patches = store.get_all_patches(&repo_id).unwrap_or_default();
    (StatusCode::OK, Json(patches))
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn serve_static_file(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    let content_type = if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    };

    let static_dir = std::path::Path::new("static");
    let file_path = static_dir.join(&path);

    let canonical_static = match tokio::fs::canonicalize(static_dir).await {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let canonical_file = match tokio::fs::canonicalize(&file_path).await {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    if !canonical_file.starts_with(&canonical_static) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match tokio::fs::read_to_string(&canonical_file).await {
        Ok(contents) => {
            let headers = [(axum::http::header::CONTENT_TYPE, content_type)];
            (StatusCode::OK, headers, contents).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn run_server(
    hub: SutureHubServer,
    addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = axum::Router::new()
        .route("/", axum::routing::get(serve_index))
        .route("/push", axum::routing::post(push_handler))
        .route("/pull", axum::routing::post(pull_handler))
        .route("/repos", axum::routing::get(list_repos_handler))
        .route("/repo/{repo_id}", axum::routing::get(repo_info_handler))
        .route(
            "/repos/{repo_id}/branches",
            axum::routing::get(repo_branches_handler),
        )
        .route(
            "/repos/{repo_id}/patches",
            axum::routing::get(repo_patches_handler),
        )
        .route("/handshake", axum::routing::get(handshake_handler))
        .route("/handshake", axum::routing::post(handshake_handler))
        .route("/auth/token", axum::routing::post(create_token_handler))
        .route("/auth/verify", axum::routing::post(verify_token_handler))
        .route("/mirror/setup", axum::routing::post(mirror_setup_handler))
        .route("/mirror/sync", axum::routing::post(mirror_sync_handler))
        .route("/mirror/status", axum::routing::get(mirror_status_handler))
        .route("/mirror/status", axum::routing::post(mirror_status_handler))
        .route("/static/{*path}", axum::routing::get(serve_static_file))
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

    pub fn make_patch(id_hex: &str, op: &str, parents: &[&str], author: &str) -> PatchProto {
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
            known_branches: None,
        };

        let resp = hub.handle_push(push_req).await.unwrap();
        assert!(resp.success);
        assert!(resp.existing_patches.is_empty());

        let pull_req = PullRequest {
            repo_id: "test-repo".to_string(),
            known_branches: vec![],
            max_depth: None,
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
            known_branches: None,
        };
        hub.handle_push(push).await.unwrap();

        let pull = PullRequest {
            repo_id: "test-repo".to_string(),
            known_branches: vec![make_branch("main", &b_hex)],
            max_depth: None,
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
            known_branches: None,
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
            known_branches: None,
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
            known_branches: None,
        };
        hub.handle_push(push).await.unwrap();

        let push = PushRequest {
            repo_id: "repo-2".to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![],
            signature: None,
            known_branches: None,
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
            known_branches: None,
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
                max_depth: None,
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
            known_branches: None,
        };
        hub.handle_push(push).await.unwrap();

        let pull = PullRequest {
            repo_id: "blob-repo".to_string(),
            known_branches: vec![],
            max_depth: None,
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

        let keypair = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
        hub.add_authorized_key("alice", &keypair.verifying_key().to_bytes())
            .await
            .unwrap();

        let push = PushRequest {
            repo_id: "auth-test".to_string(),
            patches: vec![make_patch(&"a".repeat(64), "Create", &[], "alice")],
            branches: vec![make_branch("main", &"a".repeat(64))],
            blobs: vec![],
            signature: None,
            known_branches: None,
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

        let keypair = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
        hub.add_authorized_key("alice", &keypair.verifying_key().to_bytes())
            .await
            .unwrap();

        let a_hex = "a".repeat(64);
        let push_req = PushRequest {
            repo_id: "auth-test-2".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
        };

        let canonical = canonical_push_bytes(&push_req);
        let signature = keypair.sign(&canonical);

        let mut signed_req = push_req;
        signed_req.signature = Some(signature.to_bytes().to_vec());

        let resp = hub.handle_push(signed_req).await;
        assert!(resp.is_ok());
        assert!(resp.unwrap().success);
    }

    #[tokio::test]
    async fn test_no_auth_when_no_keys_configured() {
        let hub = SutureHubServer::new();

        let push = PushRequest {
            repo_id: "no-auth-test".to_string(),
            patches: vec![make_patch(&"a".repeat(64), "Create", &[], "alice")],
            branches: vec![make_branch("main", &"a".repeat(64))],
            blobs: vec![],
            signature: None,
            known_branches: None,
        };
        let resp = hub.handle_push(push).await;
        assert!(resp.is_ok());
        assert!(resp.unwrap().success);
    }

    #[tokio::test]
    async fn test_handshake_handler() {
        let req = crate::types::HandshakeRequest {
            client_version: 1,
            client_name: "test-client".to_string(),
        };
        let resp = handshake_handler(Json(req)).await;
        assert!(resp.compatible);
        assert_eq!(resp.server_version, 1);
    }

    #[tokio::test]
    async fn test_handshake_incompatible() {
        let req = crate::types::HandshakeRequest {
            client_version: 99,
            client_name: "test-client".to_string(),
        };
        let resp = handshake_handler(Json(req)).await;
        assert!(!resp.compatible);
    }

    #[tokio::test]
    async fn test_token_creation_and_verification() {
        let hub = Arc::new(SutureHubServer::new());

        let (status, token_resp) = create_token_handler(State(hub.clone())).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!token_resp.token.is_empty());

        let auth_req = crate::types::AuthRequest {
            method: crate::types::AuthMethod::Token(token_resp.token.clone()),
            timestamp: 0,
        };
        let verify_resp = verify_token_handler(State(hub.clone()), Json(auth_req)).await;
        assert!(verify_resp.valid);

        let bad_req = crate::types::AuthRequest {
            method: crate::types::AuthMethod::Token("invalid-token".to_string()),
            timestamp: 0,
        };
        let bad_resp = verify_token_handler(State(hub.clone()), Json(bad_req)).await;
        assert!(!bad_resp.valid);
    }

    #[tokio::test]
    async fn test_no_auth_mode() {
        let mut hub = SutureHubServer::new();
        hub.set_no_auth(true);
        assert!(hub.is_no_auth());
    }
}
