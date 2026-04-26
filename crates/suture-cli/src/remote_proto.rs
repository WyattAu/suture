use base64::Engine;
use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HashProto {
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PatchProto {
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
pub(crate) struct BranchProto {
    pub name: String,
    pub target_id: HashProto,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct BlobRef {
    pub hash: HashProto,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PushRequest {
    pub repo_id: String,
    pub patches: Vec<PatchProto>,
    pub branches: Vec<BranchProto>,
    pub blobs: Vec<BlobRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_branches: Vec<BranchProto>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PushResponse {
    pub success: bool,
    #[allow(dead_code)]
    pub error: Option<String>,
    #[allow(dead_code)]
    pub existing_patches: Vec<HashProto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PullRequest {
    pub repo_id: String,
    pub known_branches: Vec<BranchProto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PullResponse {
    pub success: bool,
    pub error: Option<String>,
    pub patches: Vec<PatchProto>,
    pub branches: Vec<BranchProto>,
    pub blobs: Vec<BlobRef>,
}

pub(crate) fn hex_to_hash_proto(hex: &str) -> HashProto {
    HashProto {
        value: hex.to_string(),
    }
}

pub(crate) fn patch_to_proto(patch: &suture_core::patch::types::Patch) -> PatchProto {
    PatchProto {
        id: hex_to_hash_proto(&patch.id.to_hex()),
        operation_type: patch.operation_type.to_string(),
        touch_set: patch.touch_set.addresses(),
        target_path: patch.target_path.clone(),
        payload: base64::engine::general_purpose::STANDARD.encode(&patch.payload),
        parent_ids: patch
            .parent_ids
            .iter()
            .map(|id| hex_to_hash_proto(&id.to_hex()))
            .collect(),
        author: patch.author.clone(),
        message: patch.message.clone(),
        timestamp: patch.timestamp,
    }
}

pub(crate) fn proto_to_patch(
    proto: &PatchProto,
) -> Result<suture_core::patch::types::Patch, Box<dyn std::error::Error>> {
    use suture_common::Hash;
    use suture_core::patch::types::{OperationType, Patch, PatchId, TouchSet};

    let id = Hash::from_hex(&proto.id.value)?;
    let parent_ids: Vec<PatchId> = proto
        .parent_ids
        .iter()
        .filter_map(|h| Hash::from_hex(&h.value).ok())
        .collect();
    let op_type = match proto.operation_type.as_str() {
        "create" => OperationType::Create,
        "delete" => OperationType::Delete,
        "modify" => OperationType::Modify,
        "move" => OperationType::Move,
        "metadata" => OperationType::Metadata,
        "merge" => OperationType::Merge,
        "identity" => OperationType::Identity,
        "batch" => OperationType::Batch,
        _ => OperationType::Modify,
    };
    let touch_set = TouchSet::from_addrs(proto.touch_set.iter().cloned());
    let payload = base64::engine::general_purpose::STANDARD.decode(&proto.payload)?;

    Ok(Patch::with_id(
        id,
        op_type,
        touch_set,
        proto.target_path.clone(),
        payload,
        parent_ids,
        proto.author.clone(),
        proto.message.clone(),
        proto.timestamp,
    ))
}

pub(crate) fn canonical_push_bytes(req: &PushRequest) -> Vec<u8> {
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

    buf.extend_from_slice(&(req.known_branches.len() as u64).to_le_bytes());
    for branch in &req.known_branches {
        buf.extend_from_slice(branch.name.as_bytes());
        buf.push(0);
        buf.extend_from_slice(branch.target_id.value.as_bytes());
        buf.push(0);
    }

    buf.push(if req.force { 1 } else { 0 });

    buf
}

pub(crate) fn sign_push_request(
    repo: &suture_core::repository::Repository,
    mut req: PushRequest,
) -> Result<PushRequest, Box<dyn std::error::Error>> {
    let key_name = match repo.get_config("signing.key")? {
        Some(name) => name,
        None => return Ok(req),
    };

    let keys_dir = std::path::Path::new(".suture").join("keys");
    // Validate key_name to prevent path traversal
    if !key_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "invalid signing key name '{}': must contain only alphanumeric characters, hyphens, and underscores",
            key_name
        )
        .into());
    }
    let key_path = keys_dir.join(format!("{key_name}.ed25519"));

    let priv_key_bytes = std::fs::read(&key_path).map_err(|e| {
        format!(
            "cannot read signing key '{}': {e}. Run `suture key generate {key_name}`",
            key_path.display()
        )
    })?;

    if priv_key_bytes.len() != 32 {
        return Err("invalid private key length (expected 32 bytes)".into());
    }

    let signing_key = ed25519_dalek::SigningKey::from_bytes(
        priv_key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "invalid key bytes")?,
    );
    let canonical = canonical_push_bytes(&req);
    let signature = signing_key.sign(&canonical);
    req.signature = Some(signature.to_bytes().to_vec());

    Ok(req)
}

pub(crate) fn get_remote_token(
    repo: &suture_core::repository::Repository,
    remote: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let key = format!("remote.{}.token", remote);
    Ok(repo.get_config(&key)?)
}

pub(crate) fn derive_repo_id(url: &str, remote_name: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let after_scheme = if let Some(idx) = trimmed.find("://") {
        &trimmed[idx + 3..]
    } else {
        trimmed
    };
    if let Some(path_start) = after_scheme.find('/') {
        let path = &after_scheme[path_start + 1..];
        if let Some(name) = path.rsplit('/').next()
            && !name.is_empty()
        {
            return name.to_string();
        }
    }
    remote_name.to_string()
}

pub(crate) async fn check_handshake(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    const PROTOCOL_VERSION: u32 = 1;

    #[derive(serde::Deserialize)]
    struct HandshakeResponse {
        server_version: u32,
        compatible: bool,
    }

    let client = reqwest::Client::new();
    let resp = client.get(format!("{}/handshake", url)).send().await?;

    if !resp.status().is_success() {
        return Err(format!("handshake failed: server returned {}", resp.status()).into());
    }

    let hs: HandshakeResponse = resp.json().await?;
    if !hs.compatible {
        return Err(format!(
            "protocol version mismatch: client={}, server={}",
            PROTOCOL_VERSION, hs.server_version
        )
        .into());
    }

    Ok(())
}

pub(crate) async fn do_fetch(
    repo: &mut suture_core::repository::Repository,
    remote: &str,
    depth: Option<u32>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let url = repo.get_remote_url(remote)?;

    check_handshake(&url).await?;

    let known_branches = repo
        .list_branches()
        .iter()
        .map(|(name, target_id)| BranchProto {
            name: name.clone(),
            target_id: hex_to_hash_proto(&target_id.to_hex()),
        })
        .collect();

    let pull_body = PullRequest {
        repo_id: derive_repo_id(&url, remote),
        known_branches,
        max_depth: depth,
    };

    let client = reqwest::Client::new();
    let mut req_builder = client.post(format!("{}/pull", url)).json(&pull_body);

    if let Some(token) = get_remote_token(repo, remote)? {
        req_builder = req_builder.bearer_auth(&token);
    }

    let resp = req_builder.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await?;
        return Err(format!("fetch failed: server returned {status} — {text}").into());
    }

    let result: PullResponse = resp.json().await?;
    if !result.success {
        let detail = result.error.as_deref().unwrap_or("unknown error");
        return Err(format!("fetch failed: {detail}").into());
    }

    if !result.blobs.is_empty() || !result.patches.is_empty() {
        eprintln!(
            "Receiving objects: {} blob(s), {} patch(es)",
            result.blobs.len(),
            result.patches.len()
        );
    }

    let b64 = base64::engine::general_purpose::STANDARD;

    for (i, blob) in result.blobs.iter().enumerate() {
        if (i + 1) % 100 == 0 || i + 1 == result.blobs.len() {
            eprint!("\r  blobs: {}/{}", i + 1, result.blobs.len());
        }
        let hash = suture_common::Hash::from_hex(&blob.hash.value)?;
        let data = b64.decode(&blob.data)?;
        repo.cas().put_blob_with_hash(&data, &hash)?;
    }
    if !result.blobs.is_empty() {
        eprintln!();
    }

    let mut new_patches = 0;
    for (i, patch_proto) in result.patches.iter().enumerate() {
        if (i + 1) % 100 == 0 || i + 1 == result.patches.len() {
            eprint!("\r  patches: {}/{}", i + 1, result.patches.len());
        }
        let patch = proto_to_patch(patch_proto)?;
        if !repo.dag().has_patch(&patch.id) {
            repo.meta().store_patch(&patch)?;
            let valid_parents: Vec<_> = patch
                .parent_ids
                .iter()
                .filter(|pid| repo.dag().has_patch(pid))
                .copied()
                .collect();
            let _ = repo.dag_mut().add_patch(patch, valid_parents)?;
            new_patches += 1;
        }
    }
    if !result.patches.is_empty() {
        eprintln!();
    }

    for branch in &result.branches {
        let target_id = suture_common::Hash::from_hex(&branch.target_id.value)?;
        let branch_name = suture_common::BranchName::new(&branch.name)?;
        let ref_key = format!("remote.{}.ref.{}", remote, branch.name);
        repo.meta().set_config(&ref_key, &target_id.to_hex()).ok();
        if !repo.dag().branch_exists(&branch_name) {
            let _ = repo.dag_mut().create_branch(branch_name.clone(), target_id);
        } else {
            let _ = repo.dag_mut().update_branch(&branch_name, target_id);
        }
        repo.meta().set_branch(&branch_name, &target_id)?;
    }

    repo.invalidate_head_cache();

    Ok(new_patches)
}

pub(crate) async fn do_pull(
    repo: &mut suture_core::repository::Repository,
    remote: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    do_pull_with_depth(repo, remote, None).await
}

pub(crate) async fn do_pull_with_depth(
    repo: &mut suture_core::repository::Repository,
    remote: &str,
    max_depth: Option<u32>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let old_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());
    let new_patches = do_fetch(repo, remote, max_depth).await?;
    repo.sync_working_tree(&old_tree)?;
    Ok(new_patches)
}
