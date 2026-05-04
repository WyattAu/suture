use axum::{
    Json,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::blob_backend::BlobBackend;
use crate::middleware::request_id_layer;
use crate::storage::HubStorage;
use crate::storage::{ReplicationEntry, ReplicationStatus};
pub use crate::types::*;
use crate::webhooks::{Webhook, WebhookManager};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Admin,
    Member,
    Reader,
}

impl Role {
    #[must_use] 
    pub fn parse(s: &str) -> Self {
        match s {
            "admin" => Self::Admin,
            "member" => Self::Member,
            _ => Self::Reader,
        }
    }

    #[must_use] 
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
            Self::Reader => "reader",
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
    pub cursor: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct AuditQueryParams {
    pub actor: Option<String>,
    pub action: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CursorData {
    offset: u64,
}

fn decode_cursor(cursor: &str) -> Option<u64> {
    let bytes = base64_decode(cursor).ok()?;
    let data: CursorData = serde_json::from_slice(&bytes).ok()?;
    Some(data.offset)
}

fn encode_cursor(offset: u64) -> String {
    let data = CursorData { offset };
    let json = serde_json::to_vec(&data).unwrap_or_default();
    base64_encode(&json)
}

pub struct SutureHubServer {
    pub(crate) storage: Arc<RwLock<HubStorage>>,
    blob_backend: Option<Arc<dyn BlobBackend>>,
    no_auth: bool,
    rate_limits:
        Arc<std::sync::RwLock<std::collections::HashMap<String, (u32, std::time::Instant)>>>,
    max_pushes_per_hour: u32,
    max_pulls_per_hour: u32,
    max_token_creates_per_minute: u32,
    rate_limit_window: std::time::Duration,
    replication_role: Arc<std::sync::RwLock<String>>,
    webhook_manager: Arc<WebhookManager>,
    #[allow(dead_code)]
    rate_limit_db: Option<Arc<tokio::sync::Mutex<rusqlite::Connection>>>,
    lfs_data_dir: Option<std::path::PathBuf>,
    #[cfg(feature = "raft-cluster")]
    raft_node: Arc<tokio::sync::Mutex<suture_raft::RaftNode>>,
    #[cfg(feature = "raft-cluster")]
    raft_node_id: u64,
}

impl Default for SutureHubServer {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl SutureHubServer {
    #[must_use] 
    pub fn new() -> Self {
        Self::new_in_memory()
    }

    #[must_use] 
    pub fn new_in_memory() -> Self {
        Self {
            storage: Arc::new(RwLock::new(
                HubStorage::open_in_memory().expect("in-memory storage must open"),
            )),
            blob_backend: None,
            no_auth: false,
            rate_limits: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            max_pushes_per_hour: 100,
            max_pulls_per_hour: 1000,
            max_token_creates_per_minute: 5,
            rate_limit_window: std::time::Duration::from_secs(60),
            replication_role: Arc::new(std::sync::RwLock::new("standalone".to_owned())),
            webhook_manager: Arc::new(WebhookManager::new()),
            rate_limit_db: None,
            lfs_data_dir: None,
            #[cfg(feature = "raft-cluster")]
            raft_node: Arc::new(tokio::sync::Mutex::new(suture_raft::RaftNode::new(1, vec![]))),
            #[cfg(feature = "raft-cluster")]
            raft_node_id: 1,
        }
    }

    pub fn with_db(path: &std::path::Path) -> Result<Self, crate::storage::StorageError> {
        let rate_limit_db_path = path.with_extension("rate.db");
        let rate_limit_conn = rusqlite::Connection::open(&rate_limit_db_path)?;
        rate_limit_conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        rate_limit_conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS rate_limits (
                key TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 0,
                window_start INTEGER NOT NULL
            );",
        )?;
        Ok(Self {
            storage: Arc::new(RwLock::new(HubStorage::open(path)?)),
            blob_backend: None,
            no_auth: false,
            rate_limits: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            max_pushes_per_hour: 100,
            max_pulls_per_hour: 1000,
            max_token_creates_per_minute: 5,
            rate_limit_window: std::time::Duration::from_secs(60),
            replication_role: Arc::new(std::sync::RwLock::new("standalone".to_owned())),
            webhook_manager: Arc::new(WebhookManager::new()),
            rate_limit_db: Some(Arc::new(tokio::sync::Mutex::new(rate_limit_conn))),
            lfs_data_dir: None,
            #[cfg(feature = "raft-cluster")]
            raft_node: Arc::new(tokio::sync::Mutex::new(suture_raft::RaftNode::new(1, vec![]))),
            #[cfg(feature = "raft-cluster")]
            raft_node_id: 1,
        })
    }

    pub fn set_no_auth(&mut self, no_auth: bool) {
        self.no_auth = no_auth;
    }

    #[must_use] 
    pub fn is_no_auth(&self) -> bool {
        self.no_auth
    }

    #[must_use] 
    pub fn storage(&self) -> &Arc<RwLock<HubStorage>> {
        &self.storage
    }

    pub fn shutdown(&self) {
        tracing::info!("Hub server shutting down");
    }

    /// Check if this node is the Raft leader (or standalone).
    ///
    /// Returns `true` if:
    /// - Raft is not enabled (standalone mode), or
    /// - Raft is enabled and this node is the leader
    #[cfg(feature = "raft-cluster")]
    pub async fn is_leader(&self) -> bool {
        let raft = self.raft_node.lock().await;
        matches!(raft.state(), suture_raft::NodeState::Leader)
    }

    /// Check if this node is the leader (no-op when Raft is disabled).
    #[cfg(not(feature = "raft-cluster"))]
    pub async fn is_leader(&self) -> bool {
        true // standalone mode — always leader
    }

    /// Get the current Raft leader ID, if known.
    #[cfg(feature = "raft-cluster")]
    pub async fn raft_leader(&self) -> Option<u64> {
        self.raft_node.lock().await.leader()
    }

    /// Get this node's Raft state.
    #[cfg(feature = "raft-cluster")]
    pub async fn raft_state(&self) -> String {
        let raft = self.raft_node.lock().await;
        match raft.state() {
            suture_raft::NodeState::Leader => "leader".to_owned(),
            suture_raft::NodeState::Follower => "follower".to_owned(),
            suture_raft::NodeState::Candidate => "candidate".to_owned(),
            suture_raft::NodeState::PreCandidate => "pre-candidate".to_owned(),
        }
    }

    /// Propose a Raft command (must be called on leader).
    #[cfg(feature = "raft-cluster")]
    pub async fn raft_propose(&self, command: Vec<u8>) -> Result<(), suture_raft::RaftError> {
        let mut raft = self.raft_node.lock().await;
        raft.propose(command)
    }

    /// Get committed but unapplied Raft entries.
    #[cfg(feature = "raft-cluster")]
    pub async fn raft_committed_entries(&self) -> Vec<suture_raft::LogEntry> {
        let raft = self.raft_node.lock().await;
        raft.committed_entries().to_vec()
    }

    /// Advance the Raft applied index.
    #[cfg(feature = "raft-cluster")]
    pub async fn raft_advance_applied(&self, count: usize) {
        let mut raft = self.raft_node.lock().await;
        raft.advance_applied(count);
    }

    pub fn set_rate_limit_config(&mut self, pushes: u32, pulls: u32, window: std::time::Duration) {
        self.max_pushes_per_hour = pushes;
        self.max_pulls_per_hour = pulls;
        self.rate_limit_window = window;
    }

    #[must_use]
    pub fn with_lfs_dir(mut self, path: std::path::PathBuf) -> Self {
        if let Err(e) = std::fs::create_dir_all(&path) {
            tracing::warn!("Failed to create directory {}: {}", path.display(), e);
        }
        self.lfs_data_dir = Some(path);
        self
    }

    pub fn set_replication_role(&self, role: &str) {
        *self.replication_role.write().unwrap_or_else(std::sync::PoisonError::into_inner) = role.to_owned();
    }

    #[must_use] 
    pub fn get_replication_role(&self) -> String {
        self.replication_role.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    pub fn set_blob_backend(&mut self, backend: Arc<dyn BlobBackend>) {
        self.blob_backend = Some(backend);
    }

    fn blob_store(
        &self,
        store: &HubStorage,
        repo_id: &str,
        hash_hex: &str,
        data: &[u8],
    ) -> Result<(), String> {
        self.blob_backend.as_ref().map_or_else(
            || store
                .store_blob(repo_id, hash_hex, data)
                .map_err(|e| e.to_string()),
            |backend| backend.store_blob(repo_id, hash_hex, data),
        )
    }

    fn blob_get(
        &self,
        store: &HubStorage,
        repo_id: &str,
        hash_hex: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        self.blob_backend.as_ref().map_or_else(
            || store.get_blob(repo_id, hash_hex).map_err(|e| e.to_string()),
            |backend| backend.get_blob(repo_id, hash_hex),
        )
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
            peers: store.list_replication_peers().unwrap_or_else(|e| {
                tracing::warn!("store list_replication_peers failed: {e}");
                Default::default()
            }),
        }
    }

    pub async fn handle_replication_status(&self) -> ReplicationStatusResponse {
        let store = self.storage.read().await;
        let status = match store.get_replication_status() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to get replication status: {e}");
                return ReplicationStatusResponse {
                    status: ReplicationStatus {
                        current_seq: 0,
                        peer_count: 0,
                        peers: vec![],
                    },
                };
            }
        };
        ReplicationStatusResponse { status }
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

        let full_key = format!("{key}:{ip}");
        let now = std::time::Instant::now();
        let mut limits = self.rate_limits.write().unwrap_or_else(std::sync::PoisonError::into_inner);

        limits.retain(|_, (_, start)| now.duration_since(*start) < window);

        let limit = match key {
            "push" => self.max_pushes_per_hour,
            "pull" => self.max_pulls_per_hour,
            "token_create" => self.max_token_creates_per_minute,
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

    pub async fn handle_repo_patches_cursor(
        &self,
        repo_id: &str,
        offset: u64,
        limit: u32,
    ) -> (Vec<PatchProto>, Option<String>) {
        let store = self.storage.read().await;
        let effective_limit = limit.min(200) as usize;
        let offset = offset as usize;
        let patches = store.get_all_patches(repo_id, offset, effective_limit + 1).unwrap_or_else(|e| {
            tracing::warn!("store get_all_patches failed: {e}");
            Default::default()
        });
        let has_more = patches.len() > effective_limit;
        let mut collected = patches;
        if has_more {
            collected.truncate(effective_limit);
        }
        let next_cursor = if has_more {
            Some(encode_cursor(offset as u64 + limit as u64))
        } else {
            None
        };
        (collected, next_cursor)
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
            let has_keys = match store.has_authorized_keys() {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Failed to check authorized keys: {e}");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        PushResponse {
                            success: false,
                            error: Some("database error".to_owned()),
                            existing_patches: vec![],
                        },
                    ));
                }
            };
            if has_keys
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
            let has_keys = store.has_authorized_keys().unwrap_or_else(|e| {
                tracing::error!("Failed to check authorized keys: {e}");
                true
            });
            let has_tokens = store.has_tokens().unwrap_or_else(|e| {
                tracing::error!("Failed to check tokens: {e}");
                true
            });
            if has_keys || has_tokens {
                return Err((
                    StatusCode::FORBIDDEN,
                    PushResponse {
                        success: false,
                        error: Some("authentication required: no signature provided".to_owned()),
                        existing_patches: vec![],
                    },
                ));
            }
        }

        let mut existing_patches = Vec::new();

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
            if let Err(e) = self.blob_store(&store, &req.repo_id, &hex, &data) {
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

        for patch in &req.patches {
            if let Err(e) = store.log_operation("insert", "patches", &hash_to_hex(&patch.id), None) {
                tracing::warn!("Failed to log operation: {}", e);
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
                        .unwrap_or_else(|e| {
                            tracing::warn!("Failed to check ancestry: {e}");
                            true
                        })
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
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to check branch protection: {e}");
                    false
                })
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

        for branch in &req.branches {
            let target_hex = hash_to_hex(&branch.target_id);
            if let Err(e) = store.log_operation(
                "set",
                "branches",
                &format!("{}:{}", req.repo_id, branch.name),
                Some(&target_hex),
            ) {
                tracing::warn!("Failed to log operation: {}", e);
            }
        }

        let repo_id = req.repo_id.clone();
        let patch_data = serde_json::json!({
            "patch_count": req.patches.len(),
            "branch_count": req.branches.len(),
            "existing_patches": existing_patches.clone(),
        });
        let manager = Arc::clone(&self.webhook_manager);
        let storage = Arc::clone(&self.storage);
        tokio::spawn(async move {
            let hooks = {
                let store = storage.read().await;
                store.list_webhooks(&repo_id).unwrap_or_else(|e| {
                    tracing::warn!("store list_webhooks failed: {e}");
                    Default::default()
                })
            };
            if !hooks.is_empty() {
                let result = manager.trigger(&hooks, "push", &repo_id, patch_data).await;
                if result.failed > 0 {
                    tracing::warn!("Hook trigger failed: {} of {} webhooks failed", result.failed, result.triggered);
                }
            }
        });

        Ok(PushResponse {
            success: true,
            error: None,
            existing_patches,
        })
    }

    pub async fn handle_pull(&self, req: PullRequest) -> PullResponse {
        let store = self.storage.read().await;

        let exists = match store.repo_exists(&req.repo_id) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Failed to check repo existence: {e}");
                return PullResponse {
                    success: false,
                    error: Some(format!("database error: {e}")),
                    patches: vec![],
                    branches: vec![],
                    blobs: vec![],
                };
            }
        };
        if !exists {
            return PullResponse {
                success: false,
                error: Some(format!("repo not found: {}", req.repo_id)),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
            };
        }

        let all_patches = store.get_all_patches_unbounded(&req.repo_id).unwrap_or_else(|e| {
            tracing::warn!("store get_all_patches failed: {e}");
            Default::default()
        });
        let client_ancestors = collect_ancestors(&all_patches, &req.known_branches);
        let mut new_patches = collect_new_patches(&all_patches, &client_ancestors);

        if let Some(depth) = req.max_depth {
            new_patches.truncate(depth as usize);
        }

        let branches = store.get_branches(&req.repo_id).unwrap_or_else(|e| {
            tracing::warn!("store get_branches failed: {e}");
            Default::default()
        });

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
            .unwrap_or_else(|e| {
                tracing::warn!("store get_blobs failed: {e}");
                Default::default()
            });

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
        let repo_ids = match store.list_repos() {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to list repos: {e}");
                return ListReposResponse { repo_ids: vec![] };
            }
        };
        ListReposResponse { repo_ids }
    }

    pub async fn handle_repo_info(&self, repo_id: &str) -> RepoInfoResponse {
        let store = self.storage.read().await;

        let exists = match store.repo_exists(repo_id) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Failed to check repo existence: {e}");
                return RepoInfoResponse {
                    repo_id: repo_id.to_owned(),
                    patch_count: 0,
                    branches: vec![],
                    success: false,
                    error: Some(format!("database error: {e}")),
                };
            }
        };
        if !exists {
            return RepoInfoResponse {
                repo_id: repo_id.to_owned(),
                patch_count: 0,
                branches: vec![],
                success: false,
                error: Some(format!("repo not found: {repo_id}")),
            };
        }

        let patch_count = store.patch_count(repo_id).unwrap_or_else(|e| {
            tracing::error!("Failed to get patch count: {e}");
            0
        });
        let branches = store.get_branches(repo_id).unwrap_or_else(|e| {
            tracing::warn!("store get_branches failed: {e}");
            Default::default()
        });

        RepoInfoResponse {
            repo_id: repo_id.to_owned(),
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
            .post(format!("{upstream_url}/pull"))
            .json(&upstream_pull)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let store = self.storage.write().await;
                if let Err(e) = store.update_mirror_status(req.mirror_id, "error", None) {
                    tracing::warn!("Failed to update mirror status: {}", e);
                }
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
                if let Err(e) = store.update_mirror_status(req.mirror_id, "error", None) {
                    tracing::warn!("Failed to update mirror status: {}", e);
                }
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
            if let Err(e) = store.update_mirror_status(req.mirror_id, "error", None) {
                tracing::warn!("Failed to update mirror status: {}", e);
            }
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
            let Ok(data) = base64_decode(&blob.data) else { continue };
            if let Err(e) = self.blob_store(&store, &local_repo, &hex, &data) {
                tracing::warn!("Failed to store blob during mirror sync: {}", e);
            }
        }

        for patch in &pull_result.patches {
            let inserted = store.insert_patch(&local_repo, patch).unwrap_or_else(|e| {
                tracing::warn!("Failed to insert patch during mirror sync: {e}");
                false
            });
            if inserted {
                patches_synced += 1;
            }
        }

        let branches_synced = pull_result.branches.len() as u64;
        for branch in &pull_result.branches {
            let target_hex = hash_to_hex(&branch.target_id);
            if let Err(e) = store.set_branch(&local_repo, &branch.name, &target_hex) {
                tracing::warn!("Failed to update branch during mirror sync: {}", e);
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if let Err(e) = store.update_mirror_status(req.mirror_id, "idle", Some(now)) {
            tracing::warn!("Failed to update mirror status: {}", e);
        }

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

        let mirrors = store.list_mirrors().unwrap_or_else(|e| {
            tracing::warn!("store list_mirrors failed: {e}");
            Default::default()
        });

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

    pub async fn handle_pull_v2(
        &self,
        req: crate::types::PullRequestV2,
    ) -> crate::types::PullResponseV2 {
        let store = self.storage.read().await;

        let exists = match store.repo_exists(&req.repo_id) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Failed to check repo existence: {e}");
                return crate::types::PullResponseV2 {
                    success: false,
                    error: Some(format!("database error: {e}")),
                    patches: vec![],
                    branches: vec![],
                    blobs: vec![],
                    deltas: vec![],
                    protocol_version: crate::types::PROTOCOL_VERSION_V2,
                };
            }
        };
        if !exists {
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

        let all_patches = store.get_all_patches_unbounded(&req.repo_id).unwrap_or_else(|e| {
            tracing::warn!("store get_all_patches failed: {e}");
            Default::default()
        });
        let client_ancestors = collect_ancestors(&all_patches, &req.known_branches);
        let mut new_patches = collect_new_patches(&all_patches, &client_ancestors);

        if let Some(depth) = req.max_depth {
            new_patches.truncate(depth as usize);
        }

        let branches = store.get_branches(&req.repo_id).unwrap_or_else(|e| {
            tracing::warn!("store get_branches failed: {e}");
            Default::default()
        });

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
                let Ok(Some(target_data)) = self.blob_get(&store, &req.repo_id, needed_hash) else {
                        if let Ok(b) = store.get_blobs(
                            &req.repo_id,
                            &std::collections::HashSet::from([needed_hash.clone()]),
                        ) && let Some(blob) = b.into_iter().next()
                        {
                            blobs.push(blob);
                        }
                        continue;
                    };

                if known_hash_set.contains(needed_hash) {
                    let Ok(Some(base_data)) = self.blob_get(&store, &req.repo_id, needed_hash) else {
                            blobs.push(BlobRef {
                                hash: HashProto {
                                    value: needed_hash.clone(),
                                },
                                data: base64_encode(&target_data),
                                truncated: false,
                            });
                            continue;
                        };

                    if base_data == target_data {
                        continue;
                    }

                    let (_base_copy, delta_bytes) =
                        suture_protocol::compute_delta(&base_data, &target_data);

                    if delta_bytes.len() < target_data.len() {
                        deltas.push(BlobDelta {
                            base_hash: HashProto {
                                value: needed_hash.clone(),
                            },
                            target_hash: HashProto {
                                value: needed_hash.clone(),
                            },
                            encoding: DeltaEncoding::BinaryPatch,
                            delta_data: base64_encode(&delta_bytes),
                        });
                    } else {
                        blobs.push(BlobRef {
                            hash: HashProto {
                                value: needed_hash.clone(),
                            },
                            data: base64_encode(&target_data),
                            truncated: false,
                        });
                    }
                } else {
                    blobs.push(BlobRef {
                        hash: HashProto {
                            value: needed_hash.clone(),
                        },
                        data: base64_encode(&target_data),
                        truncated: false,
                    });
                }
            }
        } else {
            blobs = store
                .get_blobs(&req.repo_id, &needed_hashes)
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to get blobs for repo {}: {e}", req.repo_id);
                    Default::default()
                });
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
            let has_keys = match store.has_authorized_keys() {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Failed to check authorized keys: {e}");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        PushResponse {
                            success: false,
                            error: Some("database error".to_owned()),
                            existing_patches: vec![],
                        },
                    ));
                }
            };
            if has_keys {
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
            let has_keys = store.has_authorized_keys().unwrap_or_else(|e| {
                tracing::error!("Failed to check authorized keys: {e}");
                true
            });
            let has_tokens = store.has_tokens().unwrap_or_else(|e| {
                tracing::error!("Failed to check tokens: {e}");
                true
            });
            if has_keys || has_tokens {
                return Err((
                    StatusCode::FORBIDDEN,
                    PushResponse {
                        success: false,
                        error: Some("authentication required: no signature provided".to_owned()),
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
                let Ok(Some(base_data)) = self.blob_get(&store, &req.repo_id, &base_hex) else { continue };
                let Ok(delta_bytes) = base64_decode(&delta.delta_data) else { continue };
                let reconstructed = suture_protocol::apply_delta(&base_data, &delta_bytes);
                if let Err(e) = self.blob_store(&store, &req.repo_id, &target_hex, &reconstructed) {
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
                if let Err(e) = self.blob_store(&store, &req.repo_id, &target_hex, &data) {
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
            if let Err(e) = self.blob_store(&store, &req.repo_id, &hex, &data) {
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

        for patch in &req.patches {
            if let Err(e) = store.log_operation("insert", "patches", &hash_to_hex(&patch.id), None) {
                tracing::warn!("Failed to log operation: {}", e);
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
                        .unwrap_or_else(|e| {
                            tracing::warn!("Failed to check ancestry: {e}");
                            true
                        })
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
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to check branch protection: {e}");
                    false
                })
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

        for branch in &req.branches {
            let target_hex = hash_to_hex(&branch.target_id);
            if let Err(e) = store.log_operation(
                "set",
                "branches",
                &format!("{}:{}", req.repo_id, branch.name),
                Some(&target_hex),
            ) {
                tracing::warn!("Failed to log operation: {}", e);
            }
        }

        let repo_id = req.repo_id.clone();
        let patch_data = serde_json::json!({
            "patch_count": req.patches.len(),
            "branch_count": req.branches.len(),
            "existing_patches": existing_patches.clone(),
        });
        let manager = Arc::clone(&self.webhook_manager);
        let storage = Arc::clone(&self.storage);
        tokio::spawn(async move {
            let hooks = {
                let store = storage.read().await;
                store.list_webhooks(&repo_id).unwrap_or_else(|e| {
                    tracing::warn!("store list_webhooks failed: {e}");
                    Default::default()
                })
            };
            if !hooks.is_empty() {
                let result = manager.trigger(&hooks, "push", &repo_id, patch_data).await;
                if result.failed > 0 {
                    tracing::warn!("Hook trigger failed: {} of {} webhooks failed", result.failed, result.triggered);
                }
            }
        });

        Ok(PushResponse {
            success: true,
            error: None,
            existing_patches,
        })
    }

    pub async fn handle_batch_push(
        &self,
        req: BatchPatchRequest,
    ) -> Result<PushResponse, (StatusCode, PushResponse)> {
        let mut existing_patches = Vec::new();

        let store = self.storage.write().await;
        if let Err(e) = store.ensure_repo(&req.repo_id) {
            let msg = format!("storage error: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                PushResponse {
                    success: false,
                    error: Some(msg),
                    existing_patches: vec![],
                },
            ));
        }

        for blob in &req.blobs {
            let hex = hash_to_hex(&blob.hash);
            let data = match base64_decode(&blob.data) {
                Ok(d) => d,
                Err(e) => {
                    let msg = format!("invalid base64 in blob: {e}");
                    return Err((
                        StatusCode::BAD_REQUEST,
                        PushResponse {
                            success: false,
                            error: Some(msg),
                            existing_patches: vec![],
                        },
                    ));
                }
            };
            if let Err(e) = self.blob_store(&store, &req.repo_id, &hex, &data) {
                let msg = format!("storage error: {e}");
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    PushResponse {
                        success: false,
                        error: Some(msg),
                        existing_patches: vec![],
                    },
                ));
            }
        }

        for patch in &req.patches {
            let inserted = match store.insert_patch(&req.repo_id, patch) {
                Ok(i) => i,
                Err(e) => {
                    let msg = format!("storage error: {e}");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        PushResponse {
                            success: false,
                            error: Some(msg),
                            existing_patches: vec![],
                        },
                    ));
                }
            };
            if !inserted {
                existing_patches.push(patch.id.clone());
            }
        }

        for patch in &req.patches {
            if let Err(e) = store.log_operation("insert", "patches", &hash_to_hex(&patch.id), None) {
                tracing::warn!("Failed to log operation: {}", e);
            }
        }

        for branch in &req.branches {
            let target_hex = hash_to_hex(&branch.target_id);

            if store
                .is_branch_protected(&req.repo_id, &branch.name)
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to check branch protection: {e}");
                    false
                })
            {
                let push_authors: std::collections::HashSet<&str> =
                    req.patches.iter().map(|p| p.author.as_str()).collect();
                let is_owner =
                    push_authors.len() == 1 && push_authors.contains(branch.name.as_str());
                if !is_owner && !req.force {
                    let msg = format!(
                        "branch '{}' is protected and can only be updated by its owner",
                        branch.name
                    );
                    return Err((
                        StatusCode::FORBIDDEN,
                        PushResponse {
                            success: false,
                            error: Some(msg),
                            existing_patches: vec![],
                        },
                    ));
                }
            }

            if let Err(e) = store.set_branch(&req.repo_id, &branch.name, &target_hex) {
                let msg = format!("storage error: {e}");
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    PushResponse {
                        success: false,
                        error: Some(msg),
                        existing_patches: vec![],
                    },
                ));
            }
        }

        for branch in &req.branches {
            let target_hex = hash_to_hex(&branch.target_id);
            if let Err(e) = store.log_operation(
                "set",
                "branches",
                &format!("{}:{}", req.repo_id, branch.name),
                Some(&target_hex),
            ) {
                tracing::warn!("Failed to log operation: {}", e);
            }
        }

        let repo_id = req.repo_id.clone();
        let patch_data = serde_json::json!({
            "patch_count": req.patches.len(),
            "branch_count": req.branches.len(),
            "existing_patches": existing_patches.clone(),
        });
        let manager = Arc::clone(&self.webhook_manager);
        let storage = Arc::clone(&self.storage);
        tokio::spawn(async move {
            let hooks = {
                let store = storage.read().await;
                store.list_webhooks(&repo_id).unwrap_or_else(|e| {
                    tracing::warn!("store list_webhooks failed: {e}");
                    Default::default()
                })
            };
            if !hooks.is_empty() {
                let result = manager.trigger(&hooks, "push", &repo_id, patch_data).await;
                if result.failed > 0 {
                    tracing::warn!("Hook trigger failed: {} of {} webhooks failed", result.failed, result.triggered);
                }
            }
        });

        Ok(PushResponse {
            success: true,
            error: None,
            existing_patches,
        })
    }

    #[cfg(feature = "raft-cluster")]
    pub async fn apply_raft_command(&self, cmd: crate::raft::HubCommand) -> Result<(), String> {
        use crate::raft::HubCommand;

        let store = self.storage.write().await;

        match cmd {
            HubCommand::CreateRepo { repo_id } => {
                store.ensure_repo(&repo_id).map_err(|e| e.to_string())?;
                Ok(())
            }
            HubCommand::DeleteRepo { repo_id } => {
                store.delete_repo(&repo_id).map_err(|e| e.to_string())
            }
            HubCommand::StoreBlob { hash, data } => store
                .store_blob("_raft_default", &hash, &data)
                .map_err(|e| e.to_string()),
            HubCommand::DeleteBlob { hash } => {
                if let Err(e) = store.delete_blob("_raft_default", &hash) {
                    tracing::warn!("Failed to delete blob: {}", e);
                }
                Ok(())
            }
            HubCommand::CreateBranch {
                repo_id,
                branch,
                target,
            }
            | HubCommand::UpdateBranch {
                repo_id,
                branch,
                target,
            } => store
                .set_branch(&repo_id, &branch, &target)
                .map_err(|e| e.to_string()),
            HubCommand::DeleteBranch { repo_id, branch } => store
                .delete_branch(&repo_id, &branch)
                .map_err(|e| e.to_string()),
            HubCommand::StorePatch {
                repo_id,
                patch_id,
                patch_data,
            } => {
                let patch: crate::types::PatchProto = match serde_json::from_slice(&patch_data) {
                    Ok(p) => p,
                    Err(e) => return Err(format!("failed to deserialize patch: {e}")),
                };
                let expected_hex = patch_id;
                let actual_hex = hash_to_hex(&patch.id);
                if actual_hex != expected_hex {
                    return Err(format!(
                        "patch_id mismatch: expected {expected_hex}, got {actual_hex}"
                    ));
                }
                store
                    .insert_patch(&repo_id, &patch)
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
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
        return Err("signature must be 64 bytes".to_owned());
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
        let pub_key_bytes = match store.get_authorized_key(author) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => continue,
            Err(e) => {
                tracing::warn!("Failed to get authorized key for '{}': {e}", author);
                continue;
            }
        };
        if pub_key_bytes.len() != 32 {
            continue;
        }
        let pub_key_array: [u8; 32] = pub_key_bytes
            .try_into()
            .map_err(|_| "invalid public key length")?;
        let verifying_key = VerifyingKey::from_bytes(&pub_key_array)
            .map_err(|e| format!("invalid public key: {e}"))?;
        if verifying_key.verify(&canonical, &signature).is_ok() {
            return Ok(());
        }
    }

    Err("no matching authorized key found for signature".to_owned())
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
    let auth_keys_configured = store.has_authorized_keys().unwrap_or_else(|e| {
        tracing::error!("Failed to check authorized keys: {e}");
        true
    });
    let tokens_exist = store.has_tokens().unwrap_or_else(|e| {
        tracing::error!("Failed to check tokens: {e}");
        true
    });
    drop(store);

    if !auth_keys_configured && !tokens_exist {
        return Ok(());
    }

    if let Some(auth_header) = headers.get("authorization")
        && let Ok(auth_str) = auth_header.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
    {
        let store = hub.storage.read().await;
        if store.verify_token(token).unwrap_or_else(|e| {
            tracing::error!("Failed to verify token: {e}");
            false
        }) {
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
        return match store.get_user_by_token(token) {
            Ok(Some(user)) => Some(user),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("Failed to get user by token: {e}");
                None
            }
        };
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

    let user = resolve_user(hub, headers)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let user_role = Role::parse(&user.role);

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
                error: Some("authentication failed".to_owned()),
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
                error: Some("authentication failed".to_owned()),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
            }),
        );
    }
    let mut resp = hub.handle_pull(req).await;
    if resp.success {
        for blob in &mut resp.blobs {
            let Ok(raw) = base64_decode(&blob.data) else { continue };
            let Ok(compressed) = suture_protocol::compress(&raw) else { continue };
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
        if let Ok(val) = retry_after.to_string().parse() {
            hdrs.insert(axum::http::header::RETRY_AFTER, val);
        }
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(PushResponse {
                success: false,
                error: Some("rate limit exceeded".to_owned()),
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
                error: Some("authentication failed".to_owned()),
                existing_patches: vec![],
            }),
        );
    }

    if !hub.no_auth
        && let Some(user) = resolve_user(&hub, &headers).await
        && Role::parse(&user.role) < Role::Member
    {
        return (
            StatusCode::FORBIDDEN,
            HeaderMap::new(),
            Json(PushResponse {
                success: false,
                error: Some("insufficient permissions: readers cannot push".to_owned()),
                existing_patches: vec![],
            }),
        );
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
        if let Ok(val) = retry_after.to_string().parse() {
            hdrs.insert(axum::http::header::RETRY_AFTER, val);
        }
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(PullResponse {
                success: false,
                error: Some("rate limit exceeded".to_owned()),
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
                error: Some("authentication failed".to_owned()),
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
        server_name: "suture-hub".to_owned(),
        compatible,
    })
}

/// GET /handshake — returns version info without requiring a request body.
/// Used by `suture push`/`suture pull` which send a bare GET for compatibility checking.
pub async fn handshake_get_handler() -> Json<crate::types::HandshakeResponse> {
    Json(crate::types::HandshakeResponse {
        server_version: crate::types::PROTOCOL_VERSION,
        server_name: "suture-hub".to_owned(),
        compatible: true,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub created_at: u64,
}

pub async fn create_token_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
) -> (StatusCode, HeaderMap, Json<TokenResponse>) {
    let ip = addr.ip().to_string();
    if let Err(retry_after) = hub.check_rate_limit(&ip, "token_create") {
        let mut hdrs = HeaderMap::new();
        if let Ok(val) = retry_after.to_string().parse() {
            hdrs.insert(axum::http::header::RETRY_AFTER, val);
        }
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(TokenResponse {
                token: String::new(),
                created_at: 0,
            }),
        );
    }

    if hub.no_auth {
        let token = generate_random_token();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = (created_at + (30 * 24 * 60 * 60)) as i64;
        let store = hub.storage.write().await;
        if let Err(e) = store.store_token(&token, created_at, "cli-generated", expires_at) {
            tracing::error!("Failed to store auth token: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::new(),
                Json(TokenResponse { token: String::new(), created_at: 0 }),
            );
        }
        return (
            StatusCode::OK,
            HeaderMap::new(),
            Json(TokenResponse { token, created_at }),
        );
    }

    let store = hub.storage.read().await;
    let tokens_exist = store.has_tokens().unwrap_or_else(|e| {
        tracing::error!("Failed to check tokens: {e}");
        true
    });
    let users_exist = store.has_users().unwrap_or_else(|e| {
        tracing::error!("Failed to check users: {e}");
        true
    });
    let auth_keys_configured = store.has_authorized_keys().unwrap_or_else(|e| {
        tracing::error!("Failed to check authorized keys: {e}");
        true
    });
    drop(store);

    if !tokens_exist && !users_exist && !auth_keys_configured {
        let token = generate_random_token();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = (created_at + (30 * 24 * 60 * 60)) as i64;
        let store = hub.storage.write().await;
        if let Err(e) = store.store_token(&token, created_at, "cli-generated", expires_at) {
            tracing::error!("Failed to store auth token: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::new(),
                Json(TokenResponse { token: String::new(), created_at: 0 }),
            );
        }
        return (
            StatusCode::OK,
            HeaderMap::new(),
            Json(TokenResponse { token, created_at }),
        );
    }

    let user = resolve_user(&hub, &headers).await;
    if let Some(u) = user {
        let role = Role::parse(&u.role);
        if role < Role::Admin {
            return (
                StatusCode::FORBIDDEN,
                HeaderMap::new(),
                Json(TokenResponse {
                    token: String::new(),
                    created_at: 0,
                }),
            );
        }
    } else {
        let store = hub.storage.read().await;
        let valid_token = if let Some(auth_header) = headers.get("authorization")
            && let Ok(auth_str) = auth_header.to_str()
            && let Some(token) = auth_str.strip_prefix("Bearer ")
        {
            store.verify_token(token).unwrap_or_else(|e| {
                tracing::error!("Failed to verify token: {e}");
                false
            })
        } else {
            false
        };
        drop(store);
        if !valid_token {
            return (
                StatusCode::UNAUTHORIZED,
                HeaderMap::new(),
                Json(TokenResponse {
                    token: String::new(),
                    created_at: 0,
                }),
            );
        }
    }

    let token = generate_random_token();
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let expires_at = (created_at + (30 * 24 * 60 * 60)) as i64;

    let store = hub.storage.write().await;
    if let Err(e) = store.store_token(&token, created_at, "cli-generated", expires_at) {
        tracing::error!("Failed to store auth token: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            HeaderMap::new(),
            Json(TokenResponse {
                token: String::new(),
                created_at: 0,
            }),
        );
    }

    (
        StatusCode::OK,
        HeaderMap::new(),
        Json(TokenResponse { token, created_at }),
    )
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
            store.verify_token(token).unwrap_or_else(|e| {
                tracing::warn!("Failed to verify token: {e}");
                false
            })
        }
        _ => false,
    };
    Json(VerifyResponse { valid })
}

pub async fn mirror_setup_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(req): Json<crate::types::MirrorSetupRequest>,
) -> (StatusCode, Json<crate::types::MirrorSetupResponse>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        let resp = crate::types::MirrorSetupResponse { success: false, mirror_id: None, error: Some("unauthorized".to_owned()) };
        return (status, Json(resp));
    }
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
    headers: HeaderMap,
    Json(req): Json<crate::types::MirrorSyncRequest>,
) -> (StatusCode, Json<crate::types::MirrorSyncResponse>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        let resp = crate::types::MirrorSyncResponse { success: false, error: Some("unauthorized".to_owned()), patches_synced: 0, branches_synced: 0 };
        return (status, Json(resp));
    }
    let store = hub.storage.read().await;
    let actual_mirror_id = if req.mirror_id == 0 {
        let repo_name = req.local_repo.clone().unwrap_or_default();
        match store.get_mirror_by_repo(&repo_name) {
            Ok(Some(id)) => id,
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(crate::types::MirrorSyncResponse {
                        success: false,
                        error: Some("mirror not found by local_repo".to_owned()),
                        patches_synced: 0,
                        branches_synced: 0,
                    }),
                );
            }
        }
    } else {
        req.mirror_id
    };
    drop(store);
    let actual_req = crate::types::MirrorSyncRequest {
        mirror_id: actual_mirror_id,
        local_repo: req.local_repo,
        remote_url: req.remote_url,
    };
    let resp = hub.handle_mirror_sync(actual_req).await;
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

pub async fn mirror_status_get_handler(
    State(hub): State<Arc<SutureHubServer>>,
) -> (StatusCode, Json<crate::types::MirrorStatusResponse>) {
    let resp = hub
        .handle_mirror_status(crate::types::MirrorStatusRequest {
            mirror_id: None,
            repo_name: None,
        })
        .await;
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
        let branches = store.get_branches(&repo_id).unwrap_or_else(|e| {
            tracing::warn!("store get_branches failed: {e}");
            Default::default()
        });
    (StatusCode::OK, Json(branches))
}

pub async fn repo_patches_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path(repo_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> (StatusCode, Json<serde_json::Value>) {
    let offset = params
        .cursor
        .as_deref()
        .and_then(decode_cursor)
        .unwrap_or_else(|| u64::from(params.offset.unwrap_or(0)));
    let limit = params.limit.unwrap_or(50);
    let (patches, next_cursor) = hub
        .handle_repo_patches_cursor(&repo_id, offset, limit)
        .await;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "patches": patches,
            "next_cursor": next_cursor.unwrap_or_default(),
        })),
    )
}

pub async fn repo_tree_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path((repo_id, branch)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    let store = hub.storage.read().await;
    match store.get_tree_at_branch(&repo_id, &branch) {
        Ok(entries) => {
            let files: Vec<serde_json::Value> = entries
                .into_iter()
                .map(|e| {
                    serde_json::json!({
                        "path": e.path,
                        "content_hash": e.content_hash,
                    })
                })
                .collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "files": files})),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn protect_branch_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path((repo_id, branch)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"success": false, "error": "unauthorized"})));
    }
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
    headers: HeaderMap,
    Path((repo_id, branch)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"success": false, "error": "unauthorized"})));
    }
    let store = hub.storage.write().await;
    match store.unprotect_branch(&repo_id, &branch) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn create_repo_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(req): Json<crate::types::CreateRepoRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(serde_json::json!({"success": false, "error": "unauthorized"})),
        );
    }
    let store = hub.storage.write().await;
    match store.ensure_repo(&req.repo_id) {
        Ok(_) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "repo_id": req.repo_id})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn delete_repo_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(repo_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(serde_json::json!({"success": false, "error": "unauthorized"})),
        );
    }
    let store = hub.storage.write().await;
    match store.delete_repo(&repo_id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn create_branch_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(repo_id): Path<String>,
    Json(req): Json<crate::types::CreateBranchRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(serde_json::json!({"success": false, "error": "unauthorized"})),
        );
    }
    let store = hub.storage.write().await;
    match store.set_branch(&repo_id, &req.name, &req.target) {
        Ok(()) => {
            let branch_data = serde_json::json!({"name": req.name, "target": req.target});
            let manager = Arc::clone(&hub.webhook_manager);
            let storage = Arc::clone(&hub.storage);
            let rid = repo_id.clone();
            drop(store);
            tokio::spawn(async move {
                let hooks = {
                    let store = storage.read().await;
                    store.list_webhooks(&rid).unwrap_or_else(|e| {
                        tracing::warn!("store list_webhooks failed: {e}");
                        Default::default()
                    })
                };
                if !hooks.is_empty() {
                    let result = manager
                        .trigger(&hooks, "branch.create", &rid, branch_data)
                        .await;
                    if result.failed > 0 {
                        tracing::warn!("Hook trigger failed: {} of {} webhooks failed", result.failed, result.triggered);
                    }
                }
            });
            (
                StatusCode::CREATED,
                Json(serde_json::json!({"success": true})),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn delete_branch_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path((repo_id, branch_name)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(serde_json::json!({"success": false, "error": "unauthorized"})),
        );
    }
    let store = hub.storage.write().await;
    match store.delete_branch(&repo_id, &branch_name) {
        Ok(()) => {
            let branch_data = serde_json::json!({"name": branch_name});
            let manager = Arc::clone(&hub.webhook_manager);
            let storage = Arc::clone(&hub.storage);
            let rid = repo_id.clone();
            drop(store);
            tokio::spawn(async move {
                let hooks = {
                    let store = storage.read().await;
                    store.list_webhooks(&rid).unwrap_or_else(|e| {
                        tracing::warn!("store list_webhooks failed: {e}");
                        Default::default()
                    })
                };
                if !hooks.is_empty() {
                    let result = manager
                        .trigger(&hooks, "branch.delete", &rid, branch_data)
                        .await;
                    if result.failed > 0 {
                        tracing::warn!("Hook trigger failed: {} of {} webhooks failed", result.failed, result.triggered);
                    }
                }
            });
            (StatusCode::OK, Json(serde_json::json!({"success": true})))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn get_blob_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path((repo_id, hash)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    let store = hub.storage.read().await;
    match hub.blob_get(&store, &repo_id, &hash) {
        Ok(Some(data)) => {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "data": encoded})),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "blob not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

pub async fn lfs_batch_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(req): Json<suture_protocol::LfsBatchRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate repo_id to prevent path traversal
    if !req.repo_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "message": "invalid repo_id: must contain only alphanumeric characters, hyphens, underscores, and dots"
            })),
        );
    }

    let lfs_dir = match &hub.lfs_data_dir {
        Some(d) => d.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "message": "LFS storage not configured on this hub"
                })),
            );
        }
    };

    let repo_dir = lfs_dir.join(&req.repo_id);
    let obj_dir = repo_dir.join("objects");
    let obj_dir_clone = obj_dir.clone();
    if let Err(e) = tokio::task::spawn_blocking(move || std::fs::create_dir_all(obj_dir_clone))
        .await.unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())))
    {
        tracing::warn!("Failed to create directory {}: {}", obj_dir.display(), e);
    }

    let mut actions = Vec::with_capacity(req.objects.len());
    for obj in &req.objects {
        let oid = &obj.oid;
        if !oid.chars().all(|c| c.is_ascii_hexdigit()) || oid.len() > 128 {
            continue;
        }
        let prefix = &oid[..2.min(oid.len())];
        let obj_path = obj_dir.join(prefix).join(oid);

        let action = match req.operation {
            suture_protocol::LfsOperation::Upload => {
                let op = obj_path.clone();
                let exists = tokio::task::spawn_blocking(move || op.exists())
                    .await
                    .unwrap_or(false);
                if exists {
                    suture_protocol::LfsAction::None
                } else {
                    suture_protocol::LfsAction::Upload
                }
            }
            suture_protocol::LfsOperation::Download => {
                let op = obj_path.clone();
                let exists = tokio::task::spawn_blocking(move || op.exists())
                    .await
                    .unwrap_or(false);
                if exists {
                    suture_protocol::LfsAction::Download
                } else {
                    suture_protocol::LfsAction::None
                }
            }
        };

        actions.push(suture_protocol::LfsObjectAction {
            oid: obj.oid.clone(),
            size: obj.size,
            action,
            href: None,
            header: None,
        });
    }

    for action in &mut actions {
        if matches!(
            action.action,
            suture_protocol::LfsAction::Upload | suture_protocol::LfsAction::Download
        ) {
            action.href = Some(format!("/lfs/objects/{}/{}", req.repo_id, action.oid));
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "objects": actions,
            "transfer": "basic",
        })),
    )
}

pub async fn lfs_upload_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path((repo_id, oid)): Path<(String, String)>,
    body: bytes::Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({ "message": "unauthorized" })));
    }
    // Validate repo_id and oid to prevent path traversal
    let is_safe = |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !is_safe(&repo_id) || !is_safe(&oid) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "message": "invalid repo_id or oid" })),
        );
    }

    let lfs_dir = match &hub.lfs_data_dir {
        Some(d) => d.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "message": "LFS storage not configured"
                })),
            );
        }
    };

    if body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "message": "empty body"
            })),
        );
    }

    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(&body);
    let hash_hex = hex::encode(hash);
    if hash_hex != oid {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "message": format!("hash mismatch: expected {}, got {}", oid, hash_hex)
            })),
        );
    }

    let prefix = &oid[..2.min(oid.len())];
    let obj_path = lfs_dir
        .join(&repo_id)
        .join("objects")
        .join(prefix)
        .join(&oid);
    if let Some(parent) = obj_path.parent() {
        let parent_owned = parent.to_owned();
        if let Err(e) = tokio::task::spawn_blocking(move || std::fs::create_dir_all(parent_owned))
            .await.unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())))
        {
            tracing::warn!("Failed to create directory {}: {}", parent.display(), e);
        }
    }
    match tokio::task::spawn_blocking({
        let obj_path = obj_path.clone();
        let body = body.clone();
        move || std::fs::write(obj_path, body)
    }).await.unwrap_or_else(|e| Err(std::io::Error::other(e.to_string()))) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": "uploaded",
                "oid": oid,
                "size": body.len(),
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "message": e.to_string()
            })),
        ),
    }
}

pub async fn lfs_download_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Path((repo_id, oid)): Path<(String, String)>,
) -> (StatusCode, axum::response::Response) {
    // Validate repo_id and oid to prevent path traversal
    let is_safe = |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !is_safe(&repo_id) || !is_safe(&oid) {
        let body = axum::body::Body::from("{\"message\":\"invalid repo_id or oid\"}");
        let response = axum::response::Response::builder().body(body).unwrap_or_else(|_| {
            axum::response::Response::new(axum::body::Body::from("{\"message\":\"invalid repo_id or oid\"}"))
        });
        return (StatusCode::BAD_REQUEST, response.into_response());
    }

    let lfs_dir = match &hub.lfs_data_dir {
        Some(d) => d.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::response::Response::new(axum::body::Body::from(
                    serde_json::json!({"message": "LFS storage not configured"}).to_string(),
                )),
            );
        }
    };

    let prefix = &oid[..2.min(oid.len())];
    let obj_path = lfs_dir
        .join(&repo_id)
        .join("objects")
        .join(prefix)
        .join(&oid);

    match tokio::task::spawn_blocking({
        let obj_path = obj_path.clone();
        move || std::fs::read(obj_path)
    }).await.unwrap_or_else(|e| Err(std::io::Error::other(e.to_string()))) {
        Err(_) => (
            StatusCode::NOT_FOUND,
            axum::response::Response::new(axum::body::Body::from(
                serde_json::json!({"message": "object not found"}).to_string(),
            )),
        ),
        Ok(data) => {
            let len = data.len();
            let body = axum::body::Body::from(data);
            let response = axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/octet-stream")
                .header("Content-Length", len.to_string())
                .body(body)
                .unwrap_or_else(|e| {
                    tracing::error!("failed to build response: {}", e);
                    axum::response::Response::new(axum::body::Body::from("internal error"))
                });
            (StatusCode::OK, response)
        },
    }
}

pub async fn login_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Json(req): Json<crate::types::LoginRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let store = hub.storage.read().await;
    let valid = store.verify_token(&req.token).unwrap_or_else(|e| {
        tracing::warn!("Failed to verify token: {e}");
        false
    });
    if !valid {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"success": false, "error": "invalid token"})),
        );
    }
    let user = match store.get_user_by_token(&req.token) {
        Ok(Some(u)) => Some(u),
        Ok(None) => None,
        Err(e) => {
            tracing::error!("Failed to get user by token: {e}");
            None
        }
    };
    match user {
        Some(u) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "user": u, "token": req.token})),
        ),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"success": false, "error": "user not found"})),
        ),
    }
}

pub async fn search_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Query(params): Query<crate::types::SearchParams>,
) -> (StatusCode, Json<serde_json::Value>) {
    let store = hub.storage.read().await;
    let repos = match store.search_repos(&params.q) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to search repos: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "database error"})),
            );
        }
    };
    let mut patches = Vec::new();
    for repo_id in &repos {
        if let Ok(p) = store.search_patches(repo_id, &params.q) {
            patches.extend(p);
        }
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({"repos": repos, "patches": patches})),
    )
}

#[derive(serde::Deserialize)]
pub struct ActivityPaginationParams {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

pub async fn activity_handler(
    State(hub): State<Arc<SutureHubServer>>,
    Query(params): Query<ActivityPaginationParams>,
) -> (StatusCode, Json<serde_json::Value>) {
    let offset = params
        .cursor
        .as_deref()
        .and_then(decode_cursor)
        .unwrap_or(0);
    let limit = params.limit.unwrap_or(50).min(200) as usize;
    let store = hub.storage.read().await;
    let entries = store.get_replication_log(0).unwrap_or_else(|e| {
        tracing::warn!("store get_replication_log failed: {e}");
        Default::default()
    });
    let mut collected: Vec<_> = entries
        .into_iter()
        .skip(offset as usize)
        .take(limit + 1)
        .collect();
    let has_more = collected.len() > limit;
    if has_more {
        collected.truncate(limit);
    }
    let next_cursor = if has_more {
        Some(encode_cursor(offset + limit as u64))
    } else {
        None
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "entries": collected,
            "next_cursor": next_cursor.unwrap_or_default(),
        })),
    )
}

pub async fn delete_mirror_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(mirror_id): Path<i64>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(serde_json::json!({"success": false, "error": "unauthorized"})),
        );
    }
    let store = hub.storage.write().await;
    match store.delete_mirror(mirror_id) {
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

    let Ok(canonical_static) = tokio::fs::canonicalize(static_dir).await else { return StatusCode::NOT_FOUND.into_response() };
    let Ok(canonical_file) = tokio::fs::canonicalize(&file_path).await else { return StatusCode::NOT_FOUND.into_response() };

    if !canonical_file.starts_with(&canonical_static) {
        return StatusCode::FORBIDDEN.into_response();
    }

    tokio::fs::read_to_string(&canonical_file).await.map_or_else(
        |_| StatusCode::NOT_FOUND.into_response(),
        |contents| {
            let headers = [(axum::http::header::CONTENT_TYPE, content_type)];
            (StatusCode::OK, headers, contents).into_response()
        },
    )
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
                    error: Some("admin access required".to_owned()),
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
                error: Some("role must be admin, member, or reader".to_owned()),
                user: None,
            }),
        );
    }

    let api_token = generate_api_token();

    let store = hub.storage.write().await;
    match store.create_user(&req.username, &req.display_name, role, &api_token) {
        Ok(()) => {
            let mut user = match store.get_user(&req.username) {
                Ok(Some(u)) => Some(u),
                Ok(None) => None,
                Err(e) => {
                    tracing::error!("Failed to get user after creation: {e}");
                    None
                }
            };
            if let Some(ref mut u) = user {
                u.api_token = Some(api_token);
            }
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
                    error: Some("admin access required".to_owned()),
                    users: vec![],
                }),
            );
        }
    }

    let store = hub.storage.read().await;
    match store.list_users() {
        Ok(users) => {
            // Redact API tokens before returning — they are secrets that
            // must never be exposed in list responses.
            let users: Vec<_> = users
                .into_iter()
                .map(|mut u| {
                    u.api_token = None;
                    u
                })
                .collect();
            (
                StatusCode::OK,
                Json(crate::types::ListUsersResponse {
                    success: true,
                    error: None,
                    users,
                }),
            )
        }
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
        .is_some_and(|u| u.role == "admin");
    let is_self = requesting_user
        .as_ref()
        .is_some_and(|u| u.username == username);

    if !is_admin && !is_self {
        return (
            StatusCode::FORBIDDEN,
            Json(crate::types::GetUserResponse {
                success: false,
                error: Some("access denied".to_owned()),
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
                error: Some("user not found".to_owned()),
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
                    error: Some("admin access required".to_owned()),
                }),
            );
        }
    }

    if !matches!(req.role.as_str(), "admin" | "member" | "reader") {
        return (
            StatusCode::BAD_REQUEST,
            Json(crate::types::UpdateRoleResponse {
                success: false,
                error: Some("role must be admin, member, or reader".to_owned()),
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
                    error: Some("admin access required".to_owned()),
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
        server_name: "suture-hub".to_owned(),
        compatible,
        server_capabilities: crate::types::ServerCapabilities {
            supports_delta: true,
            supports_compression: true,
            max_blob_size: 50 * 1024 * 1024,
            protocol_versions: vec![
                crate::types::PROTOCOL_VERSION,
                crate::types::PROTOCOL_VERSION_V2,
            ],
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
        if let Ok(val) = retry_after.to_string().parse() {
            hdrs.insert(axum::http::header::RETRY_AFTER, val);
        }
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(crate::types::PullResponseV2 {
                success: false,
                error: Some("rate limit exceeded".to_owned()),
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
                error: Some("authentication failed".to_owned()),
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
        if let Ok(val) = retry_after.to_string().parse() {
            hdrs.insert(axum::http::header::RETRY_AFTER, val);
        }
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(PushResponse {
                success: false,
                error: Some("rate limit exceeded".to_owned()),
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
                error: Some("authentication failed".to_owned()),
                existing_patches: vec![],
            }),
        );
    }

    if !hub.no_auth
        && let Some(user) = resolve_user(&hub, &headers).await
        && Role::parse(&user.role) < Role::Member
    {
        return (
            StatusCode::FORBIDDEN,
            HeaderMap::new(),
            Json(PushResponse {
                success: false,
                error: Some("insufficient permissions: readers cannot push".to_owned()),
                existing_patches: vec![],
            }),
        );
    }

    match hub.handle_push_v2(req).await {
        Ok(resp) => (StatusCode::OK, HeaderMap::new(), Json(resp)),
        Err((status, resp)) => (status, HeaderMap::new(), Json(resp)),
    }
}

pub async fn add_peer_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(req): Json<AddPeerRequest>,
) -> (StatusCode, Json<AddPeerResponse>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(AddPeerResponse { success: false, peer_id: None, error: Some("unauthorized".to_owned()) }));
    }
    let role = hub.get_replication_role();
    if role != "leader" {
        return (
            StatusCode::FORBIDDEN,
            Json(AddPeerResponse {
                success: false,
                peer_id: None,
                error: Some("only the leader can manage peers".to_owned()),
            }),
        );
    }
    let resp = hub.handle_add_peer(req).await;
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(resp))
}

pub async fn remove_peer_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<RemovePeerResponse>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(RemovePeerResponse { success: false, error: Some("unauthorized".to_owned()) }));
    }
    let role = hub.get_replication_role();
    if role != "leader" {
        return (
            StatusCode::FORBIDDEN,
            Json(RemovePeerResponse {
                success: false,
                error: Some("only the leader can manage peers".to_owned()),
            }),
        );
    }
    let resp = hub.handle_remove_peer(id).await;
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
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
                error: Some("sync endpoint is for followers only".to_owned()),
            }),
        );
    }
    let resp = hub.handle_replication_sync(entries).await;
    let status = if resp.success {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(resp))
}

pub async fn batch_push_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<BatchPatchRequest>,
) -> (StatusCode, HeaderMap, Json<PushResponse>) {
    let ip = addr.ip().to_string();
    if let Err(retry_after) = hub.check_rate_limit(&ip, "push") {
        let mut hdrs = HeaderMap::new();
        if let Ok(val) = retry_after.to_string().parse() {
            hdrs.insert(axum::http::header::RETRY_AFTER, val);
        }
        return (
            StatusCode::TOO_MANY_REQUESTS,
            hdrs,
            Json(PushResponse {
                success: false,
                error: Some("rate limit exceeded".to_owned()),
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
                error: Some("authentication failed".to_owned()),
                existing_patches: vec![],
            }),
        );
    }

    match hub.handle_batch_push(req).await {
        Ok(resp) => (StatusCode::OK, HeaderMap::new(), Json(resp)),
        Err((status, resp)) => (status, HeaderMap::new(), Json(resp)),
    }
}

async fn replication_background_task(hub: Arc<SutureHubServer>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        interval.tick().await;

        let role = hub.get_replication_role();
        if role != "leader" {
            continue;
        }

        let peers = {
            let store = hub.storage.read().await;
            match store.list_replication_peers() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("replication: failed to list peers: {e}");
                    continue;
                }
            }
        };

        for peer in &peers {
            if peer.status != "active" {
                continue;
            }

            let entries = {
                let store = hub.storage.read().await;
                match store.get_replication_log(peer.last_sync_seq) {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("replication: failed to get log for peer {}: {e}", peer.id);
                        continue;
                    }
                }
            };

            if entries.is_empty() {
                continue;
            }

            let last_seq = entries.last().map_or(peer.last_sync_seq, |e| e.seq);

            let client = match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "replication: failed to build client for peer {}: {e}",
                        peer.id
                    );
                    continue;
                }
            };

            let sync_url = format!("{}/replication/sync", peer.peer_url.trim_end_matches('/'));
            match client.post(&sync_url).json(&entries).send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(
                        "replication: synced {} entries to peer {} (seq {}-{})",
                        entries.len(),
                        peer.id,
                        peer.last_sync_seq,
                        last_seq
                    );
                    let store = hub.storage.write().await;
                    if let Err(e) = store.update_peer_sync_seq(peer.id, last_seq) {
                        tracing::warn!("Failed to update peer sync sequence: {}", e);
                    }
                }
                Ok(resp) => {
                    tracing::warn!(
                        "replication: sync to peer {} returned {}",
                        peer.id,
                        resp.status()
                    );
                }
                Err(e) => {
                    tracing::warn!("replication: failed to sync to peer {}: {e}", peer.id);
                }
            }
        }
    }
}

#[derive(serde::Deserialize)]
pub struct CreateWebhookRequest {
    pub url: String,
    pub events: Vec<String>,
    pub secret: Option<String>,
}

pub async fn create_webhook_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(repo_id): Path<String>,
    Json(req): Json<CreateWebhookRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(serde_json::json!({"success": false, "error": "unauthorized"})),
        );
    }
    if req.events.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": "events must not be empty"})),
        );
    }
    let random_bytes: [u8; 16] = rand::random();
    let id = format!("wh_{}", hex::encode(random_bytes));
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let webhook = Webhook {
        id: id.clone(),
        repo_id: repo_id.clone(),
        url: req.url,
        events: req.events,
        secret: req.secret,
        created_at,
        active: true,
    };
    let store = hub.storage.write().await;
    match store.create_webhook(&webhook) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "id": id})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn list_webhooks_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(repo_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(serde_json::json!({"success": false, "error": "unauthorized"})),
        );
    }
    let store = hub.storage.read().await;
    match store.list_webhooks(&repo_id) {
        Ok(hooks) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "webhooks": hooks})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn delete_webhook_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path((_repo_id, id)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (
            status,
            Json(serde_json::json!({"success": false, "error": "unauthorized"})),
        );
    }
    let store = hub.storage.write().await;
    match store.delete_webhook(&id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

// === SSO / OIDC Handlers ===

/// List all configured OIDC providers.
pub async fn sso_list_providers_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"error": "unauthorized"})));
    }
    let store = hub.storage.read().await;
    match store.list_oidc_configs() {
        Ok(configs) => {
            let names: Vec<&str> = configs.iter().map(|c| c.provider_name.as_str()).collect();
            (StatusCode::OK, Json(serde_json::json!({"providers": names})))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Configure (create or update) an OIDC provider.
pub async fn sso_configure_provider_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(config): Json<crate::sso::OidcConfig>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"error": "unauthorized"})));
    }
    // Validate required fields
    if config.provider_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "provider_name is required"})),
        );
    }
    if config.issuer_url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "issuer_url is required"})),
        );
    }
    if config.client_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "client_id is required"})),
        );
    }
    let store = hub.storage.read().await;
    match store.set_oidc_config(&config) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "provider": config.provider_name})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Delete an OIDC provider configuration.
pub async fn sso_delete_provider_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Path(provider_name): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"error": "unauthorized"})));
    }
    let store = hub.storage.read().await;
    match store.delete_oidc_config(&provider_name) {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("provider '{provider_name}' not found")})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Initiate an OIDC authorization flow — returns the redirect URL.
pub async fn sso_authorize_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"error": "unauthorized"})));
    }
    let provider_name = match body.get("provider") {
        Some(v) => match v.as_str() {
            Some(s) => s.to_owned(),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "provider must be a string"})),
                );
            }
        },
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "provider is required"})),
            );
        }
    };

    let store = hub.storage.read().await;
    let config = match store.get_oidc_config(&provider_name) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("provider '{provider_name}' not configured")})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    };
    drop(store);

    let state = crate::sso::generate_state();
    let nonce = crate::sso::generate_nonce();
    let url = crate::sso::authorization_url(&config, &state, &nonce);

    (StatusCode::OK, Json(serde_json::json!({
        "authorization_url": url,
        "state": state,
        "nonce": nonce,
    })))
}

/// Handle an OIDC callback — exchange the authorization code for tokens.
/// This is a placeholder that returns instructions for the client to implement.
pub async fn sso_callback_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"error": "unauthorized"})));
    }
    let provider_name = body.get("provider").and_then(|v| v.as_str()).unwrap_or("");
    let _code = body.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let _state = body.get("state").and_then(|v| v.as_str()).unwrap_or("");

    if provider_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "provider is required"})),
        );
    }
    if _code.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "code is required"})),
        );
    }

    let store = hub.storage.read().await;
    let _config = match store.get_oidc_config(provider_name) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("provider '{provider_name}' not configured")})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    };

    // TODO: In a full implementation, this would:
    // 1. Validate the `state` parameter against the stored value (CSRF protection)
    // 2. Exchange the authorization code for tokens via POST to the provider's token endpoint
    // 3. Validate the ID token (signature, nonce, audience, issuer)
    // 4. Extract user info from the ID token or userinfo endpoint
    // 5. Create or update a local user session
    // For now, return a placeholder indicating the flow is configured but token exchange
    // requires an HTTP client to the OIDC provider (e.g., reqwest).

    (StatusCode::OK, Json(serde_json::json!({
        "status": "configured",
        "message": "SSO callback received. Full token exchange requires HTTP client integration.",
    })))
}

pub async fn audit_log_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
    Query(params): Query<AuditQueryParams>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"error": "unauthorized"})));
    }
    let store = hub.storage.read().await;
    let entries = store
        .query_audit_log(
            params.actor.as_deref(),
            params.action.as_deref(),
            params.limit.unwrap_or(100),
            params.offset.unwrap_or(0),
        )
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "entries": entries,
            "count": entries.len(),
        })),
    )
}

/// Raft cluster status endpoint.
pub async fn raft_status_handler(
    State(hub): State<Arc<SutureHubServer>>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(status) = check_auth(&hub, &headers).await {
        return (status, Json(serde_json::json!({"error": "unauthorized"})));
    }

    #[cfg(feature = "raft-cluster")]
    {
        let state = hub.raft_state().await;
        let leader = hub.raft_leader().await;
        let term = {
            let raft = hub.raft_node.lock().await;
            raft.term()
        };
        let node_id = hub.raft_node_id;
        let is_leader = hub.is_leader().await;

        let response = serde_json::json!({
            "raft_enabled": true,
            "node_id": node_id,
            "state": state,
            "term": term,
            "leader": leader,
            "is_leader": is_leader,
        });
        (StatusCode::OK, Json(response))
    }

    #[cfg(not(feature = "raft-cluster"))]
    {
        let response = serde_json::json!({
            "raft_enabled": false,
            "state": "standalone",
            "is_leader": true,
        });
        (StatusCode::OK, Json(response))
    }
}

pub async fn run_server(
    hub: SutureHubServer,
    addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let hub = Arc::new(hub);

    {
        let hub_clone = Arc::clone(&hub);
        tokio::spawn(async move {
            replication_background_task(hub_clone).await;
        });
    }

    let (set_request_id, propagate_request_id) = request_id_layer();
    let app = axum::Router::new()
        .route("/healthz", get(health_check))
        .route("/", axum::routing::get(serve_index))
        .route("/push", axum::routing::post(push_handler))
        .route(
            "/push/compressed",
            axum::routing::post(push_compressed_handler),
        )
        .route("/pull", axum::routing::post(pull_handler))
        .route(
            "/pull/compressed",
            axum::routing::post(pull_compressed_handler),
        )
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
        .route("/handshake", axum::routing::get(handshake_get_handler))
        .route("/handshake", axum::routing::post(handshake_handler))
        .route("/v2/handshake", axum::routing::get(handshake_v2_handler))
        .route("/v2/handshake", axum::routing::post(handshake_v2_handler))
        .route("/v2/pull", axum::routing::post(v2_pull_handler))
        .route("/v2/push", axum::routing::post(v2_push_handler))
        .route("/auth/token", axum::routing::post(create_token_handler))
        .route("/auth/verify", axum::routing::post(verify_token_handler))
        .route("/mirror/setup", axum::routing::post(mirror_setup_handler))
        .route("/mirror/sync", axum::routing::post(mirror_sync_handler))
        .route(
            "/mirror/status",
            axum::routing::get(mirror_status_get_handler),
        )
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
        .route(
            "/replication/peers/{id}",
            axum::routing::delete(remove_peer_handler),
        )
        .route(
            "/replication/status",
            axum::routing::get(replication_status_handler),
        )
        .route(
            "/replication/sync",
            axum::routing::post(replication_sync_handler),
        )
        .route("/repos", axum::routing::post(create_repo_handler))
        .route(
            "/repos/{repo_id}",
            axum::routing::delete(delete_repo_handler),
        )
        .route(
            "/repos/{repo_id}/branches",
            axum::routing::post(create_branch_handler),
        )
        .route(
            "/repos/{repo_id}/branches/{branch}",
            axum::routing::delete(delete_branch_handler),
        )
        .route(
            "/repos/{repo_id}/blobs/{hash}",
            axum::routing::get(get_blob_handler),
        )
        .route(
            "/repos/{repo_id}/tree/{branch}",
            axum::routing::get(repo_tree_handler),
        )
        .route("/auth/login", axum::routing::post(login_handler))
        .route("/search", axum::routing::get(search_handler))
        .route("/activity", axum::routing::get(activity_handler))
        .route(
            "/mirrors/{id}",
            axum::routing::delete(delete_mirror_handler),
        )
        .route(
            "/webhooks/{repo_id}",
            axum::routing::post(create_webhook_handler),
        )
        .route(
            "/webhooks/{repo_id}",
            axum::routing::get(list_webhooks_handler),
        )
        .route(
            "/webhooks/{repo_id}/{id}",
            axum::routing::delete(delete_webhook_handler),
        )
        .route(
            "/repos/{repo_id}/patches/batch",
            axum::routing::post(batch_push_handler),
        )
        .route("/lfs/batch", axum::routing::post(lfs_batch_handler))
        .route(
            "/lfs/objects/{repo_id}/{oid}",
            axum::routing::put(lfs_upload_handler),
        )
        .route(
            "/lfs/objects/{repo_id}/{oid}",
            axum::routing::get(lfs_download_handler),
        )
        // SSO / OIDC routes
        .route("/sso/providers", axum::routing::get(sso_list_providers_handler))
        .route(
            "/sso/providers",
            axum::routing::post(sso_configure_provider_handler),
        )
        .route(
            "/sso/providers/{provider_name}",
            axum::routing::delete(sso_delete_provider_handler),
        )
        .route("/sso/authorize", axum::routing::post(sso_authorize_handler))
        .route("/sso/callback", axum::routing::post(sso_callback_handler))
        .route("/audit/log", axum::routing::get(audit_log_handler))
        // Raft cluster endpoints (only available with raft-cluster feature)
        .route("/raft/status", axum::routing::get(raft_status_handler))
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&hub),
            crate::audit::audit_middleware,
        ))
        .with_state(Arc::clone(&hub))
        .layer(set_request_id)
        .layer(propagate_request_id);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Suture Hub listening on {addr}");

    let shutdown_tx = tokio::sync::broadcast::channel::<()>(1).0;
    let shutdown_tx_ctrlc = shutdown_tx.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("received ctrl-c, initiating graceful shutdown");
        let _ = shutdown_tx_ctrlc.send(());
    });

    // Spawn Raft tick loop when raft-cluster feature is enabled
    #[cfg(feature = "raft-cluster")]
    {
        let raft_node = Arc::clone(&hub.raft_node);
        let hub_for_raft = Arc::clone(&hub);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_millis(10), // tick every 10ms
            );
            loop {
                interval.tick().await;
                let messages = {
                    let mut raft = raft_node.lock().await;
                    raft.tick()
                };
                // In a real multi-node deployment, these messages would be sent
                // to peers via HTTP. For now, we log state transitions.
                if !messages.is_empty() {
                    tracing::trace!(
                        "[raft] tick produced {} messages",
                        messages.len()
                    );
                }
                // Apply committed entries to replication log
                let entries = hub_for_raft.raft_committed_entries().await;
                if !entries.is_empty() {
                    let count = entries.len();
                    let store = hub_for_raft.storage.read().await;
                    for entry in &entries {
                        // Deserialize the command: expected format is JSON
                        // {"operation": "insert|update|delete", "table": "...", "data": "..."}
                        if let Ok(cmd) = serde_json::from_slice::<serde_json::Value>(&entry.command) {
                            let op = cmd.get("operation").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let table = cmd.get("table").and_then(|v| v.as_str()).unwrap_or("");
                            let data = cmd.get("data").and_then(|v| v.as_str()).unwrap_or("");
                            let _ = store.log_operation(op, table, &entry.index.to_string(), Some(data));
                        }
                    }
                    drop(store);
                    hub_for_raft.raft_advance_applied(count).await;
                }
            }
        });
    }

    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let mut rx = shutdown_tx.subscribe();
        let _ = rx.recv().await;
        hub.shutdown();
    });

    server.await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;
    async fn create_test_user_hub(
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

    fn make_auth_header_val(token: &str) -> String {
        format!("Bearer {}", token)
    }

    fn make_hash_proto(hex: &str) -> HashProto {
        HashProto {
            value: hex.to_string(),
        }
    }

    fn make_patch(hex: &str, op: &str, parents: &[String], author: &str) -> PatchProto {
        PatchProto {
            id: make_hash_proto(hex),
            operation_type: op.to_string(),
            touch_set: vec!["f".to_string()],
            target_path: Some("f".to_string()),
            payload: String::new(),
            parent_ids: parents.iter().map(|p| make_hash_proto(p)).collect(),
            author: author.to_string(),
            message: format!("patch {}", hex),
            timestamp: 0,
        }
    }

    fn make_branch(name: &str, target: &str) -> BranchProto {
        BranchProto {
            name: name.to_string(),
            target_id: make_hash_proto(target),
        }
    }

    async fn start_test_hub() -> (Arc<SutureHubServer>, u16, String) {
        let hub = Arc::new(SutureHubServer::new_in_memory());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base = format!("http://127.0.0.1:{}", port);

        let app = axum::Router::new()
            .route("/", axum::routing::get(serve_index))
            .route("/push", axum::routing::post(push_handler))
            .route(
                "/push/compressed",
                axum::routing::post(push_compressed_handler),
            )
            .route("/pull", axum::routing::post(pull_handler))
            .route(
                "/pull/compressed",
                axum::routing::post(pull_compressed_handler),
            )
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
            .route("/handshake", axum::routing::get(handshake_get_handler))
            .route("/handshake", axum::routing::post(handshake_handler))
            .route("/v2/handshake", axum::routing::get(handshake_v2_handler))
            .route("/v2/handshake", axum::routing::post(handshake_v2_handler))
            .route("/v2/pull", axum::routing::post(v2_pull_handler))
            .route("/v2/push", axum::routing::post(v2_push_handler))
            .route("/auth/token", axum::routing::post(create_token_handler))
            .route("/auth/verify", axum::routing::post(verify_token_handler))
            .route("/mirror/setup", axum::routing::post(mirror_setup_handler))
            .route("/mirror/sync", axum::routing::post(mirror_sync_handler))
            .route(
                "/mirror/status",
                axum::routing::get(mirror_status_get_handler),
            )
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
            .route(
                "/replication/peers/{id}",
                axum::routing::delete(remove_peer_handler),
            )
            .route(
                "/replication/status",
                axum::routing::get(replication_status_handler),
            )
            .route(
                "/replication/sync",
                axum::routing::post(replication_sync_handler),
            )
            .route("/repos", axum::routing::post(create_repo_handler))
            .route(
                "/repos/{repo_id}",
                axum::routing::delete(delete_repo_handler),
            )
            .route(
                "/repos/{repo_id}/branches",
                axum::routing::post(create_branch_handler),
            )
            .route(
                "/repos/{repo_id}/branches/{branch}",
                axum::routing::delete(delete_branch_handler),
            )
            .route(
                "/repos/{repo_id}/blobs/{hash}",
                axum::routing::get(get_blob_handler),
            )
            .route(
                "/repos/{repo_id}/tree/{branch}",
                axum::routing::get(repo_tree_handler),
            )
            .route("/auth/login", axum::routing::post(login_handler))
            .route("/search", axum::routing::get(search_handler))
            .route("/activity", axum::routing::get(activity_handler))
            .route(
                "/mirrors/{id}",
                axum::routing::delete(delete_mirror_handler),
            )
            .route(
                "/webhooks/{repo_id}",
                axum::routing::post(create_webhook_handler),
            )
            .route(
                "/webhooks/{repo_id}",
                axum::routing::get(list_webhooks_handler),
            )
            .route(
                "/webhooks/{repo_id}/{id}",
                axum::routing::delete(delete_webhook_handler),
            )
            .route(
                "/repos/{repo_id}/patches/batch",
                axum::routing::post(batch_push_handler),
            )
            .with_state(Arc::clone(&hub));

        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        });

        for _ in 0..50 {
            if reqwest::Client::new()
                .get(format!("{}/repos", &base))
                .send()
                .await
                .is_ok()
            {
                return (hub, port, base);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("test server did not start in time");
    }

    async fn start_test_hub_with_lfs(
        hub: Arc<SutureHubServer>,
    ) -> (Arc<SutureHubServer>, u16, String) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base = format!("http://127.0.0.1:{}", port);

        let app = axum::Router::new()
            .route("/", axum::routing::get(serve_index))
            .route("/push", axum::routing::post(push_handler))
            .route(
                "/push/compressed",
                axum::routing::post(push_compressed_handler),
            )
            .route("/pull", axum::routing::post(pull_handler))
            .route(
                "/pull/compressed",
                axum::routing::post(pull_compressed_handler),
            )
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
            .route("/handshake", axum::routing::get(handshake_get_handler))
            .route("/handshake", axum::routing::post(handshake_handler))
            .route("/v2/handshake", axum::routing::get(handshake_v2_handler))
            .route("/v2/handshake", axum::routing::post(handshake_v2_handler))
            .route("/v2/pull", axum::routing::post(v2_pull_handler))
            .route("/v2/push", axum::routing::post(v2_push_handler))
            .route("/auth/token", axum::routing::post(create_token_handler))
            .route("/auth/verify", axum::routing::post(verify_token_handler))
            .route("/mirror/setup", axum::routing::post(mirror_setup_handler))
            .route("/mirror/sync", axum::routing::post(mirror_sync_handler))
            .route(
                "/mirror/status",
                axum::routing::get(mirror_status_get_handler),
            )
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
            .route(
                "/replication/peers/{id}",
                axum::routing::delete(remove_peer_handler),
            )
            .route(
                "/replication/status",
                axum::routing::get(replication_status_handler),
            )
            .route(
                "/replication/sync",
                axum::routing::post(replication_sync_handler),
            )
            .route("/repos", axum::routing::post(create_repo_handler))
            .route(
                "/repos/{repo_id}",
                axum::routing::delete(delete_repo_handler),
            )
            .route(
                "/repos/{repo_id}/branches",
                axum::routing::post(create_branch_handler),
            )
            .route(
                "/repos/{repo_id}/branches/{branch}",
                axum::routing::delete(delete_branch_handler),
            )
            .route(
                "/repos/{repo_id}/blobs/{hash}",
                axum::routing::get(get_blob_handler),
            )
            .route(
                "/repos/{repo_id}/tree/{branch}",
                axum::routing::get(repo_tree_handler),
            )
            .route("/auth/login", axum::routing::post(login_handler))
            .route("/search", axum::routing::get(search_handler))
            .route("/activity", axum::routing::get(activity_handler))
            .route(
                "/mirrors/{id}",
                axum::routing::delete(delete_mirror_handler),
            )
            .route(
                "/webhooks/{repo_id}",
                axum::routing::post(create_webhook_handler),
            )
            .route(
                "/webhooks/{repo_id}",
                axum::routing::get(list_webhooks_handler),
            )
            .route(
                "/webhooks/{repo_id}/{id}",
                axum::routing::delete(delete_webhook_handler),
            )
            .route(
                "/repos/{repo_id}/patches/batch",
                axum::routing::post(batch_push_handler),
            )
            .route("/lfs/batch", axum::routing::post(lfs_batch_handler))
            .route(
                "/lfs/objects/{repo_id}/{oid}",
                axum::routing::put(lfs_upload_handler),
            )
            .route(
                "/lfs/objects/{repo_id}/{oid}",
                axum::routing::get(lfs_download_handler),
            )
            .with_state(Arc::clone(&hub));

        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        });

        for _ in 0..50 {
            if reqwest::Client::new()
                .get(format!("{}/repos", &base))
                .send()
                .await
                .is_ok()
            {
                return (hub, port, base);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("test server did not start in time");
    }

    async fn start_test_hub_auth() -> (Arc<SutureHubServer>, u16, String) {
        let (hub, port, base) = start_test_hub().await;
        // Pre-create an admin user for auth tests
        create_test_user_hub(&hub, "test-admin", "Test Admin", "admin").await;
        (hub, port, base)
    }

    fn post_json(
        client: &reqwest::Client,
        url: &str,
        body: &serde_json::Value,
    ) -> reqwest::RequestBuilder {
        client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
    }

    fn patch_json(
        client: &reqwest::Client,
        url: &str,
        body: &serde_json::Value,
    ) -> reqwest::RequestBuilder {
        client
            .patch(url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
    }

    #[tokio::test]
    async fn test_http_index() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let resp = client.get(format!("{}/", &base)).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.text().await.unwrap();
        assert!(body.contains("Suture Hub"));
    }

    #[tokio::test]
    async fn test_http_handshake() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/handshake", &base))
            .json(&serde_json::json!({"client_version": 1, "client_name": "test"}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["server_version"], 1);
        assert_eq!(data["compatible"], true);
    }

    #[tokio::test]
    async fn test_http_repos_empty_and_populated() {
        let (hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let resp = client.get(format!("{}/repos", &base)).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["repo_ids"].as_array().unwrap().len(), 0);

        let a_hex = "a".repeat(64);
        let push_body = serde_json::json!({
            "repo_id": "http-repo",
            "patches": [{
                "id": {"value": &a_hex},
                "operation_type": "Create",
                "touch_set": ["f"],
                "target_path": "f",
                "payload": "",
                "parent_ids": [],
                "author": "alice",
                "message": "p",
                "timestamp": 0
            }],
            "branches": [],
            "blobs": []
        });
        let push_resp = client
            .post(format!("{}/push", &base))
            .json(&push_body)
            .send()
            .await
            .unwrap();
        assert_eq!(push_resp.status(), 200);

        let resp2 = client.get(format!("{}/repos", &base)).send().await.unwrap();
        let data2: serde_json::Value = resp2.json().await.unwrap();
        assert_eq!(data2["repo_ids"].as_array().unwrap().len(), 1);

        drop(hub);
    }

    #[tokio::test]
    async fn test_http_repo_info() {
        let (hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("{}/repo/nonexistent", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);

        let a_hex = "a".repeat(64);
        hub.handle_push(PushRequest {
            repo_id: "info-repo".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        })
        .await
        .unwrap();

        let resp2 = client
            .get(format!("{}/repo/info-repo", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp2.status(), 200);
        let data: serde_json::Value = resp2.json().await.unwrap();
        assert_eq!(data["patch_count"], 1);
        assert_eq!(data["branches"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_http_repo_branches() {
        let (hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let a_hex = "a".repeat(64);

        hub.handle_push(PushRequest {
            repo_id: "branch-repo".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex), make_branch("dev", &a_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        })
        .await
        .unwrap();

        let resp = client
            .get(format!("{}/repos/branch-repo/branches", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_http_repo_patches() {
        let (hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        for i in 0..3u32 {
            let hex = format!("{:064x}", i);
            let parents: Vec<String> = if i > 0 {
                vec![format!("{:064x}", i - 1)]
            } else {
                vec![]
            };
            hub.handle_push(PushRequest {
                repo_id: "patch-repo".to_string(),
                patches: vec![PatchProto {
                    id: make_hash_proto(&hex),
                    operation_type: "Create".to_string(),
                    touch_set: vec![format!("f{i}")],
                    target_path: Some(format!("f{i}")),
                    payload: String::new(),
                    parent_ids: parents.iter().map(|p| make_hash_proto(p)).collect(),
                    author: "alice".to_string(),
                    message: format!("p{i}"),
                    timestamp: 0,
                }],
                branches: vec![],
                blobs: vec![],
                signature: None,
                known_branches: None,
                force: false,
            })
            .await
            .unwrap();
        }

        let resp = client
            .get(format!(
                "{}/repos/patch-repo/patches?offset=1&limit=1",
                &base
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["patches"].as_array().unwrap().len(), 1);
        assert!(!data["next_cursor"].as_str().unwrap().is_empty());

        let resp2 = client
            .get(format!(
                "{}/repos/patch-repo/patches?limit=1&cursor={}",
                &base,
                data["next_cursor"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(resp2.status(), 200);
        let data2: serde_json::Value = resp2.json().await.unwrap();
        assert_eq!(data2["patches"].as_array().unwrap().len(), 1);

        let resp3 = client
            .get(format!("{}/repos/patch-repo/patches?limit=50", &base,))
            .send()
            .await
            .unwrap();
        assert_eq!(resp3.status(), 200);
        let data3: serde_json::Value = resp3.json().await.unwrap();
        assert_eq!(data3["patches"].as_array().unwrap().len(), 3);
        assert!(data3["next_cursor"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_http_push_pull_roundtrip() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let a_hex = "a".repeat(64);
        let b_hex = "b".repeat(64);

        let push_body = serde_json::json!({
            "repo_id": "roundtrip-repo",
            "patches": [
                {
                    "id": {"value": &a_hex},
                    "operation_type": "Create",
                    "touch_set": ["file_a"],
                    "target_path": "file_a",
                    "payload": "",
                    "parent_ids": [],
                    "author": "alice",
                    "message": "first patch",
                    "timestamp": 100
                },
                {
                    "id": {"value": &b_hex},
                    "operation_type": "Modify",
                    "touch_set": ["file_a"],
                    "target_path": "file_a",
                    "payload": "",
                    "parent_ids": [{"value": &a_hex}],
                    "author": "bob",
                    "message": "second patch",
                    "timestamp": 200
                }
            ],
            "branches": [{"name": "main", "target_id": {"value": &b_hex}}],
            "blobs": []
        });

        let push_resp = client
            .post(format!("{}/push", &base))
            .json(&push_body)
            .send()
            .await
            .unwrap();
        assert_eq!(push_resp.status(), 200);
        let push_data: serde_json::Value = push_resp.json().await.unwrap();
        assert_eq!(push_data["success"], true);

        let pull_resp = client
            .post(format!("{}/pull", &base))
            .json(&serde_json::json!({
                "repo_id": "roundtrip-repo",
                "known_branches": [],
                "max_depth": null
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(pull_resp.status(), 200);
        let pull_data: serde_json::Value = pull_resp.json().await.unwrap();
        assert_eq!(pull_data["success"], true);
        assert_eq!(pull_data["patches"].as_array().unwrap().len(), 2);
        assert_eq!(pull_data["branches"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_http_push_compressed() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let a_hex = "a".repeat(64);
        let blob_data = b"compressed test data";
        let blob_hash = "cafebabe".repeat(8);
        let compressed = suture_protocol::compress(blob_data).unwrap();

        let push_body = serde_json::json!({
            "repo_id": "comp-repo",
            "patches": [{
                "id": {"value": &a_hex},
                "operation_type": "Create",
                "touch_set": ["f"],
                "target_path": "f",
                "payload": &blob_hash,
                "parent_ids": [],
                "author": "alice",
                "message": "p",
                "timestamp": 0
            }],
            "branches": [{"name": "main", "target_id": {"value": &a_hex}}],
            "blobs": [{"hash": {"value": &blob_hash}, "data": base64_encode(&compressed)}]
        });

        let resp = client
            .post(format!("{}/push/compressed", &base))
            .json(&push_body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);
    }

    #[tokio::test]
    async fn test_http_v2_handshake() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v2/handshake", &base))
            .json(&serde_json::json!({
                "client_version": 2,
                "client_name": "test-v2",
                "capabilities": {
                    "supports_delta": true,
                    "supports_compression": false,
                    "max_blob_size": 0
                }
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["server_version"], 2);
        assert_eq!(data["compatible"], true);
        assert_eq!(data["server_capabilities"]["supports_delta"], true);
    }

    #[tokio::test]
    async fn test_http_v2_push_pull() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let a_hex = "a".repeat(64);
        let f_hash = blake3::hash(b"f").to_hex().to_string();

        let push_body = serde_json::json!({
            "repo_id": "v2-repo",
            "patches": [{
                "id": {"value": &a_hex},
                "operation_type": "Create",
                "touch_set": ["f"],
                "target_path": "f",
                "payload": "hello world",
                "parent_ids": [],
                "author": "alice",
                "message": "v2 patch",
                "timestamp": 0
            }],
            "branches": [{"name": "main", "target_id": {"value": &a_hex}}],
            "blobs": [{"hash": {"value": &f_hash}, "data": "aGVsbG8gd29ybGQ="}],
            "deltas": [],
            "signature": null,
            "known_branches": null,
            "force": false
        });

        let push_resp = post_json(&client, &format!("{}/v2/push", &base), &push_body)
            .send()
            .await
            .unwrap();
        assert_eq!(
            push_resp.status(),
            200,
            "V2 push failed: {}",
            push_resp.status()
        );

        let pull_body = serde_json::json!({
            "repo_id": "v2-repo",
            "known_branches": [],
            "max_depth": null,
            "known_blob_hashes": [],
            "capabilities": {
                "supports_delta": false,
                "supports_compression": false,
                "max_blob_size": 0
            }
        });

        let pull_resp = post_json(&client, &format!("{}/v2/pull", &base), &pull_body)
            .send()
            .await
            .unwrap();
        assert_eq!(pull_resp.status(), 200);
        let pull_data: serde_json::Value = pull_resp.json().await.unwrap();
        assert_eq!(pull_data["success"], true);
        assert_eq!(pull_data["patches"].as_array().unwrap().len(), 1);
        assert_eq!(pull_data["protocol_version"], 2);
    }

    #[tokio::test]
    async fn test_http_auth_token_bootstrap() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/auth/token", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert!(!data["token"].as_str().unwrap().is_empty());
        assert!(data["created_at"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_http_auth_verify() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let token_resp = client
            .post(format!("{}/auth/token", &base))
            .send()
            .await
            .unwrap();
        let token_data: serde_json::Value = token_resp.json().await.unwrap();
        let token = token_data["token"].as_str().unwrap().to_string();

        let verify_resp = client
            .post(format!("{}/auth/verify", &base))
            .json(&serde_json::json!({
                "method": {"Token": &token},
                "timestamp": 0
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(verify_resp.status(), 200);
        let verify_data: serde_json::Value = verify_resp.json().await.unwrap();
        assert_eq!(verify_data["valid"], true);

        let bad_resp = client
            .post(format!("{}/auth/verify", &base))
            .json(&serde_json::json!({
                "method": {"Token": "invalid-token-xyz"},
                "timestamp": 0
            }))
            .send()
            .await
            .unwrap();
        let bad_data: serde_json::Value = bad_resp.json().await.unwrap();
        assert_eq!(bad_data["valid"], false);
    }

    #[tokio::test]
    async fn test_http_auth_register() {
        let (hub, _port, base) = start_test_hub_auth().await;
        let client = reqwest::Client::new();
        let admin_token = create_test_user_hub(&hub, "http-admin", "HTTP Admin", "admin").await;

        let resp = post_json(
            &client,
            &format!("{}/auth/register", &base),
            &serde_json::json!({
                "username": "new-http-user",
                "display_name": "New HTTP User",
                "role": "member"
            }),
        )
        .header("Authorization", make_auth_header_val(&admin_token))
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), 201);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);
        assert_eq!(data["user"]["username"], "new-http-user");
    }

    #[tokio::test]
    async fn test_http_users_list() {
        let (hub, _port, base) = start_test_hub_auth().await;
        let client = reqwest::Client::new();
        let admin_token = create_test_user_hub(&hub, "ul-admin", "UL Admin", "admin").await;

        let resp = client
            .get(format!("{}/users", &base))
            .header("Authorization", make_auth_header_val(&admin_token))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);
        assert!(!data["users"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_http_user_crud() {
        let (hub, _port, base) = start_test_hub_auth().await;
        let client = reqwest::Client::new();
        let admin_token = create_test_user_hub(&hub, "crud-admin", "CRUD Admin", "admin").await;
        create_test_user_hub(&hub, "crud-target", "CRUD Target", "reader").await;
        let auth = make_auth_header_val(&admin_token);

        let get_resp = client
            .get(format!("{}/users/crud-target", &base))
            .header("Authorization", &auth)
            .send()
            .await
            .unwrap();
        assert_eq!(get_resp.status(), 200);
        let get_data: serde_json::Value = get_resp.json().await.unwrap();
        assert_eq!(get_data["user"]["username"], "crud-target");
        assert_eq!(get_data["user"]["role"], "reader");

        let patch_resp = patch_json(
            &client,
            &format!("{}/users/crud-target/role", &base),
            &serde_json::json!({"role": "admin"}),
        )
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
        assert_eq!(patch_resp.status(), 200);
        let patch_data: serde_json::Value = patch_resp.json().await.unwrap();
        assert_eq!(patch_data["success"], true);

        let del_resp = client
            .delete(format!("{}/users/crud-target", &base))
            .header("Authorization", &auth)
            .send()
            .await
            .unwrap();
        assert_eq!(del_resp.status(), 200);
        let del_data: serde_json::Value = del_resp.json().await.unwrap();
        assert_eq!(del_data["success"], true);
    }

    #[tokio::test]
    async fn test_http_mirror_setup_and_status() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let setup_resp = post_json(
            &client,
            &format!("{}/mirror/setup", &base),
            &serde_json::json!({
                "repo_name": "mirrored",
                "upstream_url": "http://example.com",
                "upstream_repo": "upstream/repo"
            }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(setup_resp.status(), 200);
        let setup_data: serde_json::Value = setup_resp.json().await.unwrap();
        assert_eq!(setup_data["success"], true);

        let status_resp = post_json(
            &client,
            &format!("{}/mirror/status", &base),
            &serde_json::json!({}),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(status_resp.status(), 200);
        let status_data: serde_json::Value = status_resp.json().await.unwrap();
        assert_eq!(status_data["mirrors"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_http_replication() {
        let (hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        hub.set_replication_role("leader");

        let add_resp = client
            .post(format!("{}/replication/peers", &base))
            .json(&serde_json::json!({
                "peer_url": "http://follower1:8080",
                "role": "follower"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(add_resp.status(), 200);
        let add_data: serde_json::Value = add_resp.json().await.unwrap();
        assert_eq!(add_data["success"], true);

        let list_resp = client
            .get(format!("{}/replication/peers", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(list_resp.status(), 200);
        let list_data: serde_json::Value = list_resp.json().await.unwrap();
        assert_eq!(list_data["peers"].as_array().unwrap().len(), 1);

        let status_resp = client
            .get(format!("{}/replication/status", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(status_resp.status(), 200);
        let status_data: serde_json::Value = status_resp.json().await.unwrap();
        assert_eq!(status_data["status"]["peer_count"], 1);
    }

    #[tokio::test]
    async fn test_http_branch_protection() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let protect_resp = client
            .post(format!("{}/repos/prot-repo/protect/main", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(protect_resp.status(), 200);
        let protect_data: serde_json::Value = protect_resp.json().await.unwrap();
        assert_eq!(protect_data["success"], true);

        let unprotect_resp = client
            .post(format!("{}/repos/prot-repo/unprotect/main", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(unprotect_resp.status(), 200);
        let unprotect_data: serde_json::Value = unprotect_resp.json().await.unwrap();
        assert_eq!(unprotect_data["success"], true);
    }

    #[tokio::test]
    async fn test_http_404_unknown_route() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("{}/nonexistent", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    // === v1.3 new route tests ===

    #[tokio::test]
    async fn test_http_create_repo() {
        let (hub, _port, base) = start_test_hub_auth().await;
        let client = reqwest::Client::new();
        let admin_token = create_test_user_hub(&hub, "repo-admin", "Repo Admin", "admin").await;

        // Store a token in the tokens table to activate auth enforcement
        {
            let store = hub.storage.write().await;
            store
                .store_token(&admin_token, 1000, "test token", i64::MAX)
                .unwrap();
        }

        // Create a repo via POST (authenticated)
        let resp = post_json(
            &client,
            &format!("{}/repos", &base),
            &serde_json::json!({
                "repo_id": "new-repo"
            }),
        )
        .header("Authorization", make_auth_header_val(&admin_token))
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), 201);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);
        assert_eq!(data["repo_id"], "new-repo");

        // Verify it shows up in list
        let list_resp = client.get(format!("{}/repos", &base)).send().await.unwrap();
        let list_data: serde_json::Value = list_resp.json().await.unwrap();
        assert!(
            list_data["repo_ids"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("new-repo"))
        );

        // Creating duplicate should still succeed (idempotent)
        let resp2 = post_json(
            &client,
            &format!("{}/repos", &base),
            &serde_json::json!({
                "repo_id": "new-repo"
            }),
        )
        .header("Authorization", make_auth_header_val(&admin_token))
        .send()
        .await
        .unwrap();
        assert_eq!(resp2.status(), 201);

        // Unauthenticated should fail (now that tokens table is populated)
        let resp3 = post_json(
            &client,
            &format!("{}/repos", &base),
            &serde_json::json!({
                "repo_id": "noauth-repo"
            }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(resp3.status(), 401);
    }

    #[tokio::test]
    async fn test_http_delete_repo() {
        let (hub, _port, base) = start_test_hub_auth().await;
        let client = reqwest::Client::new();
        let admin_token = create_test_user_hub(&hub, "del-admin", "Del Admin", "admin").await;
        let a_hex = "a".repeat(64);

        // Create a repo with data
        hub.handle_push(PushRequest {
            repo_id: "delete-me".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        })
        .await
        .unwrap();

        // Verify it exists
        let list_resp = client.get(format!("{}/repos", &base)).send().await.unwrap();
        let list_data: serde_json::Value = list_resp.json().await.unwrap();
        assert!(
            list_data["repo_ids"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("delete-me"))
        );

        // Delete it
        let del_resp = client
            .delete(format!("{}/repos/delete-me", &base))
            .header("Authorization", make_auth_header_val(&admin_token))
            .send()
            .await
            .unwrap();
        assert_eq!(del_resp.status(), 200);
        let del_data: serde_json::Value = del_resp.json().await.unwrap();
        assert_eq!(del_data["success"], true);

        // Verify it's gone
        let list_resp2 = client.get(format!("{}/repos", &base)).send().await.unwrap();
        let list_data2: serde_json::Value = list_resp2.json().await.unwrap();
        assert!(
            !list_data2["repo_ids"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("delete-me"))
        );
    }

    #[tokio::test]
    async fn test_http_create_branch() {
        let (hub, _port, base) = start_test_hub_auth().await;
        let client = reqwest::Client::new();
        let admin_token = create_test_user_hub(&hub, "branch-admin", "Branch Admin", "admin").await;
        let a_hex = "a".repeat(64);

        // Create repo with initial data
        hub.handle_push(PushRequest {
            repo_id: "branch-repo-2".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        })
        .await
        .unwrap();

        // Create a new branch
        let resp = post_json(
            &client,
            &format!("{}/repos/branch-repo-2/branches", &base),
            &serde_json::json!({
                "name": "feature",
                "target": &a_hex
            }),
        )
        .header("Authorization", make_auth_header_val(&admin_token))
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), 201);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);

        // Verify both branches exist
        let br_resp = client
            .get(format!("{}/repos/branch-repo-2/branches", &base))
            .send()
            .await
            .unwrap();
        let br_data: serde_json::Value = br_resp.json().await.unwrap();
        assert_eq!(br_data.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_http_delete_branch() {
        let (hub, _port, base) = start_test_hub_auth().await;
        let client = reqwest::Client::new();
        let admin_token = create_test_user_hub(&hub, "delbr-admin", "DelBr Admin", "admin").await;
        let a_hex = "a".repeat(64);

        // Create repo with two branches
        hub.handle_push(PushRequest {
            repo_id: "delbr-repo".to_string(),
            patches: vec![make_patch(&a_hex, "Create", &[], "alice")],
            branches: vec![make_branch("main", &a_hex), make_branch("dev", &a_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        })
        .await
        .unwrap();

        // Delete dev branch
        let resp = client
            .delete(format!("{}/repos/delbr-repo/branches/dev", &base))
            .header("Authorization", make_auth_header_val(&admin_token))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);

        // Verify only main remains
        let br_resp = client
            .get(format!("{}/repos/delbr-repo/branches", &base))
            .send()
            .await
            .unwrap();
        let br_data: serde_json::Value = br_resp.json().await.unwrap();
        assert_eq!(br_data.as_array().unwrap().len(), 1);
        assert_eq!(br_data[0]["name"], "main");
    }

    #[tokio::test]
    async fn test_http_get_blob() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let a_hex = "a".repeat(64);
        let f_hash = blake3::hash(b"hello blob").to_hex().to_string();
        let blob_bytes = b"hello blob";
        let compressed = suture_protocol::compress(blob_bytes).unwrap();

        // Push with compressed blob — server decompresses before storing
        let push_body = serde_json::json!({
            "repo_id": "blob-repo",
            "patches": [{
                "id": {"value": &a_hex},
                "operation_type": "Create",
                "touch_set": ["f"],
                "target_path": "f",
                "payload": &f_hash,
                "parent_ids": [],
                "author": "alice",
                "message": "p",
                "timestamp": 0
            }],
            "branches": [{"name": "main", "target_id": {"value": &a_hex}}],
            "blobs": [{"hash": {"value": &f_hash}, "data": base64_encode(&compressed)}]
        });
        let push_resp = client
            .post(format!("{}/push/compressed", &base))
            .json(&push_body)
            .send()
            .await
            .unwrap();
        assert_eq!(push_resp.status(), 200);

        // Get blob — returns base64-encoded raw bytes (already decompressed by push handler)
        let blob_resp = client
            .get(format!("{}/repos/blob-repo/blobs/{}", &base, &f_hash))
            .send()
            .await
            .unwrap();
        assert_eq!(blob_resp.status(), 200);
        let blob_data: serde_json::Value = blob_resp.json().await.unwrap();
        assert_eq!(blob_data["success"], true);
        let decoded = base64_decode(blob_data["data"].as_str().unwrap()).unwrap();
        assert_eq!(decoded, blob_bytes);

        // Nonexistent blob
        let miss_resp = client
            .get(format!(
                "{}/repos/blob-repo/blobs/{}",
                &base,
                "0".repeat(64)
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(miss_resp.status(), 404);
    }

    #[tokio::test]
    async fn test_http_login() {
        let (hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        // Create a user and store token in BOTH users and tokens tables
        let token = create_test_user_hub(&hub, "login-user", "Login User", "member").await;
        {
            let store = hub.storage.write().await;
            store
                .store_token(&token, 1000, "login test token", i64::MAX)
                .unwrap();
        }

        // Login with the token
        let resp = post_json(
            &client,
            &format!("{}/auth/login", &base),
            &serde_json::json!({
                "username": "login-user",
                "token": &token
            }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);
        assert_eq!(data["user"]["username"], "login-user");

        // Login with invalid token
        let bad_resp = post_json(
            &client,
            &format!("{}/auth/login", &base),
            &serde_json::json!({
                "username": "login-user",
                "token": "invalid-token"
            }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(bad_resp.status(), 401);
    }

    #[tokio::test]
    async fn test_http_search() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let a_hex = "a".repeat(64);

        // Push to create searchable data
        let push_body = serde_json::json!({
            "repo_id": "search-test-repo",
            "patches": [{
                "id": {"value": &a_hex},
                "operation_type": "Create",
                "touch_set": ["README.md"],
                "target_path": "README.md",
                "payload": "",
                "parent_ids": [],
                "author": "searcher",
                "message": "initial commit for search",
                "timestamp": 0
            }],
            "branches": [],
            "blobs": []
        });
        let push_resp = client
            .post(format!("{}/push", &base))
            .json(&push_body)
            .send()
            .await
            .unwrap();
        assert_eq!(push_resp.status(), 200);

        // Search for repo by name
        let resp = client
            .get(format!("{}/search?q=search-test", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["repos"].as_array().unwrap().len(), 1);

        // Search for patches — same query must match repo name (search only
        // searches patches within repos that match the query).
        // "search" matches repo "search-test-repo", author "searcher", and message "initial commit for search"
        let resp2 = client
            .get(format!("{}/search?q=search", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp2.status(), 200);
        let data2: serde_json::Value = resp2.json().await.unwrap();
        assert!(!data2["patches"].as_array().unwrap().is_empty());

        // Empty search
        let resp3 = client
            .get(format!("{}/search?q=nonexistent_xyz", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp3.status(), 200);
        let data3: serde_json::Value = resp3.json().await.unwrap();
        assert_eq!(data3["repos"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_http_activity() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        // Activity should work even with no data
        let resp = client
            .get(format!("{}/activity", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        // entries should be an array (may be empty)
        assert!(data["entries"].is_array());
    }

    #[tokio::test]
    async fn test_http_delete_mirror() {
        let (hub, _port, base) = start_test_hub_auth().await;
        let client = reqwest::Client::new();
        let admin_token = create_test_user_hub(&hub, "mirror-admin", "Mirror Admin", "admin").await;

        // Setup a mirror
        let setup_resp = post_json(
            &client,
            &format!("{}/mirror/setup", &base),
            &serde_json::json!({
                "repo_name": "mirrored-del",
                "upstream_url": "http://example.com/del",
                "upstream_repo": "upstream/del"
            }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(setup_resp.status(), 200);
        let setup_data: serde_json::Value = setup_resp.json().await.unwrap();
        let mirror_id = setup_data["mirror_id"].as_i64().unwrap();

        // Delete the mirror
        let del_resp = client
            .delete(format!("{}/mirrors/{}", &base, mirror_id))
            .header("Authorization", make_auth_header_val(&admin_token))
            .send()
            .await
            .unwrap();
        assert_eq!(del_resp.status(), 200);
        let del_data: serde_json::Value = del_resp.json().await.unwrap();
        assert_eq!(del_data["success"], true);

        // Verify mirror is gone
        let status_resp = post_json(
            &client,
            &format!("{}/mirror/status", &base),
            &serde_json::json!({}),
        )
        .send()
        .await
        .unwrap();
        let status_data: serde_json::Value = status_resp.json().await.unwrap();
        assert_eq!(status_data["mirrors"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_http_mirror_status_get() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        // GET /mirror/status should work (no body needed)
        let resp = client
            .get(format!("{}/mirror/status", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert!(data["mirrors"].is_array());
    }

    #[tokio::test]
    async fn test_http_repo_tree() {
        let (hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();
        let a_hex = "a".repeat(64);
        let b_hex = "b".repeat(64);
        let c_hex = "c".repeat(64);
        let d_hex = "d".repeat(64);

        hub.handle_push(PushRequest {
            repo_id: "tree-repo".to_string(),
            patches: vec![
                PatchProto {
                    id: make_hash_proto(&a_hex),
                    operation_type: "Create".to_string(),
                    touch_set: vec!["src/main.rs".to_string()],
                    target_path: Some("src/main.rs".to_string()),
                    payload: "blob_aaa".to_string(),
                    parent_ids: vec![],
                    author: "alice".to_string(),
                    message: "create main".to_string(),
                    timestamp: 100,
                },
                PatchProto {
                    id: make_hash_proto(&b_hex),
                    operation_type: "Create".to_string(),
                    touch_set: vec!["src/lib.rs".to_string()],
                    target_path: Some("src/lib.rs".to_string()),
                    payload: "blob_bbb".to_string(),
                    parent_ids: vec![make_hash_proto(&a_hex)],
                    author: "alice".to_string(),
                    message: "create lib".to_string(),
                    timestamp: 200,
                },
                PatchProto {
                    id: make_hash_proto(&c_hex),
                    operation_type: "Delete".to_string(),
                    touch_set: vec!["src/main.rs".to_string()],
                    target_path: Some("src/main.rs".to_string()),
                    payload: String::new(),
                    parent_ids: vec![make_hash_proto(&b_hex)],
                    author: "alice".to_string(),
                    message: "delete main".to_string(),
                    timestamp: 300,
                },
                PatchProto {
                    id: make_hash_proto(&d_hex),
                    operation_type: "Modify".to_string(),
                    touch_set: vec!["src/lib.rs".to_string()],
                    target_path: Some("src/lib.rs".to_string()),
                    payload: "blob_ddd".to_string(),
                    parent_ids: vec![make_hash_proto(&c_hex)],
                    author: "bob".to_string(),
                    message: "modify lib".to_string(),
                    timestamp: 400,
                },
            ],
            branches: vec![make_branch("main", &d_hex)],
            blobs: vec![],
            signature: None,
            known_branches: None,
            force: false,
        })
        .await
        .unwrap();

        let resp = client
            .get(format!("{}/repos/tree-repo/tree/main", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);
        let files = data["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["path"], "src/lib.rs");
        assert_eq!(files[0]["content_hash"], "blob_ddd");
    }

    #[tokio::test]
    async fn test_http_repo_tree_empty() {
        let (_hub, _port, base) = start_test_hub().await;
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("{}/repos/nonexistent/tree/main", &base))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["success"], true);
        let files = data["files"].as_array().unwrap();
        assert_eq!(files.len(), 0);
    }

    #[tokio::test]
    async fn test_lfs_batch_upload_none() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_id = "test-repo";
        let oid = "aabbccdd";
        let prefix = &oid[..2];
        let obj_path = tmp
            .path()
            .join(repo_id)
            .join("objects")
            .join(prefix)
            .join(oid);
        std::fs::create_dir_all(obj_path.parent().unwrap()).unwrap();
        std::fs::write(&obj_path, b"existing data").unwrap();

        let hub = SutureHubServer::new_in_memory().with_lfs_dir(tmp.path().to_path_buf());
        let (_hub, _port, base) = start_test_hub_with_lfs(Arc::new(hub)).await;
        let client = reqwest::Client::new();

        let resp = post_json(
            &client,
            &format!("{}/lfs/batch", &base),
            &serde_json::json!({
                "repo_id": repo_id,
                "operation": "upload",
                "objects": [{"oid": oid, "size": 12}],
            }),
        )
        .send()
        .await
        .unwrap();

        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["objects"][0]["action"], "none");
    }

    #[tokio::test]
    async fn test_lfs_upload_download_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_id = "test-repo";

        let hub = SutureHubServer::new_in_memory().with_lfs_dir(tmp.path().to_path_buf());
        let (_hub, _port, base) = start_test_hub_with_lfs(Arc::new(hub)).await;
        let client = reqwest::Client::new();

        let payload = b"hello lfs world".repeat(1000);
        let hash = sha2::Sha256::digest(&payload);
        let oid = hex::encode(hash);

        let resp = post_json(
            &client,
            &format!("{}/lfs/batch", &base),
            &serde_json::json!({
                "repo_id": repo_id,
                "operation": "upload",
                "objects": [{"oid": &oid, "size": payload.len()}],
            }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["objects"][0]["action"], "upload");

        let resp = client
            .put(format!("{}/lfs/objects/{}/{}", &base, repo_id, &oid))
            .body(payload.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let resp = post_json(
            &client,
            &format!("{}/lfs/batch", &base),
            &serde_json::json!({
                "repo_id": repo_id,
                "operation": "download",
                "objects": [{"oid": &oid, "size": payload.len()}],
            }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["objects"][0]["action"], "download");

        let resp = client
            .get(format!("{}/lfs/objects/{}/{}", &base, repo_id, &oid))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let downloaded = resp.bytes().await.unwrap();
        assert_eq!(downloaded.as_ref(), payload.as_slice());

        let resp = client
            .put(format!(
                "{}/lfs/objects/{}/{}",
                &base, repo_id, "badbadbadbadbadbadbadbadbadbadbadbadbadbadbad"
            ))
            .body(payload.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    async fn test_lfs_batch_download_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let hub = SutureHubServer::new_in_memory().with_lfs_dir(tmp.path().to_path_buf());
        let (_hub, _port, base) = start_test_hub_with_lfs(Arc::new(hub)).await;
        let client = reqwest::Client::new();

        let resp = post_json(&client, &format!("{}/lfs/batch", &base), &serde_json::json!({
            "repo_id": "test-repo",
            "operation": "download",
            "objects": [{"oid": "ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00", "size": 100}],
        }))
        .send().await.unwrap();

        assert_eq!(resp.status(), 200);
        let data: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(data["objects"][0]["action"], "none");
    }

    #[tokio::test]
    async fn test_lfs_no_storage_configured() {
        let hub = SutureHubServer::new_in_memory();
        let (_hub, _port, base) = start_test_hub_with_lfs(Arc::new(hub)).await;
        let client = reqwest::Client::new();

        let resp = post_json(&client, &format!("{}/lfs/batch", &base), &serde_json::json!({
            "repo_id": "test-repo",
            "operation": "upload",
            "objects": [{"oid": "aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00", "size": 10}],
        }))
        .send().await.unwrap();
        assert_eq!(resp.status(), 503);

        let resp = client
            .put(format!("{base}/lfs/objects/test-repo/aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00"))
            .body(b"some data".to_vec())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 503);
    }

    #[cfg(feature = "s3-backend")]
    #[test]
    fn test_blob_backend_used_when_set() {
        use crate::blob_backend::BlobBackend;

        struct MockBackend {
            store_called: std::sync::atomic::AtomicBool,
            get_called: std::sync::atomic::AtomicBool,
        }

        impl MockBackend {
            fn new() -> Self {
                Self {
                    store_called: std::sync::atomic::AtomicBool::new(false),
                    get_called: std::sync::atomic::AtomicBool::new(false),
                }
            }
        }

        impl BlobBackend for MockBackend {
            fn store_blob(
                &self,
                _repo_id: &str,
                _hash_hex: &str,
                _data: &[u8],
            ) -> Result<(), String> {
                self.store_called
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(())
            }
            fn get_blob(&self, _repo_id: &str, _hash_hex: &str) -> Result<Option<Vec<u8>>, String> {
                self.get_called
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(None)
            }
            fn has_blob(&self, _repo_id: &str, _hash_hex: &str) -> Result<bool, String> {
                Ok(false)
            }
            fn delete_blob(&self, _repo_id: &str, _hash_hex: &str) -> Result<(), String> {
                Ok(())
            }
            fn list_blobs(&self, _repo_id: &str) -> Result<Vec<String>, String> {
                Ok(vec![])
            }
        }

        let mock = Arc::new(MockBackend::new());
        let mut hub = SutureHubServer::new();
        hub.set_blob_backend(mock.clone());

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = hub.storage.read().await;
            hub.blob_store(&store, "test-repo", &"a".repeat(64), b"data")
                .unwrap();
            assert!(mock.store_called.load(std::sync::atomic::Ordering::Relaxed));

            hub.blob_get(&store, "test-repo", &"a".repeat(64)).unwrap();
            assert!(mock.get_called.load(std::sync::atomic::Ordering::Relaxed));
        });
    }

    #[cfg(feature = "raft-cluster")]
    #[tokio::test]
    async fn test_apply_raft_command_create_repo() {
        use crate::raft::HubCommand;

        let hub = SutureHubServer::new_in_memory();
        hub.apply_raft_command(HubCommand::CreateRepo {
            repo_id: "raft-repo".to_string(),
        })
        .await
        .unwrap();

        let store = hub.storage.read().await;
        assert!(store.repo_exists("raft-repo").unwrap_or(false));
    }

    #[cfg(feature = "raft-cluster")]
    #[tokio::test]
    async fn test_apply_raft_command_delete_repo() {
        use crate::raft::HubCommand;

        let hub = SutureHubServer::new_in_memory();
        {
            let store = hub.storage.write().await;
            store.ensure_repo("del-repo").unwrap();
        }
        hub.apply_raft_command(HubCommand::DeleteRepo {
            repo_id: "del-repo".to_string(),
        })
        .await
        .unwrap();

        let store = hub.storage.read().await;
        assert!(!store.repo_exists("del-repo").unwrap_or(false));
    }

    #[cfg(feature = "raft-cluster")]
    #[tokio::test]
    async fn test_apply_raft_command_branch() {
        use crate::raft::HubCommand;

        let hub = SutureHubServer::new_in_memory();
        {
            let store = hub.storage.write().await;
            store.ensure_repo("br-repo").unwrap();
        }

        hub.apply_raft_command(HubCommand::CreateBranch {
            repo_id: "br-repo".to_string(),
            branch: "main".to_string(),
            target: "a".repeat(64),
        })
        .await
        .unwrap();

        hub.apply_raft_command(HubCommand::UpdateBranch {
            repo_id: "br-repo".to_string(),
            branch: "main".to_string(),
            target: "b".repeat(64),
        })
        .await
        .unwrap();

        hub.apply_raft_command(HubCommand::DeleteBranch {
            repo_id: "br-repo".to_string(),
            branch: "main".to_string(),
        })
        .await
        .unwrap();

        let store = hub.storage.read().await;
        let branches = store.get_branches("br-repo").unwrap_or_default();
        assert!(branches.is_empty());
    }
}
