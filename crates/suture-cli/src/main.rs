use base64::Engine;
use clap::{Parser, Subcommand};
use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "suture",
    version,
    about = "Universal Semantic Version Control System"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Suture repository
    Init {
        /// Repository path (default: current directory)
        #[arg(default_value = ".")]
        path: String,
    },
    /// Show repository status
    Status,
    /// Add files to the staging area
    Add {
        /// File paths to add (ignored when --all is used)
        paths: Vec<String>,
        /// Add all files (respecting .sutureignore)
        #[arg(short, long)]
        all: bool,
    },
    /// Create a commit
    Commit {
        /// Commit message
        message: String,
    },
    /// Branch operations
    Branch {
        /// Branch name
        name: Option<String>,
        /// Start branch from this target (branch name or HEAD)
        #[arg(short, long)]
        target: Option<String>,
        /// Delete a branch
        #[arg(short, long)]
        delete: bool,
        /// List branches
        #[arg(short, long)]
        list: bool,
    },
    /// Show commit history
    Log {
        /// Branch to show log for (default: HEAD)
        branch: Option<String>,
    },
    /// Switch to a different branch
    Checkout {
        /// Branch name to checkout
        branch: String,
    },
    /// Show differences between commits or branches
    Diff {
        /// From ref (commit hash or branch name). Omit for empty tree.
        #[arg(short, long)]
        from: Option<String>,
        /// To ref (commit hash or branch name). Omit for HEAD.
        #[arg(short, long)]
        to: Option<String>,
    },
    /// Revert a commit
    Revert {
        /// Commit hash to revert
        commit: String,
        /// Custom revert message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Merge a branch into the current branch
    Merge {
        /// Source branch to merge into HEAD
        source: String,
    },
    /// Tag operations
    Tag {
        /// Tag name (required for create/delete, omit to list)
        name: Option<String>,
        /// Target commit/branch (default: HEAD)
        #[arg(short, long)]
        target: Option<String>,
        /// Delete a tag
        #[arg(short, long)]
        delete: bool,
        /// List tags
        #[arg(short, long)]
        list: bool,
    },
    /// Get or set configuration values
    Config {
        /// Key to get, or key=value to set
        key_value: Vec<String>,
    },
    /// Remote operations
    Remote {
        #[command(subcommand)]
        action: RemoteAction,
    },
    /// Push patches to a remote Hub
    Push {
        /// Remote name (default: "origin")
        #[arg(default_value = "origin")]
        remote: String,
    },
    /// Pull patches from a remote Hub
    Pull {
        /// Remote name (default: "origin")
        #[arg(default_value = "origin")]
        remote: String,
    },
    /// Signing key management
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
}

#[derive(Subcommand)]
enum KeyAction {
    /// Generate a new Ed25519 keypair
    Generate {
        /// Key name (default: "default")
        #[arg(default_value = "default")]
        name: String,
    },
    /// List local signing keys (public keys)
    List,
    /// Show the public key for a named key
    Public {
        /// Key name (default: "default")
        #[arg(default_value = "default")]
        name: String,
    },
}

#[derive(Subcommand)]
enum RemoteAction {
    /// Add a remote Hub
    Add {
        /// Remote name
        name: String,
        /// Remote URL (e.g., http://localhost:50051)
        url: String,
    },
    /// List configured remotes
    List,
}

// Hub proto types (matching suture-hub/src/types.rs)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct HashProto {
    value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PatchProto {
    id: HashProto,
    operation_type: String,
    touch_set: Vec<String>,
    target_path: Option<String>,
    payload: String,
    parent_ids: Vec<HashProto>,
    author: String,
    message: String,
    timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BranchProto {
    name: String,
    target_id: HashProto,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BlobRef {
    hash: HashProto,
    data: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PushRequest {
    repo_id: String,
    patches: Vec<PatchProto>,
    branches: Vec<BranchProto>,
    blobs: Vec<BlobRef>,
    /// Optional Ed25519 signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PushResponse {
    success: bool,
    #[allow(dead_code)]
    error: Option<String>,
    #[allow(dead_code)]
    existing_patches: Vec<HashProto>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PullRequest {
    repo_id: String,
    known_branches: Vec<BranchProto>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PullResponse {
    success: bool,
    error: Option<String>,
    patches: Vec<PatchProto>,
    branches: Vec<BranchProto>,
    blobs: Vec<BlobRef>,
}

fn hex_to_hash_proto(hex: &str) -> HashProto {
    HashProto {
        value: hex.to_string(),
    }
}

fn patch_to_proto(patch: &suture_core::patch::types::Patch) -> PatchProto {
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

fn proto_to_patch(proto: &PatchProto) -> Result<suture_core::patch::types::Patch, Box<dyn std::error::Error>> {
    use suture_core::patch::types::{OperationType, Patch, PatchId, TouchSet};
    use suture_common::Hash;

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

/// Build canonical bytes for a push request (for Ed25519 signing).
/// Must match the hub's `canonical_push_bytes` exactly.
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { path } => cmd_init(&path).await,
        Commands::Status => cmd_status().await,
        Commands::Add { paths, all } => cmd_add(&paths, all).await,
        Commands::Commit { message } => cmd_commit(&message).await,
        Commands::Branch {
            name,
            target,
            delete,
            list,
        } => cmd_branch(name.as_deref(), target.as_deref(), delete, list).await,
        Commands::Log { branch } => cmd_log(branch.as_deref()).await,
        Commands::Checkout { branch } => cmd_checkout(&branch).await,
        Commands::Diff { from, to } => cmd_diff(from.as_deref(), to.as_deref()).await,
        Commands::Revert { commit, message } => cmd_revert(&commit, message.as_deref()).await,
        Commands::Merge { source } => cmd_merge(&source).await,
        Commands::Tag {
            name,
            target,
            delete,
            list,
        } => cmd_tag(name.as_deref(), target.as_deref(), delete, list).await,
        Commands::Config { key_value } => cmd_config(&key_value).await,
        Commands::Remote { action } => cmd_remote(&action).await,
        Commands::Push { remote } => cmd_push(&remote).await,
        Commands::Pull { remote } => cmd_pull(&remote).await,
        Commands::Key { action } => cmd_key(&action).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn cmd_init(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = PathBuf::from(path);
    let repo = suture_core::repository::Repository::init(&repo_path, "unknown")?;
    println!(
        "Initialized empty Suture repository in {}",
        repo_path.display()
    );
    println!("Hint: run `suture config user.name \"Your Name\"` to set your identity");
    drop(repo);
    Ok(())
}

async fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let status = repo.status()?;

    println!(
        "On branch {}",
        status.head_branch.as_deref().unwrap_or("detached")
    );
    if let Some(id) = status.head_patch {
        println!("HEAD: {}", id);
    }
    println!(
        "{} patches, {} branches",
        status.patch_count, status.branch_count
    );

    if !status.staged_files.is_empty() {
        println!("\nStaged changes:");
        for (path, file_status) in &status.staged_files {
            println!("  {:?} {}", file_status, path);
        }
    }

    Ok(())
}

async fn cmd_add(paths: &[String], all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    if all {
        let count = repo.add_all()?;
        println!("Staged {} files", count);
    } else {
        for path in paths {
            repo.add(path)?;
            println!("Added {}", path);
        }
    }
    Ok(())
}

async fn cmd_commit(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patch_id = repo.commit(message)?;
    println!("Committed: {}", patch_id);
    Ok(())
}

async fn cmd_branch(
    name: Option<&str>,
    target: Option<&str>,
    delete: bool,
    list: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if list || name.is_none() {
        let branches = repo.list_branches();
        if branches.is_empty() {
            println!("No branches.");
        } else {
            let head = repo.head().ok();
            let head_branch = head.as_ref().map(|(n, _)| n.as_str());
            for (bname, _target) in &branches {
                let marker = if head_branch == Some(bname.as_str()) {
                    "* "
                } else {
                    "  "
                };
                println!("{}{}", marker, bname);
            }
        }
        return Ok(());
    }

    let name = name.unwrap();
    if delete {
        repo.delete_branch(name)?;
        println!("Deleted branch '{}'", name);
    } else {
        repo.create_branch(name, target)?;
        println!("Created branch '{}'", name);
    }
    Ok(())
}

async fn cmd_log(branch: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.log(branch)?;

    if patches.is_empty() {
        println!("No commits.");
        return Ok(());
    }

    for (i, patch) in patches.iter().enumerate() {
        if i == 0 {
            println!("* {} {}", patch.id.to_hex(), patch.message);
        } else {
            println!("  {} {}", patch.id.to_hex(), patch.message);
        }
    }

    Ok(())
}

async fn cmd_checkout(branch: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    repo.checkout(branch)?;
    println!("Switched to branch '{}'", branch);
    Ok(())
}

async fn cmd_diff(from: Option<&str>, to: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use suture_core::engine::diff::DiffType;

    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let entries = repo.diff(from, to)?;

    if entries.is_empty() {
        println!("No differences.");
        return Ok(());
    }

    for entry in &entries {
        match &entry.diff_type {
            DiffType::Renamed { old_path, new_path } => {
                println!("{} {} → {}", entry.diff_type, old_path, new_path);
            }
            _ => {
                println!("{} {}", entry.diff_type, entry.path);
            }
        }
    }

    Ok(())
}

async fn cmd_revert(commit: &str, message: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let patch_id = suture_core::Hash::from_hex(commit)?;
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let revert_id = repo.revert(&patch_id, message)?;
    println!("Reverted: {}", revert_id);
    Ok(())
}

async fn cmd_merge(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.execute_merge(source)?;

    if result.is_clean {
        if let Some(id) = result.merge_patch_id {
            println!("Merge successful: {}", id);
        }
        if result.patches_applied > 0 {
            println!(
                "Applied {} patch(es) from '{}'",
                result.patches_applied, source
            );
        }
    } else {
        println!(
            "Merge has {} conflict(s):",
            result.unresolved_conflicts.len()
        );
        for conflict in &result.unresolved_conflicts {
            println!(
                "  CONFLICT in '{}': edit the file, then commit to resolve",
                conflict.path
            );
        }
        println!("Hint: resolve conflicts, then run `suture commit`");
    }

    Ok(())
}

async fn cmd_tag(
    name: Option<&str>,
    target: Option<&str>,
    delete: bool,
    list: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if list || name.is_none() {
        let tags = repo.list_tags()?;
        if tags.is_empty() {
            println!("No tags.");
        } else {
            for (tname, target_id) in &tags {
                println!("{}  {}", tname, target_id);
            }
        }
        return Ok(());
    }

    let name = name.unwrap();
    if delete {
        repo.delete_tag(name)?;
        println!("Deleted tag '{}'", name);
    } else {
        repo.create_tag(name, target)?;
        let target_id = repo.resolve_tag(name)?.unwrap();
        println!("Tag '{}' -> {}", name, target_id);
    }
    Ok(())
}

async fn cmd_config(key_value: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if key_value.is_empty() {
        // List all config
        let entries = repo.list_config()?;
        if entries.is_empty() {
            println!("No configuration set.");
        } else {
            for (key, value) in &entries {
                // Hide internal keys
                if key.starts_with("pending_merge_parents") || key.starts_with("head_branch") {
                    continue;
                }
                println!("{}={}", key, value);
            }
        }
        return Ok(());
    }

    let kv = &key_value[0];
    if let Some((key, value)) = kv.split_once('=') {
        repo.set_config(key.trim(), value.trim())?;
        println!("{}={}", key.trim(), value.trim());
    } else {
        let key = kv.trim();
        match repo.get_config(key)? {
            Some(value) => println!("{}", value),
            None => {
                eprintln!("config key '{}' not found", key);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

async fn cmd_remote(action: &RemoteAction) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    match action {
        RemoteAction::Add { name, url } => {
            repo.add_remote(name, url)?;
            println!("Remote '{}' added -> {}", name, url);
        }
        RemoteAction::List => {
            let remotes = repo.list_remotes()?;
            if remotes.is_empty() {
                println!("No remotes configured.");
            } else {
                for (name, url) in &remotes {
                    println!("{}\t{}", name, url);
                }
            }
        }
    }
    Ok(())
}

/// Sign a push request if a signing key is configured.
/// Returns the request with the signature field populated.
fn sign_push_request(
    repo: &suture_core::repository::Repository,
    mut req: PushRequest,
) -> Result<PushRequest, Box<dyn std::error::Error>> {
    let key_name = match repo.get_config("signing.key")? {
        Some(name) => name,
        None => return Ok(req), // No signing configured
    };

    let keys_dir = std::path::Path::new(".suture").join("keys");
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

async fn cmd_push(remote: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let url = repo.get_remote_url(remote)?;

    // Incremental push: check last-pushed state
    let push_state_key = format!("remote.{}.last_pushed", remote);
    let patches = if let Some(last_pushed_hex) = repo.get_config(&push_state_key)? {
        let last_pushed = suture_common::Hash::from_hex(&last_pushed_hex)?;
        repo.patches_since(&last_pushed)
    } else {
        repo.all_patches()
    };

    let branches = repo.list_branches();
    let b64 = base64::engine::general_purpose::STANDARD;

    let mut blobs = Vec::new();
    let mut seen_hashes = std::collections::HashSet::new();
    for patch in &patches {
        if !patch.payload.is_empty() {
            let hash_hex = String::from_utf8_lossy(&patch.payload).to_string();
            let Ok(hash) = suture_common::Hash::from_hex(&hash_hex) else {
                continue;
            };
            if !seen_hashes.contains(&hash_hex) {
                seen_hashes.insert(hash_hex.clone());
                let Ok(blob_data) = repo.cas().get_blob(&hash) else {
                    continue;
                };
                blobs.push(BlobRef {
                    hash: hex_to_hash_proto(&hash_hex),
                    data: b64.encode(&blob_data),
                });
            }
        }
    }

    let push_body = PushRequest {
        repo_id: "default".to_string(),
        patches: patches.iter().map(patch_to_proto).collect(),
        branches: branches
            .iter()
            .map(|(name, target_id)| BranchProto {
                name: name.clone(),
                target_id: hex_to_hash_proto(&target_id.to_hex()),
            })
            .collect(),
        blobs,
        signature: None,
    };

    // Sign the push if a signing key is configured
    let push_body = sign_push_request(&repo, push_body)?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/push", url))
        .json(&push_body)
        .send()
        .await?;

    if resp.status().is_success() {
        let result: PushResponse = resp.json().await?;
        if result.success {
            // Update last-pushed state to current HEAD
            let (_, head_id) = repo.head()?;
            repo.set_config(&push_state_key, &head_id.to_hex())?;
            println!("Push successful ({} patch(es))", patches.len());
        } else {
            eprintln!("Push failed: {:?}", result.error);
        }
    } else {
        let text = resp.text().await?;
        eprintln!("Push failed: {}", text);
    }

    Ok(())
}

async fn cmd_pull(remote: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let url = repo.get_remote_url(remote)?;

    let known_branches = repo
        .list_branches()
        .iter()
        .map(|(name, target_id)| BranchProto {
            name: name.clone(),
            target_id: hex_to_hash_proto(&target_id.to_hex()),
        })
        .collect();

    let pull_body = PullRequest {
        repo_id: "default".to_string(),
        known_branches,
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/pull", url))
        .json(&pull_body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        eprintln!("Pull failed: {}", text);
        return Ok(());
    }

    let result: PullResponse = resp.json().await?;
    if !result.success {
        eprintln!("Pull failed: {:?}", result.error);
        return Ok(());
    }

    let b64 = base64::engine::general_purpose::STANDARD;
    let old_tree =
        repo.snapshot_head()
            .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());

    for blob in &result.blobs {
        let hash = suture_common::Hash::from_hex(&blob.hash.value)?;
        let data = b64.decode(&blob.data)?;
        repo.cas().put_blob_with_hash(&data, &hash)?;
    }

    let mut new_patches = 0;
    for patch_proto in &result.patches {
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

    for branch in &result.branches {
        let target_id = suture_common::Hash::from_hex(&branch.target_id.value)?;
        let branch_name = suture_common::BranchName::new(&branch.name)?;
        if !repo.dag().branch_exists(&branch_name) {
            let _ = repo.dag_mut().create_branch(branch_name.clone(), target_id);
        } else {
            let _ = repo.dag_mut().update_branch(&branch_name, target_id);
        }
        repo.meta().set_branch(&branch_name, &target_id)?;
    }

    repo.sync_working_tree(&old_tree)?;

    println!("Pull successful: {} new patch(es)", new_patches);
    Ok(())
}

async fn cmd_key(action: &KeyAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    match action {
        KeyAction::Generate { name } => {
            let keypair = suture_core::signing::SigningKeypair::generate();
            let keys_dir = std::path::Path::new(".suture").join("keys");
            std::fs::create_dir_all(&keys_dir)?;

            let priv_path = keys_dir.join(format!("{name}.ed25519"));
            std::fs::write(&priv_path, keypair.private_key_bytes())?;

            // Store public key in repo config
            let pub_hex = hex::encode(keypair.public_key_bytes());
            repo.set_config(&format!("key.public.{name}"), &pub_hex)?;

            // Set as default signing key if this is the "default" key
            if name == "default" {
                repo.set_config("signing.key", "default")?;
            }

            println!("Generated keypair '{}'", name);
            println!("  Private key: {}", priv_path.display());
            println!("  Public key:  {}", pub_hex);
            if name != "default" {
                println!(
                    "Hint: run `suture config signing.key={name}` to use this key for signing"
                );
            }
        }
        KeyAction::List => {
            let entries = repo.list_config()?;
            let mut found = false;
            for (key, value) in &entries {
                if let Some(name) = key.strip_prefix("key.public.") {
                    println!("{}  {}", name, value);
                    found = true;
                }
            }
            if !found {
                println!("No signing keys found.");
                println!("Run `suture key generate` to create one.");
            }
        }
        KeyAction::Public { name } => {
            let key = format!("key.public.{name}");
            match repo.get_config(&key)? {
                Some(pub_hex) => println!("{}", pub_hex),
                None => {
                    eprintln!("No public key found for '{}'", name);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
