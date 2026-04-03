use base64::Engine;
use clap::{CommandFactory, Parser, Subcommand};
use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const ANSI_RED: &str = "\x1b[31m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_BOLD_CYAN: &str = "\x1b[1;36m";
const ANSI_RESET: &str = "\x1b[0m";

/// Run a hook if it exists. Returns Ok(()) if the hook doesn't exist or succeeds.
/// Returns Err with a descriptive message if the hook fails.
///
/// Callers should pass `SUTURE_BRANCH`, `SUTURE_HEAD`, and optionally
/// `SUTURE_AUTHOR` via `extra_env`.
fn run_hook_if_exists(
    repo_root: &std::path::Path,
    hook_name: &str,
    extra_env: std::collections::HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let author = extra_env.get("SUTURE_AUTHOR").cloned();
    let branch = extra_env.get("SUTURE_BRANCH").cloned();
    let head = extra_env.get("SUTURE_HEAD").cloned();

    let env = suture_core::hooks::build_env(
        repo_root,
        hook_name,
        author.as_deref(),
        branch.as_deref(),
        head.as_deref(),
        extra_env,
    );

    match suture_core::hooks::run_hooks(repo_root, hook_name, &env) {
        Ok(results) => {
            for result in &results {
                // Print hook stdout to the user
                if !result.stdout.is_empty() {
                    print!("{}", result.stdout);
                }
                if !result.success() {
                    let msg = format!(
                        "{}{} {}",
                        ANSI_RED,
                        suture_core::hooks::format_hook_result(result),
                        ANSI_RESET
                    );
                    eprintln!("{}", msg);
                    if !result.stderr.is_empty() {
                        eprintln!("{}", result.stderr);
                    }
                    return Err(format!(
                        "Hook '{}' failed (exit code {:?}). Aborting.",
                        hook_name, result.exit_code
                    )
                    .into());
                }
            }
            Ok(())
        }
        Err(suture_core::hooks::HookError::NotFound(_)) => {
            // No hook configured — this is fine, silently continue
            Ok(())
        }
        Err(e) => Err(format!("Hook '{}' error: {}", hook_name, e).into()),
    }
}

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
    /// Remove files from the working tree and staging area
    Rm {
        /// File paths to remove
        paths: Vec<String>,
        /// Only remove from staging area, keep the file on disk
        #[arg(short, long)]
        cached: bool,
    },
    /// Create a commit
    Commit {
        /// Commit message
        message: String,
        /// Auto-stage all modified/deleted files before committing
        #[arg(short, long)]
        all: bool,
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
        /// Show only the first-parent chain (skip merge parents)
        #[arg(long)]
        first_parent: bool,
        /// Show compact one-line format
        #[arg(long)]
        oneline: bool,
        /// Filter by author name
        #[arg(long)]
        author: Option<String>,
        /// Filter by commit message pattern
        #[arg(long)]
        grep: Option<String>,
        /// Show commits across all branches
        #[arg(long)]
        all: bool,
        /// Show commits newer than a date/time (format: "2026-01-15" or "2 weeks ago")
        #[arg(long)]
        since: Option<String>,
        /// Show commits older than a date/time (same format as --since)
        #[arg(long)]
        until: Option<String>,
    },
    /// Switch to a different branch
    Checkout {
        /// Branch name to checkout (defaults to HEAD when -b is used)
        branch: Option<String>,
        /// Create a new branch before switching
        #[arg(short = 'b', long)]
        new_branch: Option<String>,
    },
    /// Move or rename a tracked file
    Mv {
        /// Source path
        source: String,
        /// Destination path
        destination: String,
    },
    /// Show differences between commits or branches
    Diff {
        /// From ref (commit hash or branch name). Omit for HEAD.
        #[arg(short, long)]
        from: Option<String>,
        /// To ref (commit hash or branch name). Omit for working tree.
        #[arg(short, long)]
        to: Option<String>,
        /// Show staged changes (diff of staging area vs HEAD)
        #[arg(long)]
        cached: bool,
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
        /// Create an annotated tag with a message
        #[arg(short, long)]
        annotate: bool,
        /// Tag message (required with --annotate)
        #[arg(short, long)]
        message: Option<String>,
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
        /// Rebase local commits on top of fetched remote history
        #[arg(long)]
        rebase: bool,
    },
    /// Fetch patches from a remote Hub without merging
    Fetch {
        /// Remote name (default: "origin")
        #[arg(default_value = "origin")]
        remote: String,
        /// Limit fetch to the last N commits
        #[arg(long, help = "Limit fetch to the last N commits")]
        depth: Option<u32>,
    },
    /// Clone a repository from a remote Hub
    Clone {
        /// Remote URL (e.g., http://localhost:50051)
        url: String,
        /// Target directory (default: repo name extracted from URL)
        dir: Option<String>,
        /// Shallow clone: fetch only the last N patches
        #[arg(short, long)]
        depth: Option<u32>,
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
    /// Show detailed information about a commit
    Show {
        /// Commit hash or branch name
        commit: String,
    },
    /// Show the reference log (HEAD movements)
    Reflog,
    /// List available semantic drivers
    Drivers,
    /// Show compact commit summary grouped by author
    Shortlog {
        /// Branch to show log for (default: HEAD)
        branch: Option<String>,
        /// Number of commits to show (default: all)
        #[arg(short = 'n', long)]
        number: Option<usize>,
    },
    /// Manage commit notes
    Notes {
        #[command(subcommand)]
        action: NotesAction,
    },
    /// Show version information
    Version,
    /// Garbage collect unreachable objects
    Gc,
    /// Verify repository integrity
    Fsck,
    /// Binary search for bug-introducing commit
    Bisect {
        #[command(subcommand)]
        action: BisectAction,
    },
    /// Squash N commits into one
    Squash {
        /// Number of commits to squash
        count: usize,
        /// Custom message for the squashed commit (default: combined messages)
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Launch terminal UI
    Tui,
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
    Push {
        #[arg(short, long)]
        message: Option<String>,
    },
    Pop,
    Apply {
        index: usize,
    },
    List,
    Drop {
        index: usize,
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
    /// Remove a configured remote
    Remove {
        /// Remote name
        name: String,
    },
    /// Authenticate with a remote Hub and store a token
    Login {
        /// Remote name (default: "origin")
        #[arg(default_value = "origin")]
        name: String,
    },
    /// Mirror a remote repository locally
    Mirror {
        /// Upstream Hub URL
        url: String,
        /// Repository name on upstream Hub
        repo: String,
        /// Local name for the mirrored repo
        #[arg(long, default_value = None)]
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum NotesAction {
    /// Add a note to a commit
    Add {
        /// Commit hash
        commit: String,
        /// Note message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// List notes for a commit
    List {
        /// Commit hash
        commit: String,
    },
    /// Show notes for a commit (alias for list)
    Show {
        /// Commit hash
        commit: String,
    },
    /// Remove a note from a commit
    Remove {
        /// Commit hash
        commit: String,
        /// Note index (0-based)
        index: usize,
    },
}

#[derive(Subcommand)]
enum BisectAction {
    /// Start a bisect session
    Start {
        /// Known good commit (no bug)
        good: String,
        /// Known bad commit (has bug)
        bad: String,
    },
    /// Automatically bisect using a test command
    Run {
        /// Known good commit (no bug)
        good: String,
        /// Known bad commit (has bug)
        bad: String,
        /// Test command to run at each step (exit 0 = good, non-zero = bad)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        cmd: Vec<String>,
    },
    /// Reset/cancel a bisect session
    Reset,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_depth: Option<u32>,
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

fn proto_to_patch(
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

fn get_remote_token(
    repo: &suture_core::repository::Repository,
    remote: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let key = format!("remote.{}.token", remote);
    Ok(repo.get_config(&key)?)
}

fn derive_repo_id(url: &str, remote_name: &str) -> String {
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

async fn check_handshake(url: &str) -> Result<(), Box<dyn std::error::Error>> {
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

fn walk_repo_files(dir: &std::path::Path) -> Vec<String> {
    let mut files = Vec::new();
    walk_repo_files_inner(dir, dir, &mut files);
    files
}

fn walk_repo_files_inner(
    root: &std::path::Path,
    current: &std::path::Path,
    files: &mut Vec<String>,
) {
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

    let has_changes = changes
        .iter()
        .any(|c| !matches!(c, LineChange::Unchanged(_)));
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

fn resolve_ref<'a>(
    repo: &suture_core::repository::Repository,
    ref_str: &str,
    all_patches: &'a [suture_core::patch::types::Patch],
) -> Result<&'a suture_core::patch::types::Patch, Box<dyn std::error::Error>> {
    if ref_str == "HEAD" || ref_str.starts_with("HEAD~") {
        let (_branch_name, head_id) = repo.head().map_err(|e| e.to_string())?;
        let mut target_id = head_id;
        if let Some(n_str) = ref_str.strip_prefix("HEAD~") {
            let n: usize = n_str
                .parse()
                .map_err(|_| format!("invalid HEAD~N: {}", n_str))?;
            for _ in 0..n {
                let patch = all_patches
                    .iter()
                    .find(|p| p.id == target_id)
                    .ok_or_else(|| String::from("HEAD ancestor not found in patches"))?;
                target_id = *patch
                    .parent_ids
                    .first()
                    .ok_or_else(|| String::from("HEAD has no parent"))?;
            }
        }
        return all_patches
            .iter()
            .find(|p| p.id == target_id)
            .ok_or_else(|| "HEAD not found in patches".into());
    }

    {
        let branches = repo.list_branches();
        for (name, target_id) in &branches {
            if name == ref_str {
                return all_patches
                    .iter()
                    .find(|p| p.id == *target_id)
                    .ok_or_else(|| "branch tip not found in patches".into());
            }
        }
    }
    if let Ok(Some(target_id)) = repo.resolve_tag(ref_str) {
        return all_patches
            .iter()
            .find(|p| p.id == target_id)
            .ok_or_else(|| "tag target not found in patches".into());
    }
    let matches: Vec<&suture_core::patch::types::Patch> = all_patches
        .iter()
        .filter(|p| p.id.to_hex().starts_with(ref_str))
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(format!("unknown ref: {}", ref_str).into()),
        n => Err(format!("ambiguous ref '{}' matches {} commits", ref_str, n).into()),
    }
}

fn format_timestamp(ts: u64) -> String {
    let days = ts / 86400;
    let hours = (ts % 86400) / 3600;
    let minutes = (ts % 3600) / 60;
    let remaining_secs = ts % 60;
    format!(
        "{}d {:02}:{:02}:{:02} (unix: {})",
        days, hours, minutes, remaining_secs, ts
    )
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Some(path) = &cli.repo_path {
        std::env::set_current_dir(path)
            .map_err(|e| format!("cannot change to '{}': {}", path, e))
            .unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
    }

    let result = match cli.command {
        Commands::Init { path } => cmd_init(&path).await,
        Commands::Status => cmd_status().await,
        Commands::Add { paths, all } => cmd_add(&paths, all).await,
        Commands::Rm { paths, cached } => cmd_rm(&paths, cached).await,
        Commands::Commit { message, all } => cmd_commit(&message, all).await,
        Commands::Branch {
            name,
            target,
            delete,
            list,
        } => cmd_branch(name.as_deref(), target.as_deref(), delete, list).await,
        Commands::Log {
            branch,
            graph,
            first_parent,
            oneline,
            author,
            grep,
            all,
            since,
            until,
        } => {
            cmd_log(
                branch.as_deref(),
                graph,
                first_parent,
                oneline,
                author.as_deref(),
                grep.as_deref(),
                all,
                since.as_deref(),
                until.as_deref(),
            )
            .await
        }
        Commands::Checkout { branch, new_branch } => {
            cmd_checkout(branch.as_deref(), new_branch.as_deref()).await
        }
        Commands::Mv {
            source,
            destination,
        } => cmd_mv(&source, &destination).await,
        Commands::Diff { from, to, cached } => {
            cmd_diff(from.as_deref(), to.as_deref(), cached).await
        }
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
            annotate,
            message,
        } => {
            cmd_tag(
                name.as_deref(),
                target.as_deref(),
                delete,
                list,
                annotate,
                message.as_deref(),
            )
            .await
        }
        Commands::Config { key_value } => cmd_config(&key_value).await,
        Commands::Remote { action } => cmd_remote(&action).await,
        Commands::Push { remote } => cmd_push(&remote).await,
        Commands::Pull { remote, rebase } => cmd_pull(&remote, rebase).await,
        Commands::Fetch { remote, depth } => cmd_fetch(&remote, depth).await,
        Commands::Clone { url, dir, depth } => cmd_clone(&url, dir.as_deref(), depth).await,
        Commands::Reset { target, mode } => cmd_reset(&target, &mode).await,
        Commands::Key { action } => cmd_key(&action).await,
        Commands::Stash { action } => cmd_stash(&action).await,
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "suture", &mut std::io::stdout());
            Ok(())
        }
        Commands::Show { commit } => cmd_show(&commit).await,
        Commands::Reflog => cmd_reflog().await,
        Commands::Drivers => cmd_drivers().await,
        Commands::Shortlog { branch, number } => cmd_shortlog(branch.as_deref(), number).await,
        Commands::Notes { action } => cmd_notes(&action).await,
        Commands::Gc => cmd_gc().await,
        Commands::Fsck => cmd_fsck().await,
        Commands::Bisect { action } => cmd_bisect(&action).await,
        Commands::Squash { count, message } => cmd_squash(count, message.as_deref()).await,
        Commands::Version => cmd_version().await,
        Commands::Tui => cmd_tui().await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn cmd_version() -> Result<(), Box<dyn std::error::Error>> {
    println!("suture {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

async fn cmd_tui() -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = std::path::Path::new(".");
    suture_tui::run(repo_path)?;
    Ok(())
}

async fn cmd_squash(count: usize, message: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let msg = message.unwrap_or("squashed commit");
    let new_id = repo.squash(count, msg)?;
    println!("Squashed {} commits into {}", count, new_id);
    Ok(())
}

async fn cmd_gc() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.gc()?;
    println!("Garbage collection complete.");
    println!("  {} patch(es) removed", result.patches_removed);
    if result.patches_removed > 0 {
        println!("  Hint: reopen the repository to fully update the in-memory DAG");
    }
    Ok(())
}

async fn cmd_fsck() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.fsck()?;
    println!("Repository integrity check complete.");
    println!("  {} check(s) passed", result.checks_passed);
    if !result.warnings.is_empty() {
        println!("\nWarnings:");
        for w in &result.warnings {
            println!("  WARNING: {}", w);
        }
    }
    if !result.errors.is_empty() {
        println!("\nErrors:");
        for e in &result.errors {
            println!("  ERROR: {}", e);
        }
    }
    Ok(())
}

async fn cmd_bisect(action: &BisectAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        BisectAction::Start {
            good: good_ref,
            bad: bad_ref,
        } => {
            let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
            let all_patches = repo.all_patches();

            let good_patch = resolve_ref(&repo, good_ref, &all_patches)?;
            let bad_patch = resolve_ref(&repo, bad_ref, &all_patches)?;

            let log = repo.log(None)?;

            let good_idx = log
                .iter()
                .position(|p| p.id == good_patch.id)
                .ok_or_else(|| format!("'{}' not found in history", good_ref))?;
            let bad_idx = log
                .iter()
                .position(|p| p.id == bad_patch.id)
                .ok_or_else(|| format!("'{}' not found in history", bad_ref))?;

            let bad_ancestors = repo.dag().ancestors(&bad_patch.id);
            if !bad_ancestors.contains(&good_patch.id) && good_patch.id != bad_patch.id {
                return Err("'good' must be an ancestor of 'bad'".into());
            }

            // log returns newest first, so higher index = older commit
            let (older_idx, newer_idx) = if good_idx > bad_idx {
                (good_idx, bad_idx) // good is older (higher idx), bad is newer (lower idx)
            } else {
                (bad_idx, good_idx) // bad is older, good is newer
            };

            let remaining = older_idx - newer_idx - 1;
            if remaining == 0 {
                println!("Only one commit between good and bad:");
                println!(
                    "  {} {}",
                    &log[newer_idx + 1].id.to_hex()[..8],
                    log[newer_idx + 1].message.lines().next().unwrap_or("")
                );
                println!("  This is the first bad commit.");
                return Ok(());
            }

            let midpoint_idx = (older_idx + newer_idx) / 2;
            let midpoint = &log[midpoint_idx];

            println!(
                "Bisecting: {} commit(s) remaining between good ({}) and bad ({})",
                remaining,
                &good_patch.id.to_hex()[..8],
                &bad_patch.id.to_hex()[..8]
            );
            println!();
            println!("  Step: test commit {}", midpoint.id.to_hex());
            println!("  {}", midpoint.message.lines().next().unwrap_or(""));
            println!();
            println!("To test this commit:");
            println!("  suture reset {} --hard", midpoint.id.to_hex());
            println!();
            println!("Then mark as:");
            if midpoint_idx > newer_idx + 1 {
                println!(
                    "  suture bisect start {} {}   (if this commit is GOOD)",
                    good_ref,
                    &midpoint.id.to_hex()[..8]
                );
            } else {
                println!("  First bad commit found: {}", midpoint.id.to_hex());
            }
            if midpoint_idx < older_idx - 1 {
                println!(
                    "  suture bisect start {} {}   (if this commit is BAD)",
                    &midpoint.id.to_hex()[..8],
                    bad_ref
                );
            } else {
                println!("  First bad commit found: {}", midpoint.id.to_hex());
            }
        }
        BisectAction::Reset => {
            let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
            let (branch_name, _) = repo.head().map_err(|e| e.to_string())?;
            println!("Bisect reset. You are on branch '{}'.", branch_name);
        }
        BisectAction::Run {
            good: good_ref,
            bad: bad_ref,
            cmd,
        } => {
            if cmd.is_empty() {
                return Err("bisect run requires a command to execute".into());
            }

            let repo_path = std::path::Path::new(".");

            // Save the branch name for restoration
            let (original_branch, original_head) = {
                let repo = suture_core::repository::Repository::open(repo_path)?;
                repo.head()?
            };

            // Resolve refs and get the full ordered log BEFORE any modifications
            // Use patch_chain (first-parent) for deterministic ordering
            let (ordered_log, good_idx, bad_idx) = {
                let repo = suture_core::repository::Repository::open(repo_path)?;
                let all_patches = repo.all_patches();

                let good_patch = resolve_ref(&repo, good_ref, &all_patches)?;
                let bad_patch = resolve_ref(&repo, bad_ref, &all_patches)?;

                // Use log (first-parent chain) for deterministic ordering by ancestry
                let log = repo.log(None)?;

                let good_idx = log
                    .iter()
                    .position(|p| p.id == good_patch.id)
                    .ok_or_else(|| format!("'{}' not found in history", good_ref))?;
                let bad_idx = log
                    .iter()
                    .position(|p| p.id == bad_patch.id)
                    .ok_or_else(|| format!("'{}' not found in history", bad_ref))?;

                // Verify ancestry
                let bad_ancestors = repo.dag().ancestors(&bad_patch.id);
                if !bad_ancestors.contains(&good_patch.id) && good_patch.id != bad_patch.id {
                    return Err("'good' must be an ancestor of 'bad'".into());
                }

                (log, good_idx, bad_idx)
            };

            // Determine older/newer indices (log is newest first, so higher index = older)
            let (older_idx, newer_idx) = if good_idx > bad_idx {
                (good_idx, bad_idx) // good is older (higher idx), bad is newer (lower idx)
            } else {
                (bad_idx, good_idx) // bad is older, good is newer
            };

            // Extract the program and arguments
            let program = &cmd[0];
            let args = &cmd[1..];

            println!(
                "bisect run '{}' with good={} bad={}",
                cmd.join(" "),
                &ordered_log[older_idx].id.to_hex()[..8],
                &ordered_log[newer_idx].id.to_hex()[..8]
            );
            println!();

            let mut current_good = older_idx;
            let mut current_bad = newer_idx;
            let mut step = 0u32;

            loop {
                step += 1;
                // current_good > current_bad (higher index = older commit)
                let remaining = current_good.saturating_sub(current_bad + 1);

                if remaining == 0 {
                    // Only one commit between good and bad — that's the first bad commit
                    // The first bad is one step newer than the last known good
                    let first_bad = &ordered_log[current_good - 1];
                    println!("✓ First bad commit found after {} step(s):", step);
                    println!(
                        "  {} {}",
                        first_bad.id.to_hex(),
                        first_bad.message.lines().next().unwrap_or("(no message)")
                    );
                    break;
                }

                let midpoint_idx = (current_good + current_bad) / 2;
                let midpoint = &ordered_log[midpoint_idx];

                // Reset to the midpoint commit
                {
                    let mut repo = suture_core::repository::Repository::open(repo_path)?;
                    repo.reset(
                        &midpoint.id.to_hex(),
                        suture_core::repository::ResetMode::Hard,
                    )?;
                }

                println!(
                    "[step {}] Testing {} ({} remaining)...",
                    step,
                    &midpoint.id.to_hex()[..8],
                    remaining
                );

                // Run the test command
                let result = std::process::Command::new(program)
                    .args(args)
                    .current_dir(repo_path)
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .status();

                match result {
                    Ok(status) => {
                        let is_good = status.success();
                        if is_good {
                            println!("  → GOOD (exit 0)");
                            // Midpoint is good; bad commit must be newer (lower index)
                            current_good = midpoint_idx - 1;
                        } else {
                            let code = status.code().unwrap_or(1);
                            println!("  → BAD (exit {})", code);
                            // Midpoint is bad; good commit must be older (higher index)
                            current_bad = midpoint_idx + 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("  → Command failed to execute: {}", e);
                        eprintln!("  Aborting bisect run.");
                        break;
                    }
                }
                println!();
            }

            // Restore the original branch to its original state
            let mut repo = suture_core::repository::Repository::open(repo_path)?;
            repo.reset(
                &original_head.to_hex(),
                suture_core::repository::ResetMode::Hard,
            )?;
            let _ = repo.checkout(&original_branch);
        }
    }

    Ok(())
}

async fn cmd_shortlog(
    branch: Option<&str>,
    number: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let mut patches = repo.log(branch)?;

    if let Some(n) = number {
        patches.truncate(n);
    }

    let mut by_author: std::collections::BTreeMap<String, Vec<&suture_core::patch::types::Patch>> =
        std::collections::BTreeMap::new();
    for patch in &patches {
        by_author
            .entry(patch.author.clone())
            .or_default()
            .push(patch);
    }

    for (author, commits) in &by_author {
        let count = commits.len();
        let short_hash = commits
            .last()
            .map(|p| p.id.to_hex().chars().take(8).collect::<String>())
            .unwrap_or_default();
        let first_msg = commits
            .first()
            .map(|p| p.message.trim().lines().next().unwrap_or(""))
            .unwrap_or("");
        println!("{} ({}) {} {}", short_hash, count, author, first_msg);
    }

    Ok(())
}

async fn cmd_notes(action: &NotesAction) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    match action {
        NotesAction::Add { commit, message } => {
            let target = resolve_ref(&repo, commit, &patches)?;
            let patch_id = target.id;
            let msg = message.clone().unwrap_or_else(|| {
                eprintln!("Enter note (Ctrl+D to finish):");
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf).unwrap_or_default();
                buf.trim_end().to_string()
            });
            repo.add_note(&patch_id, &msg)?;
            println!("Note added to {}", commit);
        }
        NotesAction::List { commit } | NotesAction::Show { commit } => {
            let target = resolve_ref(&repo, commit, &patches)?;
            let patch_id = target.id;
            let notes = repo.list_notes(&patch_id)?;
            if notes.is_empty() {
                println!("No notes for commit {}.", commit);
            } else {
                for (i, note) in notes.iter().enumerate() {
                    println!("Note {}: {}", i, note);
                }
            }
        }
        NotesAction::Remove { commit, index } => {
            let target = resolve_ref(&repo, commit, &patches)?;
            let patch_id = target.id;
            repo.remove_note(&patch_id, *index)?;
            println!("Removed note {} from {}", index, commit);
        }
    }
    Ok(())
}

async fn cmd_show(commit_ref: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit_ref, &patches)?;

    println!("commit {}", target.id.to_hex());
    println!("Author: {}", target.author);
    println!("Date:    {}", format_timestamp(target.timestamp));
    println!();
    println!("    {}", target.message);

    if !target.payload.is_empty()
        && let Some(path) = &target.target_path
    {
        println!("\n  {} {}", target.operation_type, path);
    }

    if !target.parent_ids.is_empty() {
        print!("\nParents:");
        for pid in &target.parent_ids {
            print!(" {}", pid);
        }
        println!();
    }

    Ok(())
}

async fn cmd_reflog() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let entries = repo.reflog_entries()?;
    if entries.is_empty() {
        println!("No reflog entries.");
        return Ok(());
    }
    for (head_hash, entry) in entries.iter().rev() {
        let short_hash = if head_hash.len() >= 8 {
            &head_hash[..8]
        } else {
            head_hash
        };
        println!("{} {}", short_hash, entry);
    }
    Ok(())
}

async fn cmd_drivers() -> Result<(), Box<dyn std::error::Error>> {
    use suture_driver::DriverRegistry;
    use suture_driver_csv::CsvDriver;
    use suture_driver_docx::DocxDriver;
    use suture_driver_json::JsonDriver;
    use suture_driver_pptx::PptxDriver;
    use suture_driver_toml::TomlDriver;
    use suture_driver_xlsx::XlsxDriver;
    use suture_driver_xml::XmlDriver;
    use suture_driver_yaml::YamlDriver;

    let mut registry = DriverRegistry::new();
    registry.register(Box::new(JsonDriver));
    registry.register(Box::new(TomlDriver));
    registry.register(Box::new(CsvDriver));
    registry.register(Box::new(YamlDriver));
    registry.register(Box::new(XmlDriver));
    registry.register(Box::new(DocxDriver));
    registry.register(Box::new(XlsxDriver));
    registry.register(Box::new(PptxDriver));

    let drivers = registry.list();
    if drivers.is_empty() {
        println!("No semantic drivers available.");
    } else {
        for (name, extensions) in &drivers {
            println!("{} ({})", name, extensions.join(", "));
        }
    }
    Ok(())
}

async fn cmd_cherry_pick(commit: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit, &patches)?;
    let patch_id = target.id;

    // Run pre-cherry-pick hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_BRANCH".to_string(), branch);
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    extra.insert("SUTURE_CHERRY_PICK_TARGET".to_string(), patch_id.to_hex());
    run_hook_if_exists(repo.root(), "pre-cherry-pick", extra)?;

    let new_id = repo.cherry_pick(&patch_id)?;
    println!("Cherry-picked {} as {}", commit, new_id);
    Ok(())
}

async fn cmd_rebase(branch: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    // Run pre-rebase hook
    let (current_branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_string(), current_branch.clone());
    pre_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    pre_extra.insert("SUTURE_REBASE_ONTO".to_string(), branch.to_string());
    run_hook_if_exists(repo.root(), "pre-rebase", pre_extra)?;

    let result = repo.rebase(branch)?;
    if result.patches_replayed > 0 {
        println!(
            "Rebase onto '{}': {} patch(es) replayed",
            branch, result.patches_replayed
        );
    } else {
        println!("Already up to date.");
    }

    // Run post-rebase hook
    let (branch_after, head_after) = repo.head()?;
    let mut post_extra = std::collections::HashMap::new();
    post_extra.insert("SUTURE_BRANCH".to_string(), branch_after);
    post_extra.insert("SUTURE_HEAD".to_string(), head_after.to_hex());
    post_extra.insert("SUTURE_REBASE_ONTO".to_string(), branch.to_string());
    run_hook_if_exists(repo.root(), "post-rebase", post_extra)?;

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
    println!("Hint: run `suture config user.name=\"Your Name\"` to set your identity");
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
    let staged_paths: std::collections::HashSet<&str> = status
        .staged_files
        .iter()
        .map(|(p, _)| p.as_str())
        .collect();

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

async fn cmd_rm(paths: &[String], cached: bool) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    for path in paths {
        if !cached {
            let file_path = std::path::Path::new(path);
            if file_path.exists() {
                std::fs::remove_file(file_path)?;
            }
        }
        if cached {
            let repo_path = suture_common::RepoPath::new(path)?;
            repo.meta()
                .working_set_add(&repo_path, suture_common::FileStatus::Deleted)?;
        } else {
            repo.add(path)?;
        }
        println!("Removed {}", path);
    }
    Ok(())
}

async fn cmd_mv(source: &str, destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    repo.rename_file(source, destination)?;
    println!("Renamed {} -> {}", source, destination);
    Ok(())
}

async fn cmd_commit(message: &str, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    if all {
        let count = repo.add_all()?;
        if count > 0 {
            println!("Staged {} file(s)", count);
        }
    }

    // Run pre-commit hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let author = repo
        .get_config("user.name")
        .unwrap_or(None)
        .unwrap_or_default();
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_AUTHOR".to_string(), author);
    extra.insert("SUTURE_BRANCH".to_string(), branch.clone());
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    run_hook_if_exists(repo.root(), "pre-commit", extra)?;

    let patch_id = repo.commit(message)?;
    println!("Committed: {}", patch_id);

    // Run post-commit hook
    let (branch, head_id) = repo.head()?;
    let author = repo
        .get_config("user.name")
        .unwrap_or(None)
        .unwrap_or_default();
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_AUTHOR".to_string(), author);
    extra.insert("SUTURE_BRANCH".to_string(), branch);
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    extra.insert("SUTURE_COMMIT".to_string(), patch_id.to_hex());
    run_hook_if_exists(repo.root(), "post-commit", extra)?;

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

    let name =
        name.ok_or_else(|| "branch name required (use --list to show branches)".to_string())?;
    if delete {
        repo.delete_branch(name)?;
        println!("Deleted branch '{}'", name);
    } else {
        repo.create_branch(name, target)?;
        println!("Created branch '{}'", name);
    }
    Ok(())
}

fn parse_time_filter(s: &str) -> Result<u64, String> {
    if let Some(rest) = s.strip_suffix(" ago") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() == 2
            && let Ok(n) = parts[0].parse::<u64>()
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let seconds = match parts[1] {
                "second" | "seconds" => n,
                "minute" | "minutes" => n * 60,
                "hour" | "hours" => n * 3600,
                "day" | "days" => n * 86400,
                "week" | "weeks" => n * 86400 * 7,
                "month" | "months" => n * 86400 * 30,
                "year" | "years" => n * 86400 * 365,
                _ => return Err(format!("unknown time unit: {}", parts[1])),
            };
            return Ok(now.saturating_sub(seconds));
        }
    }

    // Try parsing as a date string (YYYY-MM-DD)
    let date_str = s.trim();
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() == 3
        && let (Ok(year), Ok(month), Ok(day)) = (
            parts[0].parse::<u64>(),
            parts[1].parse::<u64>(),
            parts[2].parse::<u64>(),
        )
        && (1970..=2100).contains(&year)
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
    {
        // Estimate Unix timestamp from date
        let mut ts: u64 = 0;
        let mut y = 1970;
        while y < year {
            let days_in_year =
                if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) {
                    366
                } else {
                    365
                };
            ts += days_in_year * 86400;
            y += 1;
        }
        for m in 1..month {
            ts += days_in_month(year, m) * 86400;
        }
        ts += (day - 1) * 86400;
        return Ok(ts);
    }

    Err(format!("invalid time filter: {}", s))
}

fn days_in_month(_year: u64, month: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let year = _year;
            if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[allow(clippy::too_many_arguments)]
async fn cmd_log(
    branch: Option<&str>,
    graph: bool,
    first_parent: bool,
    oneline: bool,
    author: Option<&str>,
    grep: Option<&str>,
    all: bool,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let since_ts = since.map(parse_time_filter).transpose()?;
    let until_ts = until.map(parse_time_filter).transpose()?;

    let show_graph = graph && !all;

    if !show_graph {
        let mut patches = if all {
            let branches = repo.list_branches();
            let mut seen = std::collections::HashSet::new();
            let mut all_patches = Vec::new();
            for (_, tip_id) in &branches {
                let chain = repo.dag().patch_chain(tip_id);
                for pid in &chain {
                    if seen.insert(*pid)
                        && let Some(patch) = repo.dag().get_patch(pid)
                    {
                        all_patches.push(patch.clone());
                    }
                }
            }
            all_patches.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            all_patches
        } else if first_parent {
            use suture_common::Hash;
            let _branch_name = branch.unwrap_or("HEAD");
            let (_head_branch, head_id) = repo
                .head()
                .unwrap_or_else(|_| ("main".to_string(), Hash::ZERO));
            let mut chain = Vec::new();
            let mut current = head_id;
            while current != Hash::ZERO {
                chain.push(current);
                if let Some(patch) = repo.dag().get_patch(&current) {
                    current = patch.parent_ids.first().copied().unwrap_or(Hash::ZERO);
                } else {
                    break;
                }
            }
            let mut patches = Vec::new();
            for pid in &chain {
                if let Some(patch) = repo.dag().get_patch(pid) {
                    patches.push(patch.clone());
                }
            }
            patches
        } else {
            repo.log_all(branch)?
        };

        if let Some(since) = since_ts {
            patches.retain(|p| p.timestamp >= since);
        }
        if let Some(until) = until_ts {
            patches.retain(|p| p.timestamp <= until);
        }
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
    let mut seen_messages: std::collections::HashMap<(String, u64), usize> =
        std::collections::HashMap::new();

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

    let branch_tips: std::collections::HashSet<suture_core::patch::types::PatchId> =
        branches.iter().map(|(_, id)| *id).collect();

    let tip_list: Vec<_> = branches.iter().collect();
    let mut col_assign: std::collections::HashMap<suture_core::patch::types::PatchId, usize> =
        std::collections::HashMap::new();
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

async fn cmd_checkout(
    branch: Option<&str>,
    new_branch: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    if let Some(name) = new_branch {
        let source = branch.filter(|b| *b != "HEAD");
        repo.create_branch(name, source)?;
        repo.checkout(name)?;
        println!("Created and switched to branch '{}'", name);
    } else {
        let target = branch.ok_or("no branch specified (use -b to create one)")?;
        repo.checkout(target)?;
        println!("Switched to branch '{}'", target);
    }
    Ok(())
}

async fn cmd_diff(
    from: Option<&str>,
    to: Option<&str>,
    cached: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use suture_core::engine::diff::DiffType;
    use suture_core::engine::merge::diff_lines;

    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let entries = if cached {
        repo.diff_staged()?
    } else {
        repo.diff(from, to)?
    };

    if entries.is_empty() {
        println!("No differences.");
        return Ok(());
    }

    use std::path::Path as StdPath;
    use suture_driver::DriverRegistry;
    use suture_driver_csv::CsvDriver;
    use suture_driver_docx::DocxDriver;
    use suture_driver_json::JsonDriver;
    use suture_driver_pptx::PptxDriver;
    use suture_driver_toml::TomlDriver;
    use suture_driver_xlsx::XlsxDriver;
    use suture_driver_xml::XmlDriver;
    use suture_driver_yaml::YamlDriver;

    let mut registry = DriverRegistry::new();
    registry.register(Box::new(JsonDriver));
    registry.register(Box::new(TomlDriver));
    registry.register(Box::new(CsvDriver));
    registry.register(Box::new(YamlDriver));
    registry.register(Box::new(XmlDriver));
    registry.register(Box::new(DocxDriver));
    registry.register(Box::new(XlsxDriver));
    registry.register(Box::new(PptxDriver));

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
                    let new_blob = repo
                        .cas()
                        .get_blob(new_hash)
                        .ok()
                        .or_else(|| std::fs::read(repo.root().join(&entry.path)).ok());
                    let Some(new_blob) = new_blob else {
                        println!("{ANSI_BOLD_CYAN}added {} (binary){ANSI_RESET}", entry.path);
                        continue;
                    };
                    let new_str = String::from_utf8_lossy(&new_blob);

                    if let Ok(driver) = registry.get_for_path(StdPath::new(&entry.path))
                        && let Ok(semantic) = driver.format_diff(None, &new_str)
                        && !semantic.is_empty()
                        && semantic != "no changes"
                    {
                        println!(
                            "\n{ANSI_BOLD_CYAN}--- Semantic diff for {} ---{ANSI_RESET}",
                            entry.path
                        );
                        println!("{semantic}");
                        continue;
                    }

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
                    let old_blob = repo.cas().get_blob(old_hash).ok();
                    let new_blob = repo
                        .cas()
                        .get_blob(new_hash)
                        .ok()
                        .or_else(|| std::fs::read(repo.root().join(&entry.path)).ok());
                    match (old_blob, new_blob) {
                        (Some(old_blob), Some(new_blob)) => {
                            let old_str = String::from_utf8_lossy(&old_blob);
                            let new_str = String::from_utf8_lossy(&new_blob);

                            if let Ok(driver) = registry.get_for_path(StdPath::new(&entry.path))
                                && let Ok(semantic) = driver.format_diff(Some(&old_str), &new_str)
                                && !semantic.is_empty()
                                && semantic != "no changes"
                            {
                                println!(
                                    "\n{ANSI_BOLD_CYAN}--- Semantic diff for {} ---{ANSI_RESET}",
                                    entry.path
                                );
                                println!("{semantic}");
                                continue;
                            }

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
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let target = resolve_ref(&repo, commit, &patches)?;
    let patch_id = target.id;

    // Run pre-revert hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut extra = std::collections::HashMap::new();
    extra.insert("SUTURE_BRANCH".to_string(), branch);
    extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    extra.insert("SUTURE_REVERT_TARGET".to_string(), patch_id.to_hex());
    run_hook_if_exists(repo.root(), "pre-revert", extra)?;

    let revert_id = repo.revert(&patch_id, message)?;
    println!("Reverted: {}", revert_id);
    Ok(())
}

async fn cmd_merge(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path as StdPath;
    use suture_core::repository::ConflictInfo;

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    // Run pre-merge hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_string(), branch.clone());
    pre_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    pre_extra.insert("SUTURE_MERGE_SOURCE".to_string(), source.to_string());
    run_hook_if_exists(repo.root(), "pre-merge", pre_extra)?;

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
        // Run post-merge hook (only on clean merge)
        let (branch, head_id) = repo.head()?;
        let mut post_extra = std::collections::HashMap::new();
        post_extra.insert("SUTURE_BRANCH".to_string(), branch);
        post_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
        post_extra.insert("SUTURE_MERGE_SOURCE".to_string(), source.to_string());
        run_hook_if_exists(repo.root(), "post-merge", post_extra)?;

        return Ok(());
    }

    let conflicts = result.unresolved_conflicts;
    let mut remaining: Vec<ConflictInfo> = Vec::new();
    let mut resolved_count = 0usize;

    {
        use suture_driver::DriverRegistry;
        use suture_driver_csv::CsvDriver;
        use suture_driver_docx::DocxDriver;
        use suture_driver_json::JsonDriver;
        use suture_driver_pptx::PptxDriver;
        use suture_driver_toml::TomlDriver;
        use suture_driver_xlsx::XlsxDriver;
        use suture_driver_xml::XmlDriver;
        use suture_driver_yaml::YamlDriver;

        let mut registry = DriverRegistry::new();
        registry.register(Box::new(JsonDriver));
        registry.register(Box::new(TomlDriver));
        registry.register(Box::new(CsvDriver));
        registry.register(Box::new(YamlDriver));
        registry.register(Box::new(XmlDriver));
        registry.register(Box::new(DocxDriver));
        registry.register(Box::new(XlsxDriver));
        registry.register(Box::new(PptxDriver));

        for conflict in &conflicts {
            let path = StdPath::new(&conflict.path);
            let Ok(driver) = registry.get_for_path(path) else {
                remaining.push(conflict.clone());
                continue;
            };

            let base_content = conflict
                .base_content_hash
                .and_then(|h| repo.cas().get_blob(&h).ok())
                .map(|b| String::from_utf8_lossy(&b).to_string());
            let ours_content = conflict
                .our_content_hash
                .and_then(|h| repo.cas().get_blob(&h).ok())
                .map(|b| String::from_utf8_lossy(&b).to_string());
            let theirs_content = conflict
                .their_content_hash
                .and_then(|h| repo.cas().get_blob(&h).ok())
                .map(|b| String::from_utf8_lossy(&b).to_string());

            let base_str = base_content.as_deref().unwrap_or("");
            let Some(ours_str) = ours_content.as_deref() else {
                remaining.push(conflict.clone());
                continue;
            };
            let Some(theirs_str) = theirs_content.as_deref() else {
                remaining.push(conflict.clone());
                continue;
            };

            let Ok(merged) = driver.merge(base_str, ours_str, theirs_str) else {
                remaining.push(conflict.clone());
                continue;
            };
            let Some(content) = merged else {
                remaining.push(conflict.clone());
                continue;
            };

            if let Err(e) = std::fs::write(&conflict.path, &content) {
                eprintln!(
                    "Warning: could not write resolved file '{}': {e}",
                    conflict.path
                );
                remaining.push(conflict.clone());
                continue;
            }

            if let Err(e) = repo.add(&conflict.path) {
                eprintln!(
                    "Warning: could not stage resolved file '{}': {e}",
                    conflict.path
                );
                remaining.push(conflict.clone());
                continue;
            }

            resolved_count += 1;
        }
    }

    if resolved_count > 0 {
        println!("Resolved {resolved_count} conflict(s) via semantic drivers");
    }

    if remaining.is_empty() {
        println!("All conflicts resolved via semantic drivers.");
        println!("Run `suture commit` to finalize the merge.");
    } else {
        println!("Merge has {} conflict(s):", remaining.len());
        for conflict in &remaining {
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
    annotate: bool,
    message: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if list || name.is_none() {
        let tags = repo.list_tags()?;
        if tags.is_empty() {
            println!("No tags.");
        } else {
            for (tname, target_id) in &tags {
                if let Some(msg) = repo.get_config(&format!("tag.{}.message", tname))? {
                    println!("{} (annotated)  {}  {}", tname, target_id, msg);
                } else {
                    println!("{}  {}", tname, target_id);
                }
            }
        }
        return Ok(());
    }

    let name =
        name.ok_or_else(|| "branch name required (use --list to show branches)".to_string())?;
    if delete {
        repo.delete_tag(name)?;
        let msg_key = format!("tag.{}.message", name);
        let _ = repo.meta().delete_config(&msg_key);
        println!("Deleted tag '{}'", name);
    } else {
        repo.create_tag(name, target)?;
        let target_id = repo
            .resolve_tag(name)?
            .ok_or_else(|| format!("created tag '{}', but could not resolve it", name))?;
        if annotate {
            let msg = message.ok_or_else(|| {
                eprintln!("Error: --annotate requires a message (-m)");
                std::process::exit(1);
            })?;
            repo.set_config(&format!("tag.{}.message", name), msg)?;
            println!("Tag '{}' (annotated) -> {}", name, target_id);
        } else {
            println!("Tag '{}' -> {}", name, target_id);
        }
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
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
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
        RemoteAction::Remove { name } => {
            repo.remove_remote(name)?;
            println!("Remote '{}' removed", name);
        }
        RemoteAction::Login { name } => {
            let remote_url = repo.get_remote_url(name)?;

            eprintln!("Authenticating with {}...", remote_url);

            let client = reqwest::Client::new();
            let response = client
                .post(format!("{}/auth/token", remote_url))
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!("login failed (HTTP {}): {}", status, body).into());
            }

            let body: serde_json::Value = response.json().await?;
            let token = body["token"]
                .as_str()
                .ok_or("invalid response from server")?;

            repo.set_config(&format!("remote.{}.token", name), token)?;

            eprintln!("Authentication successful. Token stored in config.");
        }
        RemoteAction::Mirror {
            url,
            repo: upstream_repo,
            name: local_name,
        } => {
            let local_repo_name = local_name.as_deref().unwrap_or(upstream_repo);

            #[derive(serde::Serialize)]
            struct MirrorSetupReq {
                repo_name: String,
                upstream_url: String,
                upstream_repo: String,
            }
            #[derive(serde::Deserialize)]
            struct MirrorSetupResp {
                success: bool,
                error: Option<String>,
                mirror_id: Option<i64>,
            }
            #[derive(serde::Serialize)]
            struct MirrorSyncReq {
                mirror_id: i64,
            }
            #[derive(serde::Deserialize)]
            struct MirrorSyncResp {
                success: bool,
                error: Option<String>,
                patches_synced: u64,
                branches_synced: u64,
            }

            let client = reqwest::Client::new();

            let setup_body = MirrorSetupReq {
                repo_name: local_repo_name.to_string(),
                upstream_url: url.clone(),
                upstream_repo: upstream_repo.clone(),
            };

            let hub_url = repo
                .get_remote_url("origin")
                .unwrap_or_else(|_| url.clone());
            let setup_resp = client
                .post(format!("{}/mirror/setup", hub_url))
                .json(&setup_body)
                .send()
                .await?;

            let setup_result: MirrorSetupResp = setup_resp.json().await?;
            if !setup_result.success {
                return Err(setup_result
                    .error
                    .unwrap_or_else(|| "mirror setup failed".to_string())
                    .into());
            }

            let mirror_id = setup_result.mirror_id.ok_or("no mirror id returned")?;
            println!("Mirror registered (id: {mirror_id}), syncing...");

            let sync_resp = client
                .post(format!("{}/mirror/sync", hub_url))
                .json(&MirrorSyncReq { mirror_id })
                .send()
                .await?;

            let sync_result: MirrorSyncResp = sync_resp.json().await?;
            if !sync_result.success {
                return Err(sync_result
                    .error
                    .unwrap_or_else(|| "mirror sync failed".to_string())
                    .into());
            }

            println!(
                "Mirror sync complete: {} patch(es), {} branch(es)",
                sync_result.patches_synced, sync_result.branches_synced
            );
        }
    }
    Ok(())
}

async fn cmd_push(remote: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let url = repo.get_remote_url(remote)?;

    check_handshake(&url).await?;

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
        repo_id: derive_repo_id(&url, remote),
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

    // Run pre-push hook
    let (branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_string(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_string(), branch);
    pre_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
    pre_extra.insert("SUTURE_PUSH_REMOTE".to_string(), remote.to_string());
    pre_extra.insert("SUTURE_PUSH_PATCHES".to_string(), patches.len().to_string());
    run_hook_if_exists(repo.root(), "pre-push", pre_extra)?;

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

            // Run post-push hook
            let (branch, head_id) = repo.head()?;
            let mut post_extra = std::collections::HashMap::new();
            post_extra.insert("SUTURE_BRANCH".to_string(), branch);
            post_extra.insert("SUTURE_HEAD".to_string(), head_id.to_hex());
            post_extra.insert("SUTURE_PUSH_REMOTE".to_string(), remote.to_string());
            post_extra.insert("SUTURE_PUSH_PATCHES".to_string(), patches.len().to_string());
            run_hook_if_exists(repo.root(), "post-push", post_extra)?;
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
    do_pull_with_depth(repo, remote, None).await
}

async fn do_pull_with_depth(
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

async fn cmd_pull(remote: &str, rebase: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if rebase {
        // Save current branch for later
        let (head_branch, head_id) = repo.head()?;
        let current_branch = head_branch.clone();

        // Fetch new patches from remote (no working tree update)
        let new_patches = do_fetch(&mut repo, remote, None).await?;

        if new_patches == 0 {
            println!("Already up to date.");
            return Ok(());
        }

        // Rebase current branch onto main (which now has remote patches)
        let (_, new_head_id) = repo.head()?;
        if new_head_id == head_id {
            // Fetch didn't move our branch — rebase onto main
            let result = repo.rebase("main")?;
            if result.patches_replayed == 0 && result.new_tip != head_id {
                println!(
                    "Fast-forward pull successful ({} new patch(es))",
                    new_patches
                );
            } else if result.patches_replayed > 0 {
                println!(
                    "Pull with rebase successful: {} new remote patch(es), {} local patch(es) rebased",
                    new_patches, result.patches_replayed
                );
            } else {
                println!("Already up to date.");
            }
        } else {
            println!("Pull successful: {} new patch(es)", new_patches);
        }

        // Ensure we're on the correct branch
        let (final_branch, _) = repo.head()?;
        if final_branch != current_branch {
            repo.checkout(&current_branch)?;
        }
    } else {
        let new_patches = do_pull(&mut repo, remote).await?;
        println!("Pull successful: {} new patch(es)", new_patches);
    }
    Ok(())
}

async fn cmd_fetch(remote: &str, depth: Option<u32>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let new_patches = do_fetch(&mut repo, remote, depth).await?;
    println!("Fetch successful: {} new patch(es)", new_patches);
    Ok(())
}

async fn cmd_clone(
    url: &str,
    dir: Option<&str>,
    depth: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let new_patches = do_pull_with_depth(&mut repo, "origin", depth).await?;

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

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&priv_path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| format!("failed to set key permissions: {}", e))?;
            }

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
