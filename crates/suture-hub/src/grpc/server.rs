use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::SutureHubServer;

mod pb {
    tonic::include_proto!("suture");
}

use pb::suture_hub_server::{SutureHub, SutureHubServer as GrpcSutureHubServer};

pub struct GrpcServer {
    addr: SocketAddr,
}

impl GrpcServer {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub async fn serve(
        &self,
        hub: Arc<RwLock<SutureHubServer>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let service = SutureHubService { hub };
        tracing::info!("gRPC server listening on {}", self.addr);
        Server::builder()
            .add_service(GrpcSutureHubServer::new(service))
            .serve(self.addr)
            .await?;
        Ok(())
    }
}

struct SutureHubService {
    hub: Arc<RwLock<SutureHubServer>>,
}

#[tonic::async_trait]
impl SutureHub for SutureHubService {
    async fn handshake(
        &self,
        request: Request<pb::HandshakeRequest>,
    ) -> Result<Response<pb::HandshakeResponse>, Status> {
        let _req = request.into_inner();
        let hub = self.hub.read().await;
        Ok(Response::new(pb::HandshakeResponse {
            server_version: suture_protocol::PROTOCOL_VERSION.to_string(),
            auth_required: !hub.is_no_auth(),
            server_capabilities: vec!["push".into(), "pull".into(), "v2".into()],
        }))
    }

    async fn list_repos(
        &self,
        _request: Request<pb::ListReposRequest>,
    ) -> Result<Response<pb::ListReposResponse>, Status> {
        let hub = self.hub.read().await;
        let resp = hub.handle_list_repos().await;
        Ok(Response::new(pb::ListReposResponse {
            repos: resp.repo_ids,
        }))
    }

    async fn get_repo_info(
        &self,
        request: Request<pb::GetRepoInfoRequest>,
    ) -> Result<Response<pb::RepoInfoResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let resp = hub.handle_repo_info(&req.repo_id).await;
        if !resp.success {
            return Err(Status::not_found(resp.error.unwrap_or_default()));
        }
        Ok(Response::new(pb::RepoInfoResponse {
            repo_id: resp.repo_id,
            branch_count: resp.branches.len() as i32,
            patch_count: resp.patch_count as i32,
            created_at: 0,
        }))
    }

    async fn create_repo(
        &self,
        request: Request<pb::CreateRepoRequest>,
    ) -> Result<Response<pb::CreateRepoResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let store = hub.storage().read().await;
        match store.ensure_repo(&req.repo_id) {
            Ok(_) => Ok(Response::new(pb::CreateRepoResponse {
                success: true,
                message: String::new(),
            })),
            Err(e) => Ok(Response::new(pb::CreateRepoResponse {
                success: false,
                message: e.to_string(),
            })),
        }
    }

    async fn delete_repo(
        &self,
        request: Request<pb::DeleteRepoRequest>,
    ) -> Result<Response<pb::DeleteRepoResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let store = hub.storage().read().await;
        match store.delete_repo(&req.repo_id) {
            Ok(()) => Ok(Response::new(pb::DeleteRepoResponse { success: true })),
            Err(_) => Ok(Response::new(pb::DeleteRepoResponse { success: false })),
        }
    }

    async fn list_branches(
        &self,
        request: Request<pb::ListBranchesRequest>,
    ) -> Result<Response<pb::ListBranchesResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let store = hub.storage().read().await;
        let branches = store.get_branches(&req.repo_id).unwrap_or_default();
        Ok(Response::new(pb::ListBranchesResponse {
            branches: branches.into_iter().map(|b| b.name).collect(),
        }))
    }

    async fn create_branch(
        &self,
        request: Request<pb::CreateBranchRequest>,
    ) -> Result<Response<pb::CreateBranchResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let store = hub.storage().read().await;
        match store.set_branch(&req.repo_id, &req.branch_name, &req.branch_name) {
            Ok(()) => Ok(Response::new(pb::CreateBranchResponse { success: true })),
            Err(_) => Ok(Response::new(pb::CreateBranchResponse { success: false })),
        }
    }

    async fn delete_branch(
        &self,
        request: Request<pb::DeleteBranchRequest>,
    ) -> Result<Response<pb::DeleteBranchResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let store = hub.storage().read().await;
        match store.delete_branch(&req.repo_id, &req.branch_name) {
            Ok(()) => Ok(Response::new(pb::DeleteBranchResponse { success: true })),
            Err(_) => Ok(Response::new(pb::DeleteBranchResponse { success: false })),
        }
    }

    async fn list_patches(
        &self,
        request: Request<pb::ListPatchesRequest>,
    ) -> Result<Response<pb::ListPatchesResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let (patches, _) = hub.handle_repo_patches_cursor(&req.repo_id, 0, 200).await;
        let grpc_patches: Vec<pb::PatchInfo> = patches
            .into_iter()
            .map(|p| pb::PatchInfo {
                hash: p.id.value,
                message: p.message,
                author: p.author,
                timestamp: p.timestamp as i64,
                parents: p.parent_ids.into_iter().map(|pid| pid.value).collect(),
            })
            .collect();
        Ok(Response::new(pb::ListPatchesResponse {
            patches: grpc_patches,
        }))
    }

    async fn get_blob(
        &self,
        request: Request<pb::GetBlobRequest>,
    ) -> Result<Response<pb::BlobResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let store = hub.storage().read().await;
        match store.get_blob(&req.repo_id, &req.hash) {
            Ok(Some(data)) => Ok(Response::new(pb::BlobResponse { data })),
            Ok(None) => Err(Status::not_found("blob not found")),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn push(
        &self,
        request: Request<pb::PushRequest>,
    ) -> Result<Response<pb::PushResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let patches: Vec<crate::types::PatchProto> = req
            .patches
            .into_iter()
            .map(|p| crate::types::PatchProto {
                id: crate::types::HashProto { value: p.hash },
                operation_type: String::new(),
                touch_set: vec![],
                target_path: None,
                payload: String::new(),
                parent_ids: p
                    .parents
                    .into_iter()
                    .map(|pid| crate::types::HashProto { value: pid })
                    .collect(),
                author: p.author,
                message: p.message,
                timestamp: p.timestamp as u64,
            })
            .collect();
        let blobs: Vec<crate::types::BlobRef> = req
            .blobs
            .into_iter()
            .map(|b| {
                use base64::Engine;
                crate::types::BlobRef {
                    hash: crate::types::HashProto { value: b.hash },
                    data: base64::engine::general_purpose::STANDARD.encode(&b.data),
                }
            })
            .collect();
        let patches_len = patches.len();
        let blobs_len = blobs.len();
        let hub_push = crate::types::PushRequest {
            repo_id: req.repo_id,
            patches,
            branches: vec![],
            blobs,
            signature: None,
            known_branches: None,
            force: false,
        };
        match hub.handle_push(hub_push).await {
            Ok(resp) => Ok(Response::new(pb::PushResponse {
                success: resp.success,
                patches_stored: (patches_len - resp.existing_patches.len()) as i32,
                blobs_stored: blobs_len as i32,
            })),
            Err((_, _resp)) => Ok(Response::new(pb::PushResponse {
                success: false,
                patches_stored: 0,
                blobs_stored: 0,
            })),
        }
    }

    async fn pull(
        &self,
        request: Request<pb::PullRequest>,
    ) -> Result<Response<pb::PullResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let hub_pull = crate::types::PullRequest {
            repo_id: req.repo_id,
            known_branches: vec![],
            max_depth: None,
        };
        let resp = hub.handle_pull(hub_pull).await;
        if !resp.success {
            return Err(Status::not_found(
                resp.error.unwrap_or_else(|| "pull failed".to_string()),
            ));
        }
        let grpc_patches: Vec<pb::PatchInfo> = resp
            .patches
            .into_iter()
            .map(|p| pb::PatchInfo {
                hash: p.id.value,
                message: p.message,
                author: p.author,
                timestamp: p.timestamp as i64,
                parents: p.parent_ids.into_iter().map(|pid| pid.value).collect(),
            })
            .collect();
        let grpc_blobs: Vec<pb::BlobData> = resp
            .blobs
            .into_iter()
            .map(|b| {
                use base64::Engine;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(&b.data)
                    .unwrap_or_default();
                pb::BlobData {
                    hash: b.hash.value,
                    data,
                }
            })
            .collect();
        Ok(Response::new(pb::PullResponse {
            patches: grpc_patches,
            blobs: grpc_blobs,
            missing_blobs: vec![],
        }))
    }

    async fn get_tree(
        &self,
        request: Request<pb::GetTreeRequest>,
    ) -> Result<Response<pb::TreeResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let store = hub.storage().read().await;
        match store.get_tree_at_branch(&req.repo_id, &req.branch) {
            Ok(entries) => {
                let grpc_entries: Vec<pb::TreeEntry> = entries
                    .into_iter()
                    .map(|e| pb::TreeEntry {
                        path: e.path,
                        hash: e.content_hash,
                        is_directory: false,
                    })
                    .collect();
                Ok(Response::new(pb::TreeResponse {
                    entries: grpc_entries,
                }))
            }
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn search(
        &self,
        request: Request<pb::SearchRequest>,
    ) -> Result<Response<pb::SearchResponse>, Status> {
        let req = request.into_inner();
        let hub = self.hub.read().await;
        let store = hub.storage().read().await;
        let repos = store.search_repos(&req.query).unwrap_or_default();
        let mut results = Vec::new();
        for repo_id in &repos {
            results.push(pb::SearchResult {
                repo_id: repo_id.clone(),
                match_type: "repo".to_string(),
                snippet: format!("repository: {repo_id}"),
            });
            if let Ok(patches) = store.search_patches(repo_id, &req.query) {
                for p in patches {
                    results.push(pb::SearchResult {
                        repo_id: repo_id.clone(),
                        match_type: "patch".to_string(),
                        snippet: p.message,
                    });
                }
            }
        }
        Ok(Response::new(pb::SearchResponse { results }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_server_creation() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = GrpcServer::new(addr);
        assert_eq!(server.addr(), addr);
    }

    #[test]
    fn test_grpc_server_ephemeral_port() {
        let addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let server = GrpcServer::new(addr);
        assert_eq!(server.addr().port(), 0);
    }
}
