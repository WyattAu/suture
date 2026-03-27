use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::*;

struct RepoState {
    patches: HashMap<String, PatchProto>,
    branches: HashMap<String, String>,
    blobs: HashMap<String, Vec<u8>>,
}

pub struct SutureHubServer {
    repos: Arc<RwLock<HashMap<String, RepoState>>>,
}

impl Default for SutureHubServer {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureHubServer {
    pub fn new() -> Self {
        Self {
            repos: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn handle_push(
        &self,
        req: PushRequest,
    ) -> Result<PushResponse, (StatusCode, PushResponse)> {
        let mut repos = self.repos.write().await;
        let _repo = repos
            .entry(req.repo_id.clone())
            .or_insert_with(|| RepoState {
                patches: HashMap::new(),
                branches: HashMap::new(),
                blobs: HashMap::new(),
            });
        let repo = repos.get_mut(&req.repo_id).unwrap();

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
            repo.blobs.entry(hex).or_insert(data);
        }

        for patch in &req.patches {
            let id_hex = hash_to_hex(&patch.id);
            if let std::collections::hash_map::Entry::Vacant(e) = repo.patches.entry(id_hex) {
                e.insert(patch.clone());
            } else {
                existing_patches.push(patch.id.clone());
            }
        }

        for branch in &req.branches {
            let target_hex = hash_to_hex(&branch.target_id);
            repo.branches.insert(branch.name.clone(), target_hex);
        }

        Ok(PushResponse {
            success: true,
            error: None,
            existing_patches,
        })
    }

    pub async fn handle_pull(&self, req: PullRequest) -> PullResponse {
        let repos = self.repos.read().await;
        let Some(repo) = repos.get(&req.repo_id) else {
            return PullResponse {
                success: false,
                error: Some(format!("repo not found: {}", req.repo_id)),
                patches: vec![],
                branches: vec![],
                blobs: vec![],
            };
        };

        let client_ancestors = collect_ancestors(repo, &req.known_branches);
        let new_patches = collect_new_patches(repo, &client_ancestors);

        let branches: Vec<BranchProto> = repo
            .branches
            .iter()
            .map(|(name, target)| BranchProto {
                name: name.clone(),
                target_id: hex_to_hash(target),
            })
            .collect();

        let blobs: Vec<BlobRef> = repo
            .blobs
            .iter()
            .map(|(hex, data)| BlobRef {
                hash: hex_to_hash(hex),
                data: base64_encode(data),
            })
            .collect();

        PullResponse {
            success: true,
            error: None,
            patches: new_patches,
            branches,
            blobs,
        }
    }

    pub async fn handle_list_repos(&self) -> ListReposResponse {
        let repos = self.repos.read().await;
        ListReposResponse {
            repo_ids: repos.keys().cloned().collect(),
        }
    }

    pub async fn handle_repo_info(&self, repo_id: &str) -> RepoInfoResponse {
        let repos = self.repos.read().await;
        let Some(repo) = repos.get(repo_id) else {
            return RepoInfoResponse {
                repo_id: repo_id.to_string(),
                patch_count: 0,
                branches: vec![],
                success: false,
                error: Some(format!("repo not found: {repo_id}")),
            };
        };

        let branches: Vec<BranchProto> = repo
            .branches
            .iter()
            .map(|(name, target)| BranchProto {
                name: name.clone(),
                target_id: hex_to_hash(target),
            })
            .collect();

        RepoInfoResponse {
            repo_id: repo_id.to_string(),
            patch_count: repo.patches.len() as u64,
            branches,
            success: true,
            error: None,
        }
    }
}

fn collect_ancestors(repo: &RepoState, known_branches: &[BranchProto]) -> HashSet<String> {
    let mut ancestors = HashSet::new();
    let mut stack: Vec<String> = known_branches
        .iter()
        .filter_map(|b| {
            let hex = hash_to_hex(&b.target_id);
            if repo.patches.contains_key(&hex) {
                Some(hex)
            } else {
                None
            }
        })
        .collect();

    while let Some(id_hex) = stack.pop() {
        if ancestors.insert(id_hex.clone())
            && let Some(patch) = repo.patches.get(&id_hex)
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

fn collect_new_patches(repo: &RepoState, client_ancestors: &HashSet<String>) -> Vec<PatchProto> {
    let mut new_ids: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = repo.branches.values().cloned().collect();

    while let Some(id_hex) = stack.pop() {
        if !client_ancestors.contains(&id_hex)
            && new_ids.insert(id_hex.clone())
            && let Some(patch) = repo.patches.get(&id_hex)
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
        .filter_map(|id| repo.patches.get(&id).cloned())
        .collect();

    topological_sort(&mut result);
    result
}

fn topological_sort(patches: &mut Vec<PatchProto>) {
    let index_map: HashMap<String, usize> = patches
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
    index_map: &HashMap<String, usize>,
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

    fn make_hash_proto(hex: &str) -> HashProto {
        HashProto {
            value: hex.to_string(),
        }
    }

    fn make_patch(
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

    fn make_branch(name: &str, target: &str) -> BranchProto {
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
        };
        let resp1 = hub.handle_push(push1).await.unwrap();
        assert!(resp1.success);
        assert!(resp1.existing_patches.is_empty());

        let push2 = PushRequest {
            repo_id: "test-repo".to_string(),
            patches: vec![p1.clone()],
            branches: vec![make_branch("main", &a_hex)],
            blobs: vec![],
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
        };
        hub.handle_push(push).await.unwrap();

        let push = PushRequest {
            repo_id: "repo-2".to_string(),
            patches: vec![],
            branches: vec![],
            blobs: vec![],
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
}
