use base64::Engine;
use clap::{CommandFactory, Parser, Subcommand};
use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const ANSI_RED: &str = "\x1b[31m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_BOLD_CYAN: &str = "\x1b[1;36m";
const ANSI_RESET: &str = "\x1b[0m";

#[derive(Parser)]
#[command(
    name = "suture",
    version,
    about = "Universal Semantic Version Control System"
)]
struct Cli {
    /// Run as if suture was started in <path>
    #[arg(short = 'C', global = true)]
    repo_path: Option<String>,
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
        /// Show ASCII graph of branch topology
        #[arg(short, long)]
        graph: bool,
        /// Show compact one-line format
        #[arg(long)]
        oneline: bool,
        /// Filter by author name
        #[arg(long)]
        author: Option<String>,
        /// Filter by commit message pattern
        #[arg(long)]
        grep: Option<String>,
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
    /// Apply a specific commit onto the current branch
    CherryPick {
        /// Commit hash to cherry-pick
        commit: String,
    },
    /// Rebase the current branch onto another branch
    Rebase {
        /// Target branch to rebase onto
        branch: String,
    },
    /// Show per-line commit attribution for a file
    Blame {
        /// File path to blame
        path: String,
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
    /// Fetch patches from a remote Hub without merging
    Fetch {
        /// Remote name (default: "origin")
        #[arg(default_value = "origin")]
        remote: String,
    },
    /// Clone a repository from a remote Hub
    Clone {
        /// Remote URL (e.g., http://localhost:50051)
        url: String,
        /// Target directory (default: repo name extracted from URL)
        dir: Option<String>,
    },
    /// Reset HEAD to a specific commit
    Reset {
        /// Target commit hash or branch name
        target: String,
        /// Reset mode
        #[arg(short, long, default_value = "mixed")]
        mode: String,
    },
    /// Signing key management
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// Stash management
    Stash {
        #[command(subcommand)]
        action: StashAction,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
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
enum StashAction {
    Push { #[arg(short, long)] message: Option<String> },
    Pop,
    Apply { index: usize },
    List,
    Drop { index: usize },
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

fn sign_push_request(
    repo: &suture_core::repository::Repository,
    mut req: PushRequest,
) -> Result<PushRequest, Box<dyn std::error::Error>> {
    let key_name = match repo.get_config("signing.key")? {
        Some(name) => name,
        None => return Ok(req),
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

fn walk_repo_files(dir: &std::path::Path) -> Vec<String> {
    let mut files = Vec::new();
    walk_repo_files_inner(dir, dir, &mut files);
    files
}

fn walk_repo_files_inner(root: &std::path::Path, current: &std::path::Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name();
        if name == ".suture" {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            walk_repo_files_inner(root, &path, files);
        } else if path.is_file() {
            files.push(rel);
        }
    }
}

fn format_line_diff(path: &str, changes: &[suture_core::engine::merge::LineChange]) {
    use suture_core::engine::merge::LineChange;

    let has_changes = changes.iter().any(|c| !matches!(c, LineChange::Unchanged(_)));
    if !has_changes {
        return;
    }

    println!("{ANSI_BOLD_CYAN}diff --git a/{path} b/{path}{ANSI_RESET}");
    println!("{ANSI_BOLD_CYAN}--- a/{path}{ANSI_RESET}");
    println!("{ANSI_BOLD_CYAN}+++ b/{path}{ANSI_RESET}");

    let mut old_line = 1usize;
    let mut new_line = 1usize;
    let mut i = 0;

    while i < changes.len() {
        match &changes[i] {
            LineChange::Unchanged(lines) => {
                old_line += lines.len();
                new_line += lines.len();
                i += 1;
            }
            LineChange::Deleted(_) | LineChange::Inserted(_) => {
                let hunk_old_start = old_line;
                let hunk_new_start = new_line;
                let mut hunk_old_count = 0usize;
                let mut hunk_new_count = 0usize;
                let mut hunk_lines: Vec<(char, String)> = Vec::new();

                while i < changes.len() {
                    match &changes[i] {
                        LineChange::Deleted(lines) => {
                            for line in lines {
                                hunk_lines.push(('-', line.clone()));
                                hunk_old_count += 1;
                                old_line += 1;
                            }
                            i += 1;
                        }
                        LineChange::Inserted(lines) => {
                            for line in lines {
                                hunk_lines.push(('+', line.clone()));
                                hunk_new_count += 1;
                                new_line += 1;
                            }
                            i += 1;
                        }
                        LineChange::Unchanged(_) => break,
                    }
                }

                println!(
                    "{ANSI_BOLD_CYAN}@@ -{hunk_old_start},{hunk_old_count} +{hunk_new_start},{hunk_new_count} @@{ANSI_RESET}"
                );
                for (prefix, line) in &hunk_lines {
                    if *prefix == '-' {
                        println!("{ANSI_RED}-{line}{ANSI_RESET}");
                    } else {
                        println!("{ANSI_GREEN}+{line}{ANSI_RESET}");
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Some(path) = &cli.repo_path {
        std::env::set_current_dir(path).map_err(|e| {
            format!("cannot change to '{}': {}", path, e)
        }).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        });
    }

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
        Commands::Log {
            branch,
            graph,
            oneline,
            author,
            grep,
        } => cmd_log(branch.as_deref(), graph, oneline, author.as_deref(), grep.as_deref()).await,
        Commands::Checkout { branch } => cmd_checkout(&branch).await,
        Commands::Diff { from, to } => cmd_diff(from.as_deref(), to.as_deref()).await,
        Commands::Revert { commit, message } => cmd_revert(&commit, message.as_deref()).await,
        Commands::Merge { source } => cmd_merge(&source).await,
        Commands::CherryPick { commit } => cmd_cherry_pick(&commit).await,
        Commands::Rebase { branch } => cmd_rebase(&branch).await,
        Commands::Blame { path } => cmd_blame(&path).await,
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
        Commands::Fetch { remote } => cmd_fetch(&remote).await,
        Commands::Clone { url, dir } => cmd_clone(&url, dir.as_deref()).await,
        Commands::Reset { target, mode } => cmd_reset(&target, &mode).await,
        Commands::Key { action } => cmd_key(&action).await,
        Commands::Stash { action } => cmd_stash(&action).await,
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "suture", &mut std::io::stdout());
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn cmd_cherry_pick(commit: &str) -> Result<(), Box<dyn std::error::Error>> {
    let patch_id = suture_common::Hash::from_hex(commit)?;
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let new_id = repo.cherry_pick(&patch_id)?;
    println!("Cherry-picked {} as {}", commit, new_id);
    Ok(())
}

async fn cmd_rebase(branch: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.rebase(branch)?;
    if result.patches_replayed > 0 {
        println!(
            "Rebase onto '{}': {} patch(es) replayed",
            branch, result.patches_replayed
        );
    } else {
        println!("Already up to date.");
    }
    Ok(())
}

async fn cmd_blame(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let entries = repo.blame(path)?;
    for entry in &entries {
        let short_hash = entry.patch_id.to_hex().chars().take(8).collect::<String>();
        if entry.patch_id == suture_common::Hash::ZERO {
            println!("{:>4} | {}", entry.line_number, entry.line);
        } else {
            println!(
                "{:>4} | {} ({}) {}",
                entry.line_number, short_hash, entry.author, entry.line
            );
        }
    }
    Ok(())
}

async fn cmd_stash(action: &StashAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    match action {
        StashAction::Push { message } => {
            let idx = repo.stash_push(message.as_deref())?;
            println!("Saved as stash@{{{}}}", idx);
        }
        StashAction::Pop => {
            repo.stash_pop()?;
            println!("Stash popped.");
        }
        StashAction::Apply { index } => {
            repo.stash_apply(*index)?;
            println!("Applied stash@{{{}}}", index);
        }
        StashAction::List => {
            let stashes = repo.stash_list()?;
            if stashes.is_empty() {
                println!("No stashes found.");
            } else {
                for s in &stashes {
                    println!("stash@{{{}}}: {} ({})", s.index, s.message, s.branch);
                }
            }
        }
        StashAction::Drop { index } => {
            repo.stash_drop(*index)?;
            println!("Dropped stash@{{{}}}", index);
        }
    }
    Ok(())
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

    let head_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());
    let staged_paths: std::collections::HashSet<&str> =
        status.staged_files.iter().map(|(p, _)| p.as_str()).collect();

    let mut unstaged_modified: Vec<String> = Vec::new();
    let mut unstaged_deleted: Vec<String> = Vec::new();
    let mut untracked: Vec<String> = Vec::new();

    let repo_dir = std::path::Path::new(".");
    let disk_files = walk_repo_files(repo_dir);

    for rel_path in &disk_files {
        let full_path = repo_dir.join(rel_path);
        if let Ok(data) = std::fs::read(&full_path) {
            let current_hash = suture_common::Hash::from_data(&data);
            if let Some(head_hash) = head_tree.get(rel_path) {
                if &current_hash != head_hash {
                    unstaged_modified.push(rel_path.clone());
                }
            } else if !staged_paths.contains(rel_path.as_str()) {
                untracked.push(rel_path.clone());
            }
        }
    }

    for (path, _) in head_tree.iter() {
        if !disk_files.iter().any(|f| f == path) && !staged_paths.contains(path.as_str()) {
            unstaged_deleted.push(path.clone());
        }
    }

    if !unstaged_modified.is_empty() || !unstaged_deleted.is_empty() || !untracked.is_empty() {
        println!("\nUnstaged changes:");
        for path in &unstaged_modified {
            let marker = if staged_paths.contains(path.as_str()) {
                " [staged+unstaged]"
            } else {
                ""
            };
            println!("  modified: {}{}", path, marker);
        }
        for path in &unstaged_deleted {
            println!("  deleted:  {}", path);
        }
        for path in &untracked {
            println!("  untracked: {}", path);
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

async fn cmd_log(
    branch: Option<&str>,
    graph: bool,
    oneline: bool,
    author: Option<&str>,
    grep: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if !graph {
        let mut patches = repo.log(branch)?;

        if let Some(author_filter) = author {
            patches.retain(|p| p.author.contains(author_filter));
        }
        if let Some(grep_filter) = grep {
            let grep_lower = grep_filter.to_lowercase();
            patches.retain(|p| p.message.to_lowercase().contains(&grep_lower));
        }

        if patches.is_empty() {
            println!("No commits.");
            return Ok(());
        }

        if oneline {
            for patch in &patches {
                let short_hash = patch.id.to_hex().chars().take(8).collect::<String>();
                println!("{} {}", short_hash, patch.message);
            }
            return Ok(());
        }

        for (i, patch) in patches.iter().enumerate() {
            if i == 0 {
                println!("* {} {}", patch.id.to_hex(), patch.message);
            } else {
                println!("  {} {}", patch.id.to_hex(), patch.message);
            }
        }

        return Ok(());
    }

    let branches = repo.list_branches();
    if branches.is_empty() {
        println!("No commits.");
        return Ok(());
    }

    let all_patches = repo.all_patches();
    let mut commit_groups: Vec<(Vec<suture_core::patch::types::PatchId>, String, u64)> = Vec::new();
    let mut seen_messages: std::collections::HashMap<(String, u64), usize> = std::collections::HashMap::new();

    for patch in &all_patches {
        let key = (patch.message.clone(), patch.timestamp);
        if let Some(&idx) = seen_messages.get(&key) {
            commit_groups[idx].0.push(patch.id);
        } else {
            seen_messages.insert(key, commit_groups.len());
            commit_groups.push((vec![patch.id], patch.message.clone(), patch.timestamp));
        }
    }

    commit_groups.sort_by(|a, b| b.2.cmp(&a.2));

    let branch_tips: std::collections::HashSet<suture_core::patch::types::PatchId> = branches
        .iter()
        .map(|(_, id)| *id)
        .collect();

    let tip_list: Vec<_> = branches.iter().collect();
    let mut col_assign: std::collections::HashMap<suture_core::patch::types::PatchId, usize> = std::collections::HashMap::new();
    for (i, (_, id)) in tip_list.iter().enumerate() {
        col_assign.insert(*id, i);
    }

    let mut next_col = tip_list.len();

    let num_cols = tip_list.len() + 5;
    for (patch_ids, message, _ts) in &commit_groups {
        let mut row = vec![' '; num_cols];
        let mut used_cols: std::collections::HashSet<usize> = std::collections::HashSet::new();

        for pid in patch_ids {
            if let Some(&col) = col_assign.get(pid) {
                row[col] = '*';
                used_cols.insert(col);
            } else {
                row[next_col % num_cols] = '*';
                used_cols.insert(next_col % num_cols);
                col_assign.insert(*pid, next_col % num_cols);
                next_col += 1;
            }
        }

        let is_tip = patch_ids.iter().any(|pid| branch_tips.contains(pid));
        if !is_tip {
            for &col in &used_cols {
                row[col] = '|';
            }
        }

        let row_str: String = row.iter().collect();
        let short_hash = if let Some(pid) = patch_ids.first() {
            pid.to_hex().chars().take(8).collect()
        } else {
            "????????".to_string()
        };

        let labels: Vec<String> = branches
            .iter()
            .filter(|(_, id)| patch_ids.contains(id))
            .map(|(name, _)| name.clone())
            .collect();
        let label_str = if labels.is_empty() {
            String::new()
        } else {
            format!(" ({})", labels.join(", "))
        };

        println!("{} {} {}{}", row_str, short_hash, message, label_str);
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
    use suture_core::engine::merge::diff_lines;

    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let entries = repo.diff(from, to)?;

    if entries.is_empty() {
        println!("No differences.");
        return Ok(());
    }

    for entry in &entries {
        match &entry.diff_type {
            DiffType::Renamed { old_path, new_path } => {
                println!(
                    "{ANSI_BOLD_CYAN}renamed {} → {}{ANSI_RESET}",
                    old_path, new_path
                );
            }
            DiffType::Added => {
                if let Some(new_hash) = &entry.new_hash {
                    let Ok(new_blob) = repo.cas().get_blob(new_hash) else {
                        println!(
                            "{ANSI_BOLD_CYAN}added {} (binary){ANSI_RESET}",
                            entry.path
                        );
                        continue;
                    };
                    let new_str = String::from_utf8_lossy(&new_blob);
                    let new_lines: Vec<&str> = new_str.lines().collect();
                    let changes = diff_lines(&[], &new_lines);
                    format_line_diff(&entry.path, &changes);
                } else {
                    println!("{ANSI_BOLD_CYAN}added {}{ANSI_RESET}", entry.path);
                }
            }
            DiffType::Deleted => {
                if let Some(old_hash) = &entry.old_hash {
                    let Ok(old_blob) = repo.cas().get_blob(old_hash) else {
                        println!(
                            "{ANSI_BOLD_CYAN}deleted {} (binary){ANSI_RESET}",
                            entry.path
                        );
                        continue;
                    };
                    let old_str = String::from_utf8_lossy(&old_blob);
                    let old_lines: Vec<&str> = old_str.lines().collect();
                    let changes = diff_lines(&old_lines, &[]);
                    format_line_diff(&entry.path, &changes);
                } else {
                    println!("{ANSI_BOLD_CYAN}deleted {}{ANSI_RESET}", entry.path);
                }
            }
            DiffType::Modified => {
                if let (Some(old_hash), Some(new_hash)) = (&entry.old_hash, &entry.new_hash) {
                    match (repo.cas().get_blob(old_hash), repo.cas().get_blob(new_hash)) {
                        (Ok(old_blob), Ok(new_blob)) => {
                            let old_str = String::from_utf8_lossy(&old_blob);
                            let new_str = String::from_utf8_lossy(&new_blob);
                            let old_lines: Vec<&str> = old_str.lines().collect();
                            let new_lines: Vec<&str> = new_str.lines().collect();
                            let changes = diff_lines(&old_lines, &new_lines);
                            format_line_diff(&entry.path, &changes);
                        }
                        _ => {
                            println!(
                                "{ANSI_BOLD_CYAN}modified {} (binary){ANSI_RESET}",
                                entry.path
                            );
                        }
                    }
                } else {
                    println!("{ANSI_BOLD_CYAN}modified {}{ANSI_RESET}", entry.path);
                }
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
        let entries = repo.list_config()?;
        if entries.is_empty() {
            println!("No configuration set.");
        } else {
            for (key, value) in &entries {
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

async fn cmd_push(remote: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let url = repo.get_remote_url(remote)?;

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

async fn do_fetch(
    repo: &mut suture_core::repository::Repository,
    remote: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
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
        eprintln!("Fetch failed: {}", text);
        return Ok(0);
    }

    let result: PullResponse = resp.json().await?;
    if !result.success {
        eprintln!("Fetch failed: {:?}", result.error);
        return Ok(0);
    }

    let b64 = base64::engine::general_purpose::STANDARD;

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

    Ok(new_patches)
}

async fn do_pull(
    repo: &mut suture_core::repository::Repository,
    remote: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let old_tree = repo
        .snapshot_head()
        .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());
    let new_patches = do_fetch(repo, remote).await?;
    repo.sync_working_tree(&old_tree)?;
    Ok(new_patches)
}

async fn cmd_pull(remote: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let new_patches = do_pull(&mut repo, remote).await?;
    println!("Pull successful: {} new patch(es)", new_patches);
    Ok(())
}

async fn cmd_fetch(remote: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let new_patches = do_fetch(&mut repo, remote).await?;
    println!("Fetch successful: {} new patch(es)", new_patches);
    Ok(())
}

async fn cmd_clone(url: &str, dir: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let repo_name = dir.unwrap_or_else(|| {
        url.trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("suture-repo")
    });

    let repo_path = PathBuf::from(repo_name);
    if repo_path.exists() {
        return Err(format!("directory '{}' already exists", repo_name).into());
    }

    std::fs::create_dir_all(&repo_path)?;
    let mut repo = suture_core::repository::Repository::init(&repo_path, "unknown")?;
    repo.add_remote("origin", url)?;

    let new_patches = do_pull(&mut repo, "origin").await?;

    println!("Cloned into '{}'", repo_name);
    if new_patches > 0 {
        println!("  {} patch(es) pulled", new_patches);
    }
    Ok(())
}

async fn cmd_reset(target: &str, mode: &str) -> Result<(), Box<dyn std::error::Error>> {
    use suture_core::repository::ResetMode;

    let reset_mode = match mode {
        "soft" => ResetMode::Soft,
        "mixed" => ResetMode::Mixed,
        "hard" => ResetMode::Hard,
        _ => {
            return Err(format!(
                "invalid reset mode: '{}' (expected soft, mixed, hard)",
                mode
            )
            .into());
        }
    };

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let target_id = repo.reset(target, reset_mode)?;
    println!("HEAD is now at {}", target_id);
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

            let pub_hex = hex::encode(keypair.public_key_bytes());
            repo.set_config(&format!("key.public.{name}"), &pub_hex)?;

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
