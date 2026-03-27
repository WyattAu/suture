use base64::Engine;
use clap::{Parser, Subcommand};
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
        name: String,
        /// Start branch from this target (branch name or HEAD)
        #[arg(short, long)]
        target: Option<String>,
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { path } => cmd_init(&path).await,
        Commands::Status => cmd_status().await,
        Commands::Add { paths, all } => cmd_add(&paths, all).await,
        Commands::Commit { message } => cmd_commit(&message).await,
        Commands::Branch { name, target } => cmd_branch(&name, target.as_deref()).await,
        Commands::Log { branch } => cmd_log(branch.as_deref()).await,
        Commands::Checkout { branch } => cmd_checkout(&branch).await,
        Commands::Diff { from, to } => cmd_diff(from.as_deref(), to.as_deref()).await,
        Commands::Revert { commit, message } => cmd_revert(&commit, message.as_deref()).await,
        Commands::Merge { source } => cmd_merge(&source).await,
        Commands::Remote { action } => cmd_remote(&action).await,
        Commands::Push { remote } => cmd_push(&remote).await,
        Commands::Pull { remote } => cmd_pull(&remote).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn cmd_init(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = PathBuf::from(path);
    let repo = suture_core::repository::Repository::init(&repo_path, "local")?;
    println!(
        "Initialized empty Suture repository in {}",
        repo_path.display()
    );
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

async fn cmd_branch(name: &str, target: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    repo.create_branch(name, target)?;
    println!("Created branch '{}'", name);
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

async fn cmd_push(remote: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let url = repo.get_remote_url(remote)?;

    let patches = repo.all_patches();
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
        signature: None, // TODO: sign when key is configured
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/push", url))
        .json(&push_body)
        .send()
        .await?;

    if resp.status().is_success() {
        let result: PushResponse = resp.json().await?;
        if result.success {
            println!("Push successful");
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
    let old_tree = repo.snapshot_head().unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());

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
