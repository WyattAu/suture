use axum::{
    Json,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::storage::HubStorage;
pub use crate::types::*;
use crate::storage::{ReplicationEntry, ReplicationStatus};

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Admin,
    Member,
    Reader,
}

impl Role {
    pub fn from_str(s: &str) -> Self {
        match s {
            "admin" => Role::Admin,
            "member" => Role::Member,
            "reader" => Role::Reader,
            _ => Role::Reader,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Member => "member",
            Role::Reader => "reader",
        }
    }
}

impl PartialOrd for Role {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        fn rank(r: &Role) -> u8 {
            match r {
                Role::Admin => 3,
                Role::Member => 2,
                Role::Reader => 1,
            }
        }
        rank(self).partial_cmp(&rank(other))
    }
}

#[derive(serde::Deserialize)]
pub struct PaginationParams {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

pub struct SutureHubServer {
    storage: Arc<RwLock<HubStorage>>,
    no_auth: bool,
    rate_limits: Arc<std::sync::RwLock<std::collections::HashMap<String, (u32, std::time::Instant)>>>,
    max_pushes_per_hour: u32,
    max_pulls_per_hour: u32,
    rate_limit_window: std::time::Duration,
    replication_role: Arc<std::sync::RwLock<String>>,
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
            storage: Arc::new(RwLock::new(
                HubStorage::open_in_memory().expect("in-memory storage must open"),
            )),
            no_auth: false,
            rate_limits: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            max_pushes_per_hour: 100,
            max_pulls_per_hour: 1000,
            rate_limit_window: std::time::Duration::from_secs(60),
            replication_role: Arc::new(std::sync::RwLock::new("standalone".to_string())),
        }
    }

    pub fn with_db(path: &std::path::Path) -> Result<Self, crate::storage::StorageError> {
        Ok(Self {
            storage: Arc::new(RwLock::new(HubStorage::open(path)?)),
            no_auth: false,
            rate_limits: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            max_pushes_per_hour: 100,
            max_pulls_per_hour: 1000,
            rate_limit_window: std::time::Duration::from_secs(60),
            replication_role: Arc::new(std::sync::RwLock::new("standalone".to_string())),
        })
    }

    pub fn set_no_auth(&mut self, no_auth: bool) {
        self.no_auth = no_auth;
    }

    pub fn is_no_auth(&self) -> bool {
        self.no_auth
    }

    pub fn storage(&self) -> &Arc<RwLock<HubStorage>> {
        &self.storage
    }

    pub fn set_rate_limit_config(
        &mut self,
        pushes: u32,
        pulls: u32,
        window: std::time::Duration,
    ) {
        self.max_pushes_per_hour = pushes;
        self.max_pulls_per_hour = pulls;
        self.rate_limit_window = window;
    }

    pub fn set_replication_role(&self, role: &str) {
        *self.replication_role.write().unwrap() = role.to_string();
    }

    pub fn get_replication_role(&self) -> String {
        self.replication_role.read().unwrap().clone()
    }

    pub async fn log_write(
        &self,
        operation: &str,
        table_name: &str,
        row_id: &str,
        data: Option<&str>,
    ) -> Result<i64, crate::storage::StorageError> {
        let store = self.storage.write().await;
        store.log_operation(operation, table_name, row_id, data)
    }

    pub async fn handle_add_peer(&self, req: AddPeerRequest) -> AddPeerResponse {
        let store = self.storage.write().await;
        match store.add_replication_peer(&req.peer_url, &req.role) {
            Ok(peer_id) => AddPeerResponse {
                success: true,
                peer_id: Some(peer_id),
                error: None,
            },
            Err(e) => AddPeerResponse {
                success: false,
                peer_id: None,
                error: Some(format!("{e}")),
            },
        }
    }

    pub async fn handle_remove_peer(&self, id: i64) -> RemovePeerResponse {
        let store = self.storage.write().await;
        match store.remove_replication_peer(id) {
            Ok(()) => RemovePeerResponse {
                success: true,
                error: None,
            },
            Err(e) => RemovePeerResponse {
                success: false,
                error: Some(format!("{e}")),
            },
        }
    }

    pub async fn handle_list_peers(&self) -> ListPeersResponse {
        let store = self.storage.read().await;
        ListPeersResponse {
            peers: store.list_replication_peers().unwrap_or_default(),
        }
    }

    pub async fn handle_replication_status(&self) -> ReplicationStatusResponse {
        let store = self.storage.read().await;
        ReplicationStatusResponse {
            status: store.get_replication_status().unwrap_or(ReplicationStatus {
                current_seq: 0,
                peer_count: 0,
                peers: vec![],
            }),
        }
    }

    pub async fn handle_replication_sync(&self, entries: Vec<ReplicationEntry>) -> SyncResponse {
        let store = self.storage.write().await;
        match store.apply_replication_entries(&entries) {
            Ok(()) => SyncResponse {
                success: true,
                applied: entries.len(),
                error: None,
            },
            Err(e) => SyncResponse {
                success: false,
                applied: 0,
                error: Some(format!("{e}")),
            },
        }
    }

    pub fn check_rate_limit(&self, ip: &str, key: &str) -> Result<(), u64> {
        let window = self.rate_limit_window;
        if window.is_zero() {
            return Ok(());
        }

        let full_key = format!("{}:{}", key, ip);
        let now = std::time::Instant::now();
        let mut limits = self.rate_limits.write().unwrap();

        limits.retain(|_, (_, start)| now.duration_since(*start) < window);

        let limit = match key {
            "push" => self.max_pushes_per_hour,
            "pull" => self.max_pulls_per_hour,
            _ => return Ok(()),
        };

        if let Some(&(count, window_start)) = limits.get(&full_key) {
            if count >= limit {
                let elapsed = now.duration_since(window_start);
                let remaining = window.saturating_sub(elapsed);
                let retry_after = remaining.as_secs().max(1);
                return Err(retry_after);
            }
            limits.insert(full_key, (count + 1, window_start));
        } else {
            limits.insert(full_key, (1, now));
        }

        Ok(())
    }

    pub async fn handle_repo_patches(
        &self,
        repo_id: &str,
        offset: u32,
        limit: u32,
    ) -> Vec<PatchProto> {
        let store = self.storage.read().await;
        let patches = store.get_all_patches(repo_id).unwrap_or_default();
        let offset = offset as usize;
        let limit = limit.min(200) as usize;
        patches.into_iter().skip(offset).take(limit).collect()
    }

    pub async fn add_authorized_key(
        &self,
        author: &str,
        public_key_bytes: &[u8],
    ) -> Result<(), crate::storage::StorageError> {
        let store = self.storage.write().await;
        store.add_authorized_key(author, public_key_bytes)
    }

    pub async fn handle_push(
        &self,
        req: PushRequest,
    ) -> Result<PushResponse, (StatusCode, PushResponse)> {
        if let Some(ref sig_bytes) = req.signature {
            let store = self.storage.read().await;
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
            let store = self.storage.read().await;
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

        let store = self.storage.write().await;
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

            if !req.force
                && let Some(ref known) = req.known_branches
                && let Some(known_branch) = known.iter().find(|kb| kb.name == branch.name)
            {
                let known_target = hash_to_hex(&known_branch.target_id);
                if known_target != target_hex
                    && let Ok(Some(current_target)) =
                        store.get_branch_target(&req.repo_id, &branch.name)
                    && !store
                        .is_ancestor(&req.repo_id, &current_target, &target_hex)
                        .unwrap_or(false)
                {
                    return Err((
                        StatusCode::CONFLICT,
                        PushResponse {
                            success: false,
                            error: Some(format!(
                                "branch '{}' rejected: non-fast-forward push (use --force to override)",
                                branch.name
                            )),
                            existing_patches: vec![],
                        },
                    ));
                }
            }

            if store
                .is_branch_protected(&req.repo_id, &branch.name)
                .unwrap_or(false)
            {
                let push_authors: std::collections::HashSet<&str> =
                    req.patches.iter().map(|p| p.author.as_str()).collect();
                let is_owner =
                    push_authors.len() == 1 && push_authors.contains(branch.name.as_str());
                if !is_owner {
                    return Err((
                        StatusCode::FORBIDDEN,
                        PushResponse {
                            success: false,
                            error: Some(format!(
                                "branch '{}' is protected and can only be updated by its owner",
                                branch.name
                            )),
                            existing_patches: vec![],
                        },
                    ));
                }
            }

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
        let store = self.storage.read().await;

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
        let mut new_patches = collect_new_patches(&all_patches, &client_ancestors);

        if let Some(depth) = req.max_depth {
            new_patches.truncate(depth as usize);
        }

        let branches = store.get_branches(&req.repo_id).unwrap_or_default();

        let mut needed_hashes: std::collections::HashSet<String> = std::collections::HashSet::new();
        for patch in &new_patches {
            if patch.operation_type == "batch" {
                if let Ok(decoded) = base64_decode(&patch.payload)
                    && let Ok(changes) = serde_json::from_str::<Vec<serde_json::Value>>(
                        &String::from_utf8_lossy(&decoded),
                    )
                {
                    for change in &changes {
                        if let Some(payload_val) = change.get("payload").and_then(|v| v.as_array())
                        {
                            let hex_bytes: Vec<u8> = payload_val
                                .iter()
                                .filter_map(|v| v.as_u64().map(|n| n as u8))
                                .collect();
                            if let Ok(hex) = String::from_utf8(hex_bytes) {
                                needed_hashes.insert(hex);
                            }
                        }
                    }
                }
            } else if !patch.payload.is_empty() {
                // Payload may be raw hex (from tests) or base64-encoded (from CLI).
                // Try raw hex first — if it looks like a hex hash, use it directly.
                // Otherwise try base64 decode.
                let hex = if patch.payload.chars().all(|c| c.is_ascii_hexdigit()) {
                    patch.payload.clone()
                } else if let Ok(decoded) = base64_decode(&patch.payload) {
                    String::from_utf8_lossy(&decoded).to_string()
                } else {
                    patch.payload.clone()
                };
                needed_hashes.insert(hex);
            }
        }
        let blobs = store
            .get_blobs(&req.repo_id, &needed_hashes)
            .unwrap_or_default();

        PullResponse {
            success: true,
            error: None,
            patches: new_patches,
            branches,
            blobs,
        }
    }

    pub async fn handle_list_repos(&self) -> ListReposResponse {
        let store = self.storage.read().await;
        ListReposResponse {
            repo_ids: store.list_repos().unwrap_or_default(),
        }
    }

    pub async fn handle_repo_info(&self, repo_id: &str) -> RepoInfoResponse {
        let store = self.storage.read().await;

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

        let store = self.storage.write().await;

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
        let store = self.storage.write().await;

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
                let store = self.storage.write().await;
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
                let store = self.storage.write().await;
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
            let store = self.storage.write().await;
            let _ = store.update_mirror_status(req.mirror_id, "error", None);
            return crate::types::MirrorSyncResponse {
                success: false,
                error: pull_result.error,
                patches_synced: 0,
                branches_synced: 0,
            };
        }

        let store = self.storage.write().await;
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
        let store = self.storage.read().await;

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

    pub async fn handle_pull_v2(&self, req: crate::types::PullRequestV2) -> crate::types::PullResponseV2 {
        let store = self.storage.read().await;

        if !store.repo_exists(&req.repo_id).unwrap_or(false) {
            return crate::types::PullResponseV2 {
                success: false,
                error: Some(format!("repo not found: {}", req.repo_id)),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
                deltas: vec![],
                protocol_version: crate::types::PROTOCOL_VERSION_V2,
            };
        }

        let all_patches = store.get_all_patches(&req.repo_id).unwrap_or_default();
        let client_ancestors = collect_ancestors(&all_patches, &req.known_branches);
        let mut new_patches = collect_new_patches(&all_patches, &client_ancestors);

        if let Some(depth) = req.max_depth {
            new_patches.truncate(depth as usize);
        }

        let branches = store.get_branches(&req.repo_id).unwrap_or_default();

        let mut needed_hashes: std::collections::HashSet<String> = std::collections::HashSet::new();
        for patch in &new_patches {
            if patch.operation_type == "batch" {
                if let Ok(decoded) = base64_decode(&patch.payload)
                    && let Ok(changes) = serde_json::from_str::<Vec<serde_json::Value>>(
                        &String::from_utf8_lossy(&decoded),
                    )
                {
                    for change in &changes {
                        if let Some(payload_val) = change.get("payload").and_then(|v| v.as_array())
                        {
                            let hex_bytes: Vec<u8> = payload_val
                                .iter()
                                .filter_map(|v| v.as_u64().map(|n| n as u8))
                                .collect();
                            if let Ok(hex) = String::from_utf8(hex_bytes) {
                                needed_hashes.insert(hex);
                            }
                        }
                    }
                }
            } else if !patch.payload.is_empty() {
                let hex = if patch.payload.chars().all(|c| c.is_ascii_hexdigit()) {
                    patch.payload.clone()
                } else if let Ok(decoded) = base64_decode(&patch.payload) {
                    String::from_utf8_lossy(&decoded).to_string()
                } else {
                    patch.payload.clone()
                };
                needed_hashes.insert(hex);
            }
        }

        let known_hash_set: std::collections::HashSet<String> = req
            .known_blob_hashes
            .iter()
            .map(|h| h.value.clone())
            .collect();

        let mut blobs = Vec::new();
        let mut deltas = Vec::new();

        if req.capabilities.supports_delta {
            for needed_hash in &needed_hashes {
                let target_data = match store.get_blob(&req.repo_id, needed_hash) {
                    Ok(Some(d)) => d,
                    _ => {
                        if let Ok(b) = store.get_blobs(
                            &req.repo_id,
                            &std::collections::HashSet::from([needed_hash.clone()]),
                        ) {
                            if let Some(blob) = b.into_iter().next() {
                                blobs.push(blob);
                            }
                        }
                        continue;
                    }
                };

                if known_hash_set.contains(needed_hash) {
                    let base_data = match store.get_blob(&req.repo_id, needed_hash) {
                        Ok(Some(d)) => d,
                        _ => {
                            blobs.push(BlobRef {
                                hash: HashProto { value: needed_hash.clone() },
                                data: base64_encode(&target_data),
                            });
                            continue;
                        }
                    };

                    if base_data == target_data {
                        continue;
                    }

                    let (_base_copy, delta_bytes) =
                        suture_protocol::compute_delta(&base_data, &target_data);

                    if delta_bytes.len() < target_data.len() {
                        deltas.push(BlobDelta {
                            base_hash: HashProto { value: needed_hash.clone() },
                            target_hash: HashProto { value: needed_hash.clone() },
                            encoding: DeltaEncoding::BinaryPatch,
                            delta_data: base64_encode(&delta_bytes),
                        });
                    } else {
                        blobs.push(BlobRef {
                            hash: HashProto { value: needed_hash.clone() },
                            data: base64_encode(&target_data),
                        });
                    }
                } else {
                    blobs.push(BlobRef {
                        hash: HashProto { value: needed_hash.clone() },
                        data: base64_encode(&target_data),
                    });
                }
            }
        } else {
            blobs = store
                .get_blobs(&req.repo_id, &needed_hashes)
                .unwrap_or_default();
        }

        crate::types::PullResponseV2 {
            success: true,
            error: None,
            patches: new_patches,
            branches,
            blobs,
            deltas,
            protocol_version: crate::types::PROTOCOL_VERSION_V2,
        }
    }

    pub async fn handle_push_v2(
        &self,
        req: crate::types::PushRequestV2,
    ) -> Result<PushResponse, (StatusCode, PushResponse)> {
        if let Some(ref sig_bytes) = req.signature {
            let store = self.storage.read().await;
            if store.has_authorized_keys().unwrap_or(false) {
                let v1_req = PushRequest {
                    repo_id: req.repo_id.clone(),
                    patches: req.patches.clone(),
                    branches: req.branches.clone(),
                    blobs: req.blobs.clone(),
                    signature: req.signature.clone(),
                    known_branches: req.known_branches.clone(),
                    force: req.force,
                };
                if let Err(e) = verify_push_signature(&store, &v1_req, sig_bytes) {
                    return Err((
                        StatusCode::FORBIDDEN,
                        PushResponse {
                            success: false,
                            error: Some(format!("authentication failed: {e}")),
                            existing_patches: vec![],
                        },
                    ));
                }
            }
        } else if !self.no_auth {
            let store = self.storage.read().await;
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

        let store = self.storage.write().await;
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

        for delta in &req.deltas {
            if matches!(delta.encoding, DeltaEncoding::BinaryPatch) {
                let base_hex = hash_to_hex(&delta.base_hash);
                let target_hex = hash_to_hex(&delta.target_hash);
                let base_data = match store.get_blob(&req.repo_id, &base_hex) {
                    Ok(Some(d)) => d,
                    _ => continue,
                };
                let delta_bytes = match base64_decode(&delta.delta_data) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let reconstructed = suture_protocol::apply_delta(&base_data, &delta_bytes);
                if let Err(e) = store.store_blob(&req.repo_id, &target_hex, &reconstructed) {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        PushResponse {
                            success: false,
                            error: Some(format!("storage error reconstructing delta blob: {e}")),
                            existing_patches: vec![],
                        },
                    ));
                }
            } else if matches!(delta.encoding, DeltaEncoding::FullBlob) {
                let target_hex = hash_to_hex(&delta.target_hash);
                let data = match base64_decode(&delta.delta_data) {
                    Ok(d) => d,
                    Err(e) => {
                        return Err((
                            StatusCode::BAD_REQUEST,
                            PushResponse {
                                success: false,
                                error: Some(format!("invalid base64 in delta blob: {e}")),
                                existing_patches: vec![],
                            },
                        ));
                    }
                };
                if let Err(e) = store.store_blob(&req.repo_id, &target_hex, &data) {
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
        }

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

            if !req.force
                && let Some(ref known) = req.known_branches
                && let Some(known_branch) = known.iter().find(|kb| kb.name == branch.name)
            {
                let known_target = hash_to_hex(&known_branch.target_id);
                if known_target != target_hex
                    && let Ok(Some(current_target)) =
                        store.get_branch_target(&req.repo_id, &branch.name)
                    && !store
                        .is_ancestor(&req.repo_id, &current_target, &target_hex)
                        .unwrap_or(false)
                {
                    return Err((
                        StatusCode::CONFLICT,
                        PushResponse {
                            success: false,
                            error: Some(format!(
                                "branch '{}' rejected: non-fast-forward push (use --force to override)",
                                branch.name
                            )),
                            existing_patches: vec![],
                        },
                    ));
                }
            }

            if store
                .is_branch_protected(&req.repo_id, &branch.name)
                .unwrap_or(false)
            {
                let push_authors: std::collections::HashSet<&str> =
                    req.patches.iter().map(|p| p.author.as_str()).collect();
                let is_owner =
                    push_authors.len() == 1 && push_authors.contains(branch.name.as_str());
                if !is_owner {
                    return Err((
                        StatusCode::FORBIDDEN,
                        PushResponse {
                            success: false,
                            error: Some(format!(
                                "branch '{}' is protected and can only be updated by its owner",
                                branch.name
                            )),
                            existing_patches: vec![],
                        },
                    ));
                }
            }

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

    let store = hub.storage.read().await;
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
        let store = hub.storage.read().await;
        if store.verify_token(token).unwrap_or(false) {
            return Ok(());
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

async fn resolve_user(hub: &SutureHubServer, headers: &HeaderMap) -> Option<UserInfo> {
    if let Some(auth_header) = headers.get("authorization")
        && let Ok(auth_str) = auth_header.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
    {
        let store = hub.storage.read().await;
        return store.get_user_by_token(token).ok().flatten();
    }
    None
}

async fn require_role(
    hub: &SutureHubServer,
    headers: &HeaderMap,
    required_role: &Role,
) -> Result<UserInfo, StatusCode> {
    if hub.no_auth {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let user = resolve_user(hub, headers).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let user_role = Role::from_str(&user.role);

    if user_role >= *required_role {
        Ok(user)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

fn generate_api_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    hex::encode(bytes)
}

fn generate_random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    hex::encode(bytes)
}

pub async fn push_compressed_handler(
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
    let mut req = req;
    for blob in &mut req.blobs {
        let compressed_data = match base64_decode(&blob.data) {
            Ok(d) => d,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(PushResponse {
                        success: false,
                        error: Some(format!("invalid base64 in compressed blob: {e}")),
                        existing_patches: vec![],
                    }),
                );
            }
        };
        let decompressed = match suture_protocol::decompress(&compressed_data) {
            Ok(d) => d,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(PushResponse {
                        success: false,
                        error: Some(e),
                        existing_patches: vec![],
                    }),
                );
            }
        };
        blob.data = base64_encode(&decompressed);
    }
    match hub.handle_push(req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)),
        Err((status, resp)) => (status, Json(resp)),
    }
}

pub async fn pull_compressed_handler(
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
    let mut resp = hub.handle_pull(req).await;
    if resp.success {
        for blob in &mut resp.blobs {
            let raw = match base64_decode(&blob.data) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let compressed = match suture_protocol::compress(&raw) {
                Ok(c) => c,
                Err(_) => continue,
            };
            blob.data = base64_encode(&compressed);
        }
    }
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (status, Json(resp))
}

pub async fn push_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<PushRequest>,
) -> (StatusCode, HeaderMap, Json<PushResponse>) {
    let ip = addr.ip().to_string();
    if let Err(retry_after) = hub.check_rate_limit(&ip, "push") {
        let mut hdrs = HeaderMap::new();
        hdrs.insert(
            axum::http::header::RETRY_AFTER,
            retry_after.to_string().parse().unwrap(),
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(PushResponse {
                success: false,
                error: Some("rate limit exceeded".to_string()),
                existing_patches: vec![],
            }),
        );
    }

    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            HeaderMap::new(),
            Json(PushResponse {
                success: false,
                error: Some("authentication failed".to_string()),
                existing_patches: vec![],
            }),
        );
    }

    if !hub.no_auth {
        if let Some(user) = resolve_user(&hub, &headers).await {
            let user_role = Role::from_str(&user.role);
            if user_role < Role::Member {
                return (
                    StatusCode::FORBIDDEN,
                    HeaderMap::new(),
                    Json(PushResponse {
                        success: false,
                        error: Some("insufficient permissions: readers cannot push".to_string()),
                        existing_patches: vec![],
                    }),
                );
            }
        }
    }

    match hub.handle_push(req).await {
        Ok(resp) => (StatusCode::OK, HeaderMap::new(), Json(resp)),
        Err((status, resp)) => (status, HeaderMap::new(), Json(resp)),
    }
}

pub async fn pull_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<PullRequest>,
) -> (StatusCode, HeaderMap, Json<PullResponse>) {
    let ip = addr.ip().to_string();
    if let Err(retry_after) = hub.check_rate_limit(&ip, "pull") {
        let mut hdrs = HeaderMap::new();
        hdrs.insert(
            axum::http::header::RETRY_AFTER,
            retry_after.to_string().parse().unwrap(),
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(PullResponse {
                success: false,
                error: Some("rate limit exceeded".to_string()),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
            }),
        );
    }

    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            HeaderMap::new(),
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
    (status, HeaderMap::new(), Json(resp))
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

    let store = hub.storage.write().await;
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
            let store = hub.storage.read().await;
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
    let store = hub.storage.read().await;
    let branches = store.get_branches(&repo_id).unwrap_or_default();
    (StatusCode::OK, Json(branches))
}

pub async fn repo_patches_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path(repo_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> (StatusCode, Json<Vec<PatchProto>>) {
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50);
    let patches = hub.handle_repo_patches(&repo_id, offset, limit).await;
    (StatusCode::OK, Json(patches))
}

pub async fn protect_branch_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path((repo_id, branch)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    let store = hub.storage.write().await;
    match store.protect_branch(&repo_id, &branch) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn unprotect_branch_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path((repo_id, branch)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    let store = hub.storage.write().await;
    match store.unprotect_branch(&repo_id, &branch) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
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

pub async fn register_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(req): Json<crate::types::RegisterRequest>,
) -> (StatusCode, Json<crate::types::RegisterResponse>) {
    match require_role(&hub, &headers, &Role::Admin).await {
        Ok(_) => {}
        Err(status) => {
            return (
                status,
                Json(crate::types::RegisterResponse {
                    success: false,
                    error: Some("admin access required".to_string()),
                    user: None,
                }),
            );
        }
    }

    let role = req.role.as_deref().unwrap_or("member");
    if !matches!(role, "admin" | "member" | "reader") {
        return (
            StatusCode::BAD_REQUEST,
            Json(crate::types::RegisterResponse {
                success: false,
                error: Some("role must be admin, member, or reader".to_string()),
                user: None,
            }),
        );
    }

    let api_token = generate_api_token();

    let store = hub.storage.write().await;
    match store.create_user(&req.username, &req.display_name, role, &api_token) {
        Ok(()) => {
            let user = store.get_user(&req.username).ok().flatten();
            (
                StatusCode::CREATED,
                Json(crate::types::RegisterResponse {
                    success: true,
                    error: None,
                    user,
                }),
            )
        }
        Err(e) => (
            StatusCode::CONFLICT,
            Json(crate::types::RegisterResponse {
                success: false,
                error: Some(format!("failed to create user: {e}")),
                user: None,
            }),
        ),
    }
}

pub async fn list_users_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
) -> (StatusCode, Json<crate::types::ListUsersResponse>) {
    match require_role(&hub, &headers, &Role::Admin).await {
        Ok(_) => {}
        Err(status) => {
            return (
                status,
                Json(crate::types::ListUsersResponse {
                    success: false,
                    error: Some("admin access required".to_string()),
                    users: vec![],
                }),
            );
        }
    }

    let store = hub.storage.read().await;
    match store.list_users() {
        Ok(users) => (
            StatusCode::OK,
            Json(crate::types::ListUsersResponse {
                success: true,
                error: None,
                users,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(crate::types::ListUsersResponse {
                success: false,
                error: Some(format!("database error: {e}")),
                users: vec![],
            }),
        ),
    }
}

pub async fn get_user_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> (StatusCode, Json<crate::types::GetUserResponse>) {
    let requesting_user = resolve_user(&hub, &headers).await;
    let is_admin = requesting_user
        .as_ref()
        .map(|u| u.role == "admin")
        .unwrap_or(false);
    let is_self = requesting_user
        .as_ref()
        .map(|u| u.username == username)
        .unwrap_or(false);

    if !is_admin && !is_self {
        return (
            StatusCode::FORBIDDEN,
            Json(crate::types::GetUserResponse {
                success: false,
                error: Some("access denied".to_string()),
                user: None,
            }),
        );
    }

    let store = hub.storage.read().await;
    match store.get_user(&username) {
        Ok(Some(user)) => {
            let mut resp_user = user;
            if is_self && !is_admin {
                resp_user.api_token = None;
            }
            (
                StatusCode::OK,
                Json(crate::types::GetUserResponse {
                    success: true,
                    error: None,
                    user: Some(resp_user),
                }),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(crate::types::GetUserResponse {
                success: false,
                error: Some("user not found".to_string()),
                user: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(crate::types::GetUserResponse {
                success: false,
                error: Some(format!("database error: {e}")),
                user: None,
            }),
        ),
    }
}

pub async fn update_role_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(username): Path<String>,
    Json(req): Json<crate::types::UpdateRoleRequest>,
) -> (StatusCode, Json<crate::types::UpdateRoleResponse>) {
    match require_role(&hub, &headers, &Role::Admin).await {
        Ok(_) => {}
        Err(status) => {
            return (
                status,
                Json(crate::types::UpdateRoleResponse {
                    success: false,
                    error: Some("admin access required".to_string()),
                }),
            );
        }
    }

    if !matches!(req.role.as_str(), "admin" | "member" | "reader") {
        return (
            StatusCode::BAD_REQUEST,
            Json(crate::types::UpdateRoleResponse {
                success: false,
                error: Some("role must be admin, member, or reader".to_string()),
            }),
        );
    }

    let store = hub.storage.write().await;
    match store.update_user_role(&username, &req.role) {
        Ok(()) => (
            StatusCode::OK,
            Json(crate::types::UpdateRoleResponse {
                success: true,
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(crate::types::UpdateRoleResponse {
                success: false,
                error: Some(format!("database error: {e}")),
            }),
        ),
    }
}

pub async fn delete_user_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> (StatusCode, Json<crate::types::DeleteUserResponse>) {
    match require_role(&hub, &headers, &Role::Admin).await {
        Ok(_) => {}
        Err(status) => {
            return (
                status,
                Json(crate::types::DeleteUserResponse {
                    success: false,
                    error: Some("admin access required".to_string()),
                }),
            );
        }
    }

    let store = hub.storage.write().await;
    match store.delete_user(&username) {
        Ok(()) => (
            StatusCode::OK,
            Json(crate::types::DeleteUserResponse {
                success: true,
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(crate::types::DeleteUserResponse {
                success: false,
                error: Some(format!("database error: {e}")),
            }),
        ),
    }
}

pub async fn handshake_v2_handler(
    Json(req): Json<crate::types::HandshakeRequestV2>,
) -> Json<crate::types::HandshakeResponseV2> {
    let compatible = req.client_version == crate::types::PROTOCOL_VERSION_V2;
    Json(crate::types::HandshakeResponseV2 {
        server_version: crate::types::PROTOCOL_VERSION_V2,
        server_name: "suture-hub".to_string(),
        compatible,
        server_capabilities: crate::types::ServerCapabilities {
            supports_delta: true,
            supports_compression: true,
            max_blob_size: 50 * 1024 * 1024,
            protocol_versions: vec![crate::types::PROTOCOL_VERSION, crate::types::PROTOCOL_VERSION_V2],
        },
    })
}

pub async fn v2_pull_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<crate::types::PullRequestV2>,
) -> (StatusCode, HeaderMap, Json<crate::types::PullResponseV2>) {
    let ip = addr.ip().to_string();
    if let Err(retry_after) = hub.check_rate_limit(&ip, "pull") {
        let mut hdrs = HeaderMap::new();
        hdrs.insert(
            axum::http::header::RETRY_AFTER,
            retry_after.to_string().parse().unwrap(),
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(crate::types::PullResponseV2 {
                success: false,
                error: Some("rate limit exceeded".to_string()),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
                deltas: vec![],
                protocol_version: crate::types::PROTOCOL_VERSION_V2,
            }),
        );
    }

    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            HeaderMap::new(),
            Json(crate::types::PullResponseV2 {
                success: false,
                error: Some("authentication failed".to_string()),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
                deltas: vec![],
                protocol_version: crate::types::PROTOCOL_VERSION_V2,
            }),
        );
    }

    let resp = hub.handle_pull_v2(req).await;
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (status, HeaderMap::new(), Json(resp))
}

pub async fn v2_push_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<crate::types::PushRequestV2>,
) -> (StatusCode, HeaderMap, Json<PushResponse>) {
    let ip = addr.ip().to_string();
    if let Err(retry_after) = hub.check_rate_limit(&ip, "push") {
        let mut hdrs = HeaderMap::new();
        hdrs.insert(
            axum::http::header::RETRY_AFTER,
            retry_after.to_string().parse().unwrap(),
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(PushResponse {
                success: false,
                error: Some("rate limit exceeded".to_string()),
                existing_patches: vec![],
            }),
        );
    }

    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            HeaderMap::new(),
            Json(PushResponse {
                success: false,
                error: Some("authentication failed".to_string()),
                existing_patches: vec![],
            }),
        );
    }

    if !hub.no_auth {
        if let Some(user) = resolve_user(&hub, &headers).await {
            let user_role = Role::from_str(&user.role);
            if user_role < Role::Member {
                return (
                    StatusCode::FORBIDDEN,
                    HeaderMap::new(),
                    Json(PushResponse {
                        success: false,
                        error: Some("insufficient permissions: readers cannot push".to_string()),
                        existing_patches: vec![],
                    }),
                );
            }
        }
    }

    match hub.handle_push_v2(req).await {
        Ok(resp) => (StatusCode::OK, HeaderMap::new(), Json(resp)),
        Err((status, resp)) => (status, HeaderMap::new(), Json(resp)),
    }
}

pub async fn add_peer_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(req): Json<AddPeerRequest>,
) -> (StatusCode, Json<AddPeerResponse>) {
    let role = hub.get_replication_role();
    if role != "leader" {
        return (
            StatusCode::FORBIDDEN,
            Json(AddPeerResponse {
                success: false,
                peer_id: None,
                error: Some("only the leader can manage peers".to_string()),
            }),
        );
    }
    let resp = hub.handle_add_peer(req).await;
    let status = if resp.success { StatusCode::OK } else { StatusCode::BAD_REQUEST };
    (status, Json(resp))
}

pub async fn remove_peer_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path(id): Path<i64>,
) -> (StatusCode, Json<RemovePeerResponse>) {
    let role = hub.get_replication_role();
    if role != "leader" {
        return (
            StatusCode::FORBIDDEN,
            Json(RemovePeerResponse {
                success: false,
                error: Some("only the leader can manage peers".to_string()),
            }),
        );
    }
    let resp = hub.handle_remove_peer(id).await;
    let status = if resp.success { StatusCode::OK } else { StatusCode::BAD_REQUEST };
    (status, Json(resp))
}

pub async fn list_peers_handler(
    State(hub): State<Arc<SutureHubServer>>,
) -> (StatusCode, Json<ListPeersResponse>) {
    let resp = hub.handle_list_peers().await;
    (StatusCode::OK, Json(resp))
}

pub async fn replication_status_handler(
    State(hub): State<Arc<SutureHubServer>>,
) -> (StatusCode, Json<ReplicationStatusResponse>) {
    let resp = hub.handle_replication_status().await;
    (StatusCode::OK, Json(resp))
}

pub async fn replication_sync_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(entries): Json<Vec<ReplicationEntry>>,
) -> (StatusCode, Json<SyncResponse>) {
    let role = hub.get_replication_role();
    if role != "follower" && role != "standalone" {
        return (
            StatusCode::FORBIDDEN,
            Json(SyncResponse {
                success: false,
                applied: 0,
                error: Some("sync endpoint is for followers only".to_string()),
            }),
        );
    }
    let resp = hub.handle_replication_sync(entries).await;
    let status = if resp.success { StatusCode::OK } else { StatusCode::INTERNAL_SERVER_ERROR };
    (status, Json(resp))
}

pub async fn run_server(
    hub: SutureHubServer,
    addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = axum::Router::new()
        .route("/", axum::routing::get(serve_index))
        .route("/push", axum::routing::post(push_handler))
        .route("/push/compressed", axum::routing::post(push_compressed_handler))
        .route("/pull", axum::routing::post(pull_handler))
        .route("/pull/compressed", axum::routing::post(pull_compressed_handler))
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
        .route("/v2/handshake", axum::routing::get(handshake_v2_handler))
        .route("/v2/handshake", axum::routing::post(handshake_v2_handler))
        .route("/v2/pull", axum::routing::post(v2_pull_handler))
        .route("/v2/push", axum::routing::post(v2_push_handler))
        .route("/auth/token", axum::routing::post(create_token_handler))
        .route("/auth/verify", axum::routing::post(verify_token_handler))
        .route("/mirror/setup", axum::routing::post(mirror_setup_handler))
        .route("/mirror/sync", axum::routing::post(mirror_sync_handler))
        .route("/mirror/status", axum::routing::get(mirror_status_handler))
        .route("/mirror/status", axum::routing::post(mirror_status_handler))
        .route(
            "/repos/{repo_id}/protect/{branch}",
            axum::routing::post(protect_branch_handler),
        )
        .route(
            "/repos/{repo_id}/unprotect/{branch}",
            axum::routing::post(unprotect_branch_handler),
        )
        .route("/auth/register", axum::routing::post(register_handler))
        .route("/users", axum::routing::get(list_users_handler))
        .route("/users/{username}", axum::routing::get(get_user_handler))
        .route(
            "/users/{username}/role",
            axum::routing::patch(update_role_handler),
        )
        .route(
            "/users/{username}",
            axum::routing::delete(delete_user_handler),
        )
        .route("/static/{*path}", axum::routing::get(serve_static_file))
        .route("/replication/peers", axum::routing::post(add_peer_handler))
        .route("/replication/peers", axum::routing::get(list_peers_handler))
        .route("/replication/peers/{id}", axum::routing::delete(remove_peer_handler))
        .route("/replication/status", axum::routing::get(replication_status_handler))
        .route("/replication/sync", axum::routing::post(replication_sync_handler))
        .with_state(Arc::new(hub));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Suture Hub listening on {addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
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
            force: false,
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
            force: false,
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
            force: false,
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
            force: false,
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
            force: false,
        };
        hub.handle_push(push).await.unwrap();

        let push = PushRequest {
            repo_id: "repo-2".to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
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
            force: false,
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
        let blob_hash = "deadbeef".repeat(8);
        let a_hex = "a".repeat(64);

        let push = PushRequest {
            repo_id: "blob-repo".to_string(),
            patches: vec![PatchProto {
                id: make_hash_proto(&a_hex),
                operation_type: "Create".to_string(),
                touch_set: vec!["file_a".to_string()],
                target_path: Some("file_a".to_string()),
                payload: blob_hash.clone(),
                parent_ids: vec![],
                author: "alice".to_string(),
                message: "patch a".to_string(),
                timestamp: 0,
            }],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![BlobRef {
                hash: make_hash_proto(&blob_hash),
                data: base64_encode(blob_data),
            }],
            signature: None,
            known_branches: None,
            force: false,
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
            force: false,
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
            force: false,
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
            force: false,
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
    async fn test_push_pull_compressed_roundtrip() {
        let hub = Arc::new(SutureHubServer::new());
        let a_hex = "a".repeat(64);
        let blob_data = b"hello compressed world";
        let blob_hash = "deadbeef".repeat(8);

        let compressed_blob = suture_protocol::compress(blob_data).unwrap();
        let compressed_b64 = base64_encode(&compressed_blob);

        let push = PushRequest {
            repo_id: "compressed-repo".to_string(),
            patches: vec![PatchProto {
                id: make_hash_proto(&a_hex),
                operation_type: "Create".to_string(),
                touch_set: vec!["file_a".to_string()],
                target_path: Some("file_a".to_string()),
                payload: blob_hash.clone(),
                parent_ids: vec![],
                author: "alice".to_string(),
                message: "patch a".to_string(),
                timestamp: 0,
            }],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![BlobRef {
                hash: make_hash_proto(&blob_hash),
                data: compressed_b64,
            }],
            signature: None,
            known_branches: None,
            force: false,
        };
        let (status, push_resp) = push_compressed_handler(State(hub.clone()), HeaderMap::new(), Json(push)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(push_resp.success);

        let pull = PullRequest {
            repo_id: "compressed-repo".to_string(),
            known_branches: vec![],
            max_depth: None,
        };
        let (status, pull_resp) = pull_compressed_handler(State(hub.clone()), HeaderMap::new(), Json(pull)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(pull_resp.success);
        assert_eq!(pull_resp.blobs.len(), 1);

        let pulled_compressed = base64_decode(&pull_resp.blobs[0].data).unwrap();
        let decompressed = suture_protocol::decompress(&pulled_compressed).unwrap();
        assert_eq!(decompressed, blob_data);
    }

    #[tokio::test]
    async fn test_no_auth_mode() {
        let mut hub = SutureHubServer::new();
        hub.set_no_auth(true);
        assert!(hub.is_no_auth());
    }

    #[tokio::test]
    async fn test_rate_limit_push() {
        let mut hub = SutureHubServer::new();
        hub.set_rate_limit_config(3, 1000, std::time::Duration::from_secs(60));
        let ip = "192.168.1.1";
        assert!(hub.check_rate_limit(ip, "push").is_ok());
        assert!(hub.check_rate_limit(ip, "push").is_ok());
        assert!(hub.check_rate_limit(ip, "push").is_ok());
        let err = hub.check_rate_limit(ip, "push").unwrap_err();
        assert!(err <= 60);
    }

    #[tokio::test]
    async fn test_rate_limit_allows_after_window() {
        let mut hub = SutureHubServer::new();
        hub.set_rate_limit_config(2, 1000, std::time::Duration::from_millis(100));
        let ip = "10.0.0.1";
        assert!(hub.check_rate_limit(ip, "push").is_ok());
        assert!(hub.check_rate_limit(ip, "push").is_ok());
        assert!(hub.check_rate_limit(ip, "push").is_err());
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert!(hub.check_rate_limit(ip, "push").is_ok());
    }

    #[tokio::test]
    async fn test_pagination_default() {
        let hub = SutureHubServer::new();
        let a_hex = "a".repeat(64);
        let b_hex = "b".repeat(64);
        let push = PushRequest {
            repo_id: "pag-test".to_string(),
            patches: vec![
                make_patch(&a_hex, "Create", &[], "alice"),
                make_patch(&b_hex, "Modify", &[&a_hex], "bob"),
            ],
            branches: vec![make_branch("main", &b_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        };
        hub.handle_push(push).await.unwrap();
        let patches = hub.handle_repo_patches("pag-test", 0, 50).await;
        assert_eq!(patches.len(), 2);
    }

    #[tokio::test]
    async fn test_pagination_custom() {
        let hub = SutureHubServer::new();
        for i in 0..5u32 {
            let hex = format!("{:064x}", i);
            let parents: Vec<String> = if i > 0 {
                vec![format!("{:064x}", i - 1)]
            } else {
                vec![]
            };
            let push = PushRequest {
                repo_id: "pag-custom".to_string(),
                patches: vec![PatchProto {
                    id: make_hash_proto(&hex),
                    operation_type: "Create".to_string(),
                    touch_set: vec![format!("file_{i}")],
                    target_path: Some(format!("file_{i}")),
                    payload: String::new(),
                    parent_ids: parents.iter().map(|p| make_hash_proto(p)).collect(),
                    author: "alice".to_string(),
                    message: format!("patch {i}"),
                    timestamp: 0,
                }],
                branches: vec![],
                blobs: vec![],
                signature: None,
                known_branches: None,
                force: false,
            };
            hub.handle_push(push).await.unwrap();
        }
        let page = hub.handle_repo_patches("pag-custom", 1, 2).await;
        assert_eq!(page.len(), 2);
    }

    #[tokio::test]
    async fn test_pagination_max_limit() {
        let hub = SutureHubServer::new();
        let mut patches = Vec::new();
        for i in 0..250u32 {
            let hex = format!("{:064x}", i);
            let parents: Vec<String> = if i > 0 {
                vec![format!("{:064x}", i - 1)]
            } else {
                vec![]
            };
            patches.push(PatchProto {
                id: make_hash_proto(&hex),
                operation_type: "Create".to_string(),
                touch_set: vec![format!("file_{i}")],
                target_path: Some(format!("file_{i}")),
                payload: String::new(),
                parent_ids: parents.iter().map(|p| make_hash_proto(p)).collect(),
                author: "alice".to_string(),
                message: format!("patch {i}"),
                timestamp: 0,
            });
        }
        let push = PushRequest {
            repo_id: "pag-max".to_string(),
            patches,
            branches: vec![],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        };
        hub.handle_push(push).await.unwrap();
        let result = hub.handle_repo_patches("pag-max", 0, 500).await;
        assert_eq!(result.len(), 200);
    }

    async fn create_test_user(
        hub: &SutureHubServer,
        username: &str,
        display_name: &str,
        role: &str,
    ) -> String {
        let api_token = generate_api_token();
        let store = hub.storage.write().await;
        store
            .create_user(username, display_name, role, &api_token)
            .unwrap();
        api_token
    }

    fn make_auth_header(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {token}").parse().unwrap(),
        );
        headers
    }

    #[tokio::test]
    async fn test_create_user() {
        let hub = SutureHubServer::new();
        create_test_user(&hub, "alice", "Alice Admin", "admin").await;

        let store = hub.storage.read().await;
        let user = store.get_user("alice").unwrap().unwrap();
        assert_eq!(user.username, "alice");
        assert_eq!(user.display_name, "Alice Admin");
        assert_eq!(user.role, "admin");
        assert!(user.api_token.is_some());
        assert!(user.created_at > 0);
    }

    #[tokio::test]
    async fn test_register_requires_admin() {
        let hub = Arc::new(SutureHubServer::new());
        let member_token = create_test_user(&hub, "member1", "Member One", "member").await;
        let reader_token = create_test_user(&hub, "reader1", "Reader One", "reader").await;
        let admin_token = create_test_user(&hub, "admin1", "Admin One", "admin").await;

        let req = crate::types::RegisterRequest {
            username: "newuser".to_string(),
            display_name: "New User".to_string(),
            role: Some("member".to_string()),
        };

        let (_, resp) = register_handler(
            State(hub.clone()),
            make_auth_header(&member_token),
            Json(req.clone()),
        )
        .await;
        assert!(!resp.success);

        let (_, resp) = register_handler(
            State(hub.clone()),
            make_auth_header(&reader_token),
            Json(req.clone()),
        )
        .await;
        assert!(!resp.success);

        let (status, resp) = register_handler(
            State(hub.clone()),
            make_auth_header(&admin_token),
            Json(req.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(resp.success);
        assert!(resp.user.is_some());
        assert_eq!(resp.user.as_ref().unwrap().username, "newuser");
    }

    #[tokio::test]
    async fn test_role_based_access() {
        let hub = Arc::new(SutureHubServer::new());
        let admin_token = create_test_user(&hub, "admin", "Admin", "admin").await;
        let member_token = create_test_user(&hub, "member", "Member", "member").await;
        let reader_token = create_test_user(&hub, "reader", "Reader", "reader").await;

        let a_hex = "a".repeat(64);

        let push_req_admin = PushRequest {
            repo_id: "rbac-repo".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        };

        let (status, _, _) = push_handler(
            State(hub.clone()),
            make_auth_header(&admin_token),
            ConnectInfo("127.0.0.1:1234".parse().unwrap()),
            Json(push_req_admin),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let push_req_member = PushRequest {
            repo_id: "rbac-repo-2".to_string(),
            patches: vec![make_patch(&"c".repeat(64), "Create", &[], "alice")],
            branches: vec![make_branch("main", &"c".repeat(64))],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        };

        let (status, _, _) = push_handler(
            State(hub.clone()),
            make_auth_header(&member_token),
            ConnectInfo("127.0.0.1:1234".parse().unwrap()),
            Json(push_req_member),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let push_req_reader = PushRequest {
            repo_id: "rbac-repo-reader".to_string(),
            patches: vec![make_patch(&"b".repeat(64), "Create", &[], "alice")],
            branches: vec![make_branch("main", &"b".repeat(64))],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        };
        let (status, _, resp) = push_handler(
            State(hub.clone()),
            make_auth_header(&reader_token),
            ConnectInfo("127.0.0.1:1234".parse().unwrap()),
            Json(push_req_reader),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!resp.success);

        let pull_req = PullRequest {
            repo_id: "rbac-repo".to_string(),
            known_branches: vec![],
            max_depth: None,
        };
        let (status, _, resp) = pull_handler(
            State(hub.clone()),
            make_auth_header(&reader_token),
            ConnectInfo("127.0.0.1:1234".parse().unwrap()),
            Json(pull_req),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(resp.success);
    }

    #[tokio::test]
    async fn test_list_users() {
        let hub = Arc::new(SutureHubServer::new());
        let admin_token = create_test_user(&hub, "admin", "Admin", "admin").await;
        let member_token = create_test_user(&hub, "member", "Member", "member").await;

        let (_status, resp) = list_users_handler(
            State(hub.clone()),
            make_auth_header(&admin_token),
        )
        .await;
        assert!(resp.success);
        assert!(resp.users.len() >= 2);

        let (_status, resp) = list_users_handler(
            State(hub.clone()),
            make_auth_header(&member_token),
        )
        .await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_update_role() {
        let hub = Arc::new(SutureHubServer::new());
        let admin_token = create_test_user(&hub, "admin", "Admin", "admin").await;
        create_test_user(&hub, "target", "Target User", "reader").await;

        let (_status, resp) = update_role_handler(
            State(hub.clone()),
            make_auth_header(&admin_token),
            Path("target".to_string()),
            Json(crate::types::UpdateRoleRequest {
                role: "member".to_string(),
            }),
        )
        .await;
        assert!(resp.success);

        {
            let store = hub.storage.read().await;
            let user = store.get_user("target").unwrap().unwrap();
            assert_eq!(user.role, "member");
        }

        let member_token = create_test_user(&hub, "member", "Member", "member").await;
        let (_status, resp) = update_role_handler(
            State(hub.clone()),
            make_auth_header(&member_token),
            Path("target".to_string()),
            Json(crate::types::UpdateRoleRequest {
                role: "admin".to_string(),
            }),
        )
        .await;
        assert!(!resp.success);

        let store = hub.storage.read().await;
        let user = store.get_user("target").unwrap().unwrap();
        assert_eq!(user.role, "member");
    }

    #[tokio::test]
    async fn test_v2_pull_with_deltas() {
        let hub = SutureHubServer::new();
        let a_hex = "a".repeat(64);
        let blob_hash = "deadbeef".repeat(8);

        let blob_v1 = b"Hello, World! This is version 1 of the file.";
        let push = PushRequest {
            repo_id: "delta-repo".to_string(),
            patches: vec![PatchProto {
                id: make_hash_proto(&a_hex),
                operation_type: "Create".to_string(),
                touch_set: vec!["file_a".to_string()],
                target_path: Some("file_a".to_string()),
                payload: blob_hash.clone(),
                parent_ids: vec![],
                author: "alice".to_string(),
                message: "patch a".to_string(),
                timestamp: 0,
            }],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![BlobRef {
                hash: make_hash_proto(&blob_hash),
                data: base64_encode(blob_v1),
            }],
            signature: None,
            known_branches: None,
            force: false,
        };
        hub.handle_push(push).await.unwrap();

        let pull_req = crate::types::PullRequestV2 {
            repo_id: "delta-repo".to_string(),
            known_branches: vec![make_branch("main", &a_hex)],
            max_depth: Some(0),
            known_blob_hashes: vec![make_hash_proto(&blob_hash)],
            capabilities: crate::types::ClientCapabilities {
                supports_delta: true,
                supports_compression: true,
                max_blob_size: 1024 * 1024,
            },
        };
        let resp = hub.handle_pull_v2(pull_req).await;
        assert!(resp.success);
        assert_eq!(resp.protocol_version, crate::types::PROTOCOL_VERSION_V2);
        assert!(resp.deltas.is_empty());
        assert!(resp.blobs.is_empty());
        assert_eq!(resp.patches.len(), 0);

        let b_hex = "b".repeat(64);
        let blob_v2 = b"Hello, Rust! This is version 2 of the file.";
        let push2 = PushRequest {
            repo_id: "delta-repo".to_string(),
            patches: vec![PatchProto {
                id: make_hash_proto(&b_hex),
                operation_type: "Modify".to_string(),
                touch_set: vec!["file_a".to_string()],
                target_path: Some("file_a".to_string()),
                payload: blob_hash.clone(),
                parent_ids: vec![make_hash_proto(&a_hex)],
                author: "alice".to_string(),
                message: "patch b".to_string(),
                timestamp: 1,
            }],
            branches: vec![make_branch("main", &b_hex)],
            blobs: vec![BlobRef {
                hash: make_hash_proto(&blob_hash),
                data: base64_encode(blob_v2),
            }],
            signature: None,
            known_branches: None,
            force: false,
        };
        hub.handle_push(push2).await.unwrap();

        let pull_req2 = crate::types::PullRequestV2 {
            repo_id: "delta-repo".to_string(),
            known_branches: vec![make_branch("main", &a_hex)],
            max_depth: None,
            known_blob_hashes: vec![make_hash_proto(&blob_hash)],
            capabilities: crate::types::ClientCapabilities {
                supports_delta: true,
                supports_compression: true,
                max_blob_size: 1024 * 1024,
            },
        };
        let resp2 = hub.handle_pull_v2(pull_req2).await;
        assert!(resp2.success);
        assert_eq!(resp2.patches.len(), 1);
        assert_eq!(hash_to_hex(&resp2.patches[0].id), b_hex);
    }

    #[tokio::test]
    async fn test_v2_handshake() {
        let req = crate::types::HandshakeRequestV2 {
            client_version: 2,
            client_name: "test-client".to_string(),
            capabilities: crate::types::ClientCapabilities {
                supports_delta: true,
                supports_compression: true,
                max_blob_size: 1024 * 1024,
            },
        };
        let resp = handshake_v2_handler(Json(req)).await;
        assert!(resp.compatible);
        assert_eq!(resp.server_version, 2);
        assert!(resp.server_capabilities.supports_delta);
        assert!(resp.server_capabilities.supports_compression);
    }

    #[tokio::test]
    async fn test_v2_push_with_deltas() {
        let hub = SutureHubServer::new();
        let a_hex = "a".repeat(64);
        let blob_hash = "deadbeef".repeat(8);

        let push = PushRequest {
            repo_id: "delta-push-repo".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![BlobRef {
                hash: make_hash_proto(&blob_hash),
                data: base64_encode(b"base content"),
            }],
            signature: None,
            known_branches: None,
            force: false,
        };
        hub.handle_push(push).await.unwrap();

        let new_hash = "cafebabe".repeat(8);
        let new_data = b"base content with more text appended";
        let (_base_copy, delta_bytes) = suture_protocol::compute_delta(b"base content", new_data);

        let v2_push = crate::types::PushRequestV2 {
            repo_id: "delta-push-repo".to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![],
            deltas: vec![crate::types::BlobDelta {
                base_hash: make_hash_proto(&blob_hash),
                target_hash: make_hash_proto(&new_hash),
                encoding: crate::types::DeltaEncoding::BinaryPatch,
                delta_data: base64_encode(&delta_bytes),
            }],
            signature: None,
            known_branches: None,
            force: false,
        };
        let resp = hub.handle_push_v2(v2_push).await;
        assert!(resp.is_ok());
        assert!(resp.unwrap().success);

        let store = hub.storage.read().await;
        let reconstructed = store.get_blob("delta-push-repo", &new_hash).unwrap();
        assert_eq!(reconstructed, Some(new_data.to_vec()));
    }

    #[tokio::test]
    async fn test_add_replication_peer() {
        let hub = SutureHubServer::new();
        hub.set_replication_role("leader");

        let req = AddPeerRequest {
            peer_url: "http://follower1:8080".to_string(),
            role: "follower".to_string(),
        };
        let resp = hub.handle_add_peer(req).await;
        assert!(resp.success);
        assert!(resp.peer_id.is_some());

        let dup_req = AddPeerRequest {
            peer_url: "http://follower1:8080".to_string(),
            role: "follower".to_string(),
        };
        let dup_resp = hub.handle_add_peer(dup_req).await;
        assert!(!dup_resp.success);
    }

    #[tokio::test]
    async fn test_remove_replication_peer() {
        let hub = SutureHubServer::new();
        hub.set_replication_role("leader");

        let req = AddPeerRequest {
            peer_url: "http://follower1:8080".to_string(),
            role: "follower".to_string(),
        };
        let add_resp = hub.handle_add_peer(req).await;
        let peer_id = add_resp.peer_id.unwrap();

        let remove_resp = hub.handle_remove_peer(peer_id).await;
        assert!(remove_resp.success);

        let peers = hub.handle_list_peers().await;
        assert!(peers.peers.is_empty());

        let bad_remove = hub.handle_remove_peer(9999).await;
        assert!(bad_remove.success);
    }

    #[tokio::test]
    async fn test_list_replication_peers() {
        let hub = SutureHubServer::new();
        hub.set_replication_role("leader");

        hub.handle_add_peer(AddPeerRequest {
            peer_url: "http://follower1:8080".to_string(),
            role: "follower".to_string(),
        })
        .await;
        hub.handle_add_peer(AddPeerRequest {
            peer_url: "http://follower2:8080".to_string(),
            role: "follower".to_string(),
        })
        .await;

        let resp = hub.handle_list_peers().await;
        assert_eq!(resp.peers.len(), 2);
    }

    #[tokio::test]
    async fn test_replication_log_and_sync() {
        let hub = SutureHubServer::new();

        let seq1 = hub.log_write("insert", "repos", "repo-1", None).await.unwrap();
        let seq2 = hub
            .log_write("insert", "patches", "patch-1", Some("{\"data\":true}"))
            .await
            .unwrap();

        assert!(seq2 > seq1);

        let entries = {
            let store = hub.storage.read().await;
            store.get_replication_log(0).unwrap()
        };
        assert_eq!(entries.len(), 2);

        let follower = SutureHubServer::new();
        follower.set_replication_role("follower");

        let sync_resp = follower.handle_replication_sync(entries).await;
        assert!(sync_resp.success);
        assert_eq!(sync_resp.applied, 2);

        let follower_entries = {
            let store = follower.storage.read().await;
            store.get_replication_log(0).unwrap()
        };
        assert_eq!(follower_entries.len(), 2);
    }

    #[tokio::test]
    async fn test_replication_status() {
        let hub = SutureHubServer::new();
        hub.set_replication_role("leader");

        hub.log_write("insert", "repos", "repo-1", None)
            .await
            .unwrap();
        hub.log_write("insert", "repos", "repo-2", None)
            .await
            .unwrap();

        hub.handle_add_peer(AddPeerRequest {
            peer_url: "http://follower1:8080".to_string(),
            role: "follower".to_string(),
        })
        .await;

        let status = hub.handle_replication_status().await;
        assert_eq!(status.status.current_seq, 2);
        assert_eq!(status.status.peer_count, 1);
        assert_eq!(status.status.peers.len(), 1);
    }
}
