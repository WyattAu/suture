use clap::{CommandFactory, Parser, Subcommand};

mod cmd;
mod display;
mod driver_registry;
mod fuzzy;
mod ref_utils;
mod remote_proto;
mod style;

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

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize a new Suture repository
    #[command(after_long_help = "\
EXAMPLES:
    suture init                         # Initialize in current directory (auto-detect type)
    suture init my-project              # Initialize in a new directory
    suture init --type video            # Configure for video workflows (OTIO-aware)
    suture init --type document         # Configure for document workflows (DOCX/XLSX/PPTX-aware)
    suture init --type data             # Configure for data workflows (CSV/JSON/XML-aware)")]
    Init {
        /// Repository path (default: current directory)
        #[arg(default_value = ".")]
        path: String,
        /// Repository type: video, document, or data (default: auto-detect)
        #[arg(short, long)]
        r#type: Option<String>,
    },
    /// Show repository status
    #[command(after_long_help = "\
EXAMPLES:
    suture status              # Show working tree status")]
    Status,
    /// Inspect .sutureignore patterns
    #[command(after_long_help = "\
EXAMPLES:
    suture ignore list         # List ignore patterns
    suture ignore check foo.o  # Check if a path is ignored")]
    Ignore {
        #[command(subcommand)]
        action: IgnoreAction,
    },
    /// Add files to the staging area
    #[command(after_long_help = "\
EXAMPLES:
    suture add file.txt        # Stage a specific file
    suture add src/            # Stage all files in src/
    suture add --all           # Stage all modified/deleted files
    suture add -p              # Interactively choose which files to stage")]
    Add {
        /// File paths to add (ignored when --all is used)
        paths: Vec<String>,
        /// Add all files (respecting .sutureignore)
        #[arg(short, long)]
        all: bool,
        /// Interactively review and choose which files to stage
        #[arg(short = 'p', long)]
        patch: bool,
    },
    /// Remove files from the working tree and staging area
    #[command(after_long_help = "\
EXAMPLES:
    suture rm file.txt         # Remove file from tree and staging
    suture rm --cached file    # Remove from staging only, keep on disk")]
    Rm {
        /// File paths to remove
        paths: Vec<String>,
        /// Only remove from staging area, keep the file on disk
        #[arg(short, long)]
        cached: bool,
    },
    /// Create a commit
    #[command(after_long_help = "\
EXAMPLES:
    suture commit \"fix typo\"    # Commit staged changes
    suture commit -a \"update\"  # Auto-stage all and commit
    suture commit --all \"WIP\"  # Same as above")]
    Commit {
        /// Commit message
        message: String,
        /// Auto-stage all modified/deleted files before committing
        #[arg(short, long)]
        all: bool,
    },
    /// Branch operations
    #[command(after_long_help = "\
EXAMPLES:
    suture branch              # List branches
    suture branch --list       # List branches (explicit)
    suture branch feature      # Create branch 'feature'
    suture branch feature main # Create from specific target
    suture branch -d old-branch  # Delete a branch
    suture branch --protect main   # Protect 'main' from force-push
    suture branch --unprotect main # Unprotect 'main'")]
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
        /// Protect a branch from force-push/deletion
        #[arg(long)]
        protect: bool,
        /// Unprotect a branch
        #[arg(long)]
        unprotect: bool,
    },
    /// Show commit history
    #[command(after_long_help = "\
EXAMPLES:
    suture log                 # Show log for HEAD
    suture log --oneline       # Compact one-line format
    suture log --graph         # ASCII graph of branch topology
    suture log --all           # Show commits across all branches
    suture log --author alice  # Filter by author
    suture log --grep \"fix\"   # Filter by message pattern
    suture log --since \"2 weeks ago\"  # Filter by date")]
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
    #[command(after_long_help = "\
EXAMPLES:
    suture checkout main       # Switch to 'main' branch
    suture checkout -b feature # Create and switch to 'feature'
    suture checkout -b feat main  # Create 'feat' from 'main'")]
    Checkout {
        /// Branch name to checkout (defaults to HEAD when -b is used)
        branch: Option<String>,
        /// Create a new branch before switching
        #[arg(short = 'b', long)]
        new_branch: Option<String>,
    },
    /// Move or rename a tracked file
    #[command(after_long_help = "\
EXAMPLES:
    suture mv old.txt new.txt  # Rename a file
    suture mv file dir/        # Move file into directory")]
    Mv {
        /// Source path
        source: String,
        /// Destination path
        destination: String,
    },
    /// Show differences between commits or branches
    #[command(after_long_help = "\
EXAMPLES:
    suture diff                              # Working tree vs staging area
    suture diff --cached                     # Staging area vs HEAD
    suture diff --from main                  # Compare main branch to working tree
    suture diff --from main --to feature     # Compare two branches
    suture diff report.docx                  # DOCX semantic diff (auto-detected)
    suture diff budget.xlsx                  # XLSX semantic diff (auto-detected)
    suture diff timeline.otio                # OTIO timeline diff (auto-detected)
    suture diff photo.png                    # Image metadata diff (auto-detected)
    suture diff config.yaml                  # YAML semantic diff (auto-detected)
    suture diff --integrity                  # Supply chain integrity analysis
    suture diff --integrity HEAD~5..HEAD     # Integrity check on recent commits")]
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
        /// Show supply chain integrity analysis (entropy, risk indicators)
        #[arg(long)]
        integrity: bool,
    },
    /// Revert a commit
    #[command(after_long_help = "\
EXAMPLES:
    suture revert abc123       # Revert a commit
    suture revert abc123 -m \"revert fix\"  # With custom message")]
    Revert {
        /// Commit hash to revert
        commit: String,
        /// Custom revert message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Merge a branch into the current branch
    #[command(after_long_help = "\
EXAMPLES:
    suture merge feature       # Merge feature into current branch
    suture merge --dry-run feature  # Preview merge without modifying working tree
    suture merge -s ours feature    # Auto-resolve conflicts by keeping our version
    suture merge -s theirs feature  # Auto-resolve conflicts by keeping their version
    suture merge -s manual feature  # Leave all conflicts for manual resolution")]
    Merge {
        /// Source branch to merge into HEAD
        source: String,
        /// Preview merge without modifying the working tree
        #[arg(long)]
        dry_run: bool,
        /// Conflict resolution strategy
        ///
        /// - semantic: try semantic drivers, fall back to conflict markers (default)
        /// - ours: keep our version for all conflicts
        /// - theirs: keep their version for all conflicts
        /// - manual: leave all conflicts as conflict markers (skip semantic drivers)
        #[arg(short, long, default_value = "semantic")]
        strategy: String,
    },
    /// Perform three-way file merge (standalone, no branch merge needed)
    #[command(after_long_help = "\
EXAMPLES:
    suture merge-file base.txt ours.txt theirs.txt
    suture merge-file --driver json base.json ours.json theirs.json
    suture merge-file --driver yaml -o merged.yaml base.yaml ours.yaml theirs.yaml
    suture merge-file --label-ours HEAD --label-theirs feature base.txt ours.txt theirs.txt")]
    MergeFile {
        /// Base (ancestor) file path
        base: String,
        /// Ours (current) file path
        ours: String,
        /// Theirs (incoming) file path
        theirs: String,
        /// Label for 'ours' side in conflict markers (default: ours)
        #[arg(long)]
        label_ours: Option<String>,
        /// Label for 'theirs' side in conflict markers (default: theirs)
        #[arg(long)]
        label_theirs: Option<String>,
        /// Use a semantic merge driver (e.g., json, yaml, toml, csv, xml, markdown, docx, xlsx, pptx).
        /// Auto-detected by file extension if omitted.
        #[arg(long)]
        driver: Option<String>,
        /// Write merged result to a file instead of stdout
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Apply a specific commit onto the current branch
    #[command(after_long_help = "\
EXAMPLES:
    suture cherry-pick abc123  # Apply commit onto current branch")]
    CherryPick {
        /// Commit hash to cherry-pick
        commit: String,
    },
    /// Rebase the current branch onto another branch
    #[command(after_long_help = "\
EXAMPLES:
    suture rebase main         # Rebase current branch onto 'main'
    suture rebase -i main      # Interactive rebase onto 'main'
    suture rebase --abort      # Abort an in-progress rebase")]
    Rebase {
        /// Target branch to rebase onto
        branch: String,
        /// Interactive rebase — open editor to reorder/edit/squash commits
        #[arg(short, long)]
        interactive: bool,
        /// Continue an in-progress interactive rebase
        #[arg(long, visible_alias = "continue")]
        resume: bool,
        /// Abort an in-progress interactive rebase
        #[arg(long)]
        abort: bool,
    },
    /// Show per-line commit attribution for a file
    #[command(after_long_help = "\
EXAMPLES:
    suture blame src/main.rs   # Show line-by-line attribution")]
    Blame {
        /// File path to blame
        path: String,
    },
    /// Tag operations
    #[command(after_long_help = "\
EXAMPLES:
    suture tag                 # List tags
    suture tag --list          # List tags (explicit)
    suture tag v1.0            # Create lightweight tag
    suture tag v1.0 -m \"release 1.0\"  # Create annotated tag
    suture tag -d v0.9         # Delete a tag")]
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
    #[command(after_long_help = "\
EXAMPLES:
    suture config              # List all config
    suture config user.name    # Get a config value
    suture config user.name=Alice  # Set a config value
    suture config --global user.name=Alice  # Set global config")]
    Config {
        /// Operate on the global config (~/.config/suture/config.toml) instead of the repo config
        #[arg(long)]
        global: bool,
        /// Key to get, or key=value to set
        key_value: Vec<String>,
    },
    /// Remote operations
    Remote {
        #[command(subcommand)]
        action: RemoteAction,
    },
    /// Push patches to a remote Hub
    #[command(after_long_help = "\
EXAMPLES:
    suture push                # Push all branches to origin
    suture push --force        # Force push (skip fast-forward check)
    suture push origin feature # Push only 'feature' branch to 'origin'")]
    Push {
        /// Remote name (default: \"origin\")
        #[arg(default_value = "origin")]
        remote: String,
        /// Force push even if not fast-forward
        #[arg(long)]
        force: bool,
        /// Specific branch to push (default: all branches)
        branch: Option<String>,
    },
    /// Pull patches from a remote Hub
    #[command(after_long_help = "\
EXAMPLES:
    suture pull                # Pull and merge from origin
    suture pull --rebase       # Pull with rebase
    suture pull upstream       # Pull from a specific remote")]
    Pull {
        /// Remote name (default: "origin")
        #[arg(default_value = "origin")]
        remote: String,
        /// Rebase local commits on top of fetched remote history
        #[arg(long)]
        rebase: bool,
    },
    /// Fetch patches from a remote Hub without merging
    #[command(after_long_help = "\
EXAMPLES:
    suture fetch               # Fetch from origin
    suture fetch --depth 5     # Shallow fetch last 5 commits")]
    Fetch {
        /// Remote name (default: "origin")
        #[arg(default_value = "origin")]
        remote: String,
        /// Limit fetch to the last N commits
        #[arg(long, help = "Limit fetch to the last N commits")]
        depth: Option<u32>,
    },
    /// Clone a repository from a remote Hub
    #[command(after_long_help = "\
EXAMPLES:
    suture clone http://localhost:50051/my-repo
    suture clone http://localhost:50051/my-repo my-local-dir
    suture clone --depth 10 http://localhost:50051/my-repo")]
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
    #[command(after_long_help = "\
EXAMPLES:
    suture reset HEAD~1        # Reset to parent (mixed mode)
    suture reset abc123 --soft  # Keep changes staged
    suture reset abc123 --hard  # Discard all changes")]
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
    #[command(after_long_help = "\
EXAMPLES:
    suture completions bash > ~/.bash_completion.d/suture
    suture completions zsh > ~/.zfunc/_suture
    suture completions fish > ~/.config/fish/completions/suture.fish
    suture completions powershell > suture.ps1
    suture completions nushell | save -f ~/.cache/suture/completions.nu")]
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell, nushell)
        shell: String,
    },
    /// Show detailed information about a commit
    #[command(after_long_help = "\
EXAMPLES:
    suture show HEAD           # Show HEAD commit
    suture show abc123         # Show specific commit")]
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
    /// Manage working trees
    #[command(after_long_help = "\
EXAMPLES:
    suture worktree add ../feature   # Create worktree at ../feature
    suture worktree add hotfix -b fix   # Create 'hotfix' on new branch 'fix'
    suture worktree list             # List all worktrees
    suture worktree remove feature   # Remove worktree 'feature'")]
    Worktree {
        #[command(subcommand)]
        action: WorktreeAction,
    },
    /// Show version information
    Version,
    /// Garbage collect unreachable objects
    Gc,
    /// Verify repository integrity
    Fsck,
    /// Check repository health and configuration
    Doctor,
    /// Interact with Git repositories
    #[command(after_long_help = "\
EXAMPLES:
    suture git import ./my-project     # Import Git history into Suture
    suture git log ./my-project        # Preview Git commits to import
    suture git status ./my-project     # Show import summary")]
    Git {
        #[command(subcommand)]
        action: GitAction,
    },
    /// Binary search for bug-introducing commit
    Bisect {
        #[command(subcommand)]
        action: BisectAction,
    },
    /// Undo the last operation (commit, merge, checkout, etc.)
    ///
    /// Uses the reflog to rewind HEAD to its previous state.
    /// Unlike `reset HEAD~N`, this can undo merges, checkouts, and cherry-picks.
    #[command(after_long_help = "\
EXAMPLES:
    suture undo                # Undo the last operation (soft)
    suture undo --steps 3      # Undo the last 3 operations
    suture undo --hard         # Undo and discard working changes")]
    Undo {
        /// Number of operations to undo (default: 1)
        #[arg(short, long)]
        steps: Option<usize>,
        /// Discard working tree changes (like --hard reset)
        #[arg(long)]
        hard: bool,
    },
    /// Squash N commits into one
    #[command(after_long_help = "\
EXAMPLES:
    suture squash 3            # Squash last 3 commits
    suture squash 3 -m \"combined\"  # With custom message")]
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

#[derive(Subcommand, Debug)]
pub(crate) enum IgnoreAction {
    /// List current ignore patterns
    List,
    /// Check if a path matches any ignore pattern
    Check {
        /// Path to check
        path: String,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum KeyAction {
    /// Generate a new Ed25519 keypair
    #[command(
        after_long_help = "EXAMPLES:\n    suture key generate           # Generate default key\n    suture key generate deploy    # Generate named key"
    )]
    Generate {
        /// Key name (default: "default")
        #[arg(default_value = "default")]
        name: String,
    },
    /// List local signing keys (public keys)
    #[command(after_long_help = "EXAMPLES:\n    suture key list")]
    List,
    /// Show the public key for a named key
    #[command(
        after_long_help = "EXAMPLES:\n    suture key public             # Show default public key\n    suture key public deploy      # Show named key"
    )]
    Public {
        /// Key name (default: "default")
        #[arg(default_value = "default")]
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum StashAction {
    /// Stash current changes
    #[command(
        after_long_help = "EXAMPLES:\n    suture stash push             # Stash current changes\n    suture stash push -m \"WIP\"     # Stash with message"
    )]
    Push {
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Pop the most recent stash
    #[command(after_long_help = "EXAMPLES:\n    suture stash pop")]
    Pop,
    /// Apply a specific stash
    #[command(
        after_long_help = "EXAMPLES:\n    suture stash apply 0         # Apply the latest stash\n    suture stash apply 2         # Apply a specific stash"
    )]
    Apply { index: usize },
    /// List stashes
    #[command(after_long_help = "EXAMPLES:\n    suture stash list")]
    List,
    /// Drop a specific stash
    #[command(
        after_long_help = "EXAMPLES:\n    suture stash drop 0         # Drop the latest stash\n    suture stash drop 2         # Drop a specific stash"
    )]
    Drop { index: usize },
}

#[derive(Subcommand, Debug)]
pub(crate) enum RemoteAction {
    /// Add a remote Hub
    #[command(after_long_help = "EXAMPLES:\n    suture remote add origin http://localhost:50051")]
    Add {
        /// Remote name
        name: String,
        /// Remote URL (e.g., http://localhost:50051)
        url: String,
    },
    /// List configured remotes
    #[command(after_long_help = "EXAMPLES:\n    suture remote list")]
    List,
    /// Remove a configured remote
    #[command(after_long_help = "EXAMPLES:\n    suture remote remove upstream")]
    Remove {
        /// Remote name
        name: String,
    },
    /// Authenticate with a remote Hub and store a token
    #[command(
        after_long_help = "EXAMPLES:\n    suture remote login           # Login to origin\n    suture remote login upstream   # Login to specific remote"
    )]
    Login {
        /// Remote name (default: "origin")
        #[arg(default_value = "origin")]
        name: String,
    },
    /// Mirror a remote repository locally
    #[command(
        after_long_help = "EXAMPLES:\n    suture remote mirror http://upstream/repo upstream-name"
    )]
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

#[derive(Subcommand, Debug)]
pub(crate) enum NotesAction {
    /// Add a note to a commit
    #[command(
        after_long_help = "EXAMPLES:\n    suture notes add abc123 -m \"reviewed\"\n    suture notes add abc123       # Enter note interactively"
    )]
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

#[derive(Subcommand, Debug)]
pub(crate) enum WorktreeAction {
    /// Create a new worktree
    Add {
        /// Path for the new worktree
        path: String,
        /// Branch to checkout (default: main)
        branch: Option<String>,
        /// Create a new branch with this name
        #[arg(short, long)]
        b: Option<String>,
    },
    /// List all worktrees
    List,
    /// Remove a worktree
    Remove {
        /// Worktree name
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum BisectAction {
    /// Start a bisect session
    #[command(
        after_long_help = "EXAMPLES:\n    suture bisect start abc123 def456\n    # abc123 = known good, def456 = known bad"
    )]
    Start {
        /// Known good commit (no bug)
        good: String,
        /// Known bad commit (has bug)
        bad: String,
    },
    /// Automatically bisect using a test command
    #[command(
        after_long_help = "EXAMPLES:\n    suture bisect run abc123 def456 -- cargo test\n    # exit 0 = good, non-zero = bad"
    )]
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

#[derive(Subcommand, Debug)]
pub(crate) enum GitAction {
    /// Import Git history into the current Suture repository
    Import {
        /// Path to the Git repository to import
        path: Option<String>,
    },
    /// Show Git commits that would be imported
    Log {
        /// Path to the Git repository
        path: Option<String>,
    },
    /// Show import summary
    Status {
        /// Path to the Git repository
        path: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    // On Unix, restore default SIGPIPE handling so broken pipes terminate the
    // process silently instead of panicking with a backtrace. This matches the
    // behavior of standard Unix tools (cat, grep, etc.) when piped to `head`.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cli = Cli::parse();

    if let Some(path) = &cli.repo_path
        && let Err(e) = std::env::set_current_dir(path)
    {
        eprintln!("error: cannot change to '{}': {e}", path);
        std::process::exit(1);
    }

    let result = match cli.command {
        Commands::Init { path, r#type } => {
            cmd::init::cmd_init(&path, r#type.as_deref()).await
        }
        Commands::Status => cmd::status::cmd_status().await,
        Commands::Ignore { action } => {
            let args = match action {
                IgnoreAction::List => cmd::ignore::IgnoreArgs::List,
                IgnoreAction::Check { path } => cmd::ignore::IgnoreArgs::Check { path },
            };
            cmd::ignore::cmd_ignore(&args).await
        }
        Commands::Add { paths, all, patch } => cmd::add::cmd_add(&paths, all, patch).await,
        Commands::Rm { paths, cached } => cmd::rm::cmd_rm(&paths, cached).await,
        Commands::Commit { message, all } => cmd::commit::cmd_commit(&message, all).await,
        Commands::Branch {
            name,
            target,
            delete,
            list,
            protect,
            unprotect,
        } => {
            cmd::branch::cmd_branch(
                name.as_deref(),
                target.as_deref(),
                delete,
                list,
                protect,
                unprotect,
            )
            .await
        }
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
            cmd::log::cmd_log(
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
            cmd::checkout::cmd_checkout(branch.as_deref(), new_branch.as_deref()).await
        }
        Commands::Mv {
            source,
            destination,
        } => cmd::mv::cmd_mv(&source, &destination).await,
        Commands::Diff {
            from,
            to,
            cached,
            integrity,
        } => {
            cmd::diff::cmd_diff(from.as_deref(), to.as_deref(), cached, integrity).await
        }
        Commands::Revert { commit, message } => {
            cmd::revert::cmd_revert(&commit, message.as_deref()).await
        }
        Commands::Merge { source, dry_run, strategy } => {
            cmd::merge::cmd_merge(&source, dry_run, strategy.as_str()).await
        }
        Commands::MergeFile {
            base,
            ours,
            theirs,
            label_ours,
            label_theirs,
            driver,
            output,
        } => {
            cmd::merge_file::cmd_merge_file(
                &base,
                &ours,
                &theirs,
                label_ours.as_deref(),
                label_theirs.as_deref(),
                driver.as_deref(),
                output.as_deref(),
            )
            .await
        }
        Commands::CherryPick { commit } => cmd::cherry_pick::cmd_cherry_pick(&commit).await,
        Commands::Rebase {
            branch,
            interactive,
            resume,
            abort,
        } => cmd::rebase::cmd_rebase(&branch, interactive, resume, abort).await,
        Commands::Blame { path } => cmd::blame::cmd_blame(&path).await,
        Commands::Tag {
            name,
            target,
            delete,
            list,
            annotate,
            message,
        } => {
            cmd::tag::cmd_tag(
                name.as_deref(),
                target.as_deref(),
                delete,
                list,
                annotate,
                message.as_deref(),
            )
            .await
        }
        Commands::Config { key_value, global } => cmd::config::cmd_config(&key_value, global).await,
        Commands::Remote { action } => cmd::remote::cmd_remote(&action).await,
        Commands::Push {
            remote,
            force,
            branch,
        } => cmd::push::cmd_push(&remote, force, branch.as_deref()).await,
        Commands::Pull { remote, rebase } => cmd::pull::cmd_pull(&remote, rebase).await,
        Commands::Fetch { remote, depth } => cmd::fetch::cmd_fetch(&remote, depth).await,
        Commands::Clone { url, dir, depth } => {
            cmd::clone::cmd_clone(&url, dir.as_deref(), depth).await
        }
        Commands::Reset { target, mode } => cmd::reset::cmd_reset(&target, &mode).await,
        Commands::Key { action } => cmd::key::cmd_key(&action).await,
        Commands::Stash { action } => cmd::stash::cmd_stash(&action).await,
        Commands::Completions { shell } => {
            match shell.as_str() {
                "bash" => clap_complete::generate(
                    clap_complete::Shell::Bash,
                    &mut Cli::command(),
                    "suture",
                    &mut std::io::stdout(),
                ),
                "zsh" => clap_complete::generate(
                    clap_complete::Shell::Zsh,
                    &mut Cli::command(),
                    "suture",
                    &mut std::io::stdout(),
                ),
                "fish" => clap_complete::generate(
                    clap_complete::Shell::Fish,
                    &mut Cli::command(),
                    "suture",
                    &mut std::io::stdout(),
                ),
                "powershell" | "pwsh" => clap_complete::generate(
                    clap_complete::Shell::PowerShell,
                    &mut Cli::command(),
                    "suture",
                    &mut std::io::stdout(),
                ),
                "nushell" => clap_complete::generate(
                    clap_complete_nushell::Nushell,
                    &mut Cli::command(),
                    "suture",
                    &mut std::io::stdout(),
                ),
                _ => {
                    eprintln!(
                        "error: unsupported shell '{}' (supported: bash, zsh, fish, powershell, nushell)",
                        shell
                    );
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        Commands::Show { commit } => cmd::show::cmd_show(&commit).await,
        Commands::Reflog => cmd::reflog::cmd_reflog().await,
        Commands::Drivers => cmd::drivers::cmd_drivers().await,
        Commands::Shortlog { branch, number } => {
            cmd::shortlog::cmd_shortlog(branch.as_deref(), number).await
        }
        Commands::Notes { action } => cmd::notes::cmd_notes(&action).await,
        Commands::Worktree { action } => cmd::worktree::cmd_worktree(&action).await,
        Commands::Gc => cmd::gc::cmd_gc().await,
        Commands::Fsck => cmd::fsck::cmd_fsck().await,
        Commands::Doctor => cmd::doctor::cmd_doctor().await,
        Commands::Bisect { action } => cmd::bisect::cmd_bisect(&action).await,
        Commands::Git { action } => {
            let git_action = match action {
                GitAction::Import { path } => cmd::git::GitAction::Import { path },
                GitAction::Log { path } => cmd::git::GitAction::Log { path },
                GitAction::Status { path } => cmd::git::GitAction::Status { path },
            };
            cmd::git::cmd_git(git_action).await
        }
        Commands::Squash { count, message } => {
            cmd::squash::cmd_squash(count, message.as_deref()).await
        }
        Commands::Undo { steps, hard } => cmd::undo::cmd_undo(steps, hard).await,
        Commands::Version => cmd::version::cmd_version().await,
        Commands::Tui => cmd::tui::cmd_tui().await,
    };

    if let Err(e) = result {
        user_friendly_error(e.as_ref());
        std::process::exit(1);
    }
}

fn user_friendly_error(err: &dyn std::error::Error) {
    let msg = err.to_string();
    let clean = clean_error_message(&msg);
    eprintln!("error: {clean}");
    if let Some(hint) = error_hint(&clean) {
        eprintln!("hint: {hint}");
    }
    let mut source = err.source();
    while let Some(s) = source {
        let src_clean = clean_error_message(&s.to_string());
        if src_clean != clean {
            eprintln!("  caused by: {src_clean}");
        }
        source = s.source();
    }
}

fn clean_error_message(msg: &str) -> String {
    let mut s = msg.to_string();
    s = strip_rust_type_paths(&s);
    s = strip_rust_backtrace(&s);
    s.trim().to_string()
}

fn strip_rust_type_paths(s: &str) -> String {
    let re = regex::Regex::new(r"[a-z_][a-z0-9_]*(?:::[a-z_][a-z0-9_]*)+::[A-Z][a-zA-Z0-9]*").unwrap();
    re.replace_all(s, "…").to_string()
}

fn strip_rust_backtrace(s: &str) -> String {
    let re = regex::Regex::new(r"\s*at [^\n]+\.(rs|rlib):?\d*").unwrap();
    re.replace_all(s, "").to_string()
}

fn error_hint(msg: &str) -> Option<&'static str> {
    let lower = msg.to_lowercase();
    if lower.contains("no remote") || lower.contains("remote not found") || lower.contains("no remotes configured") {
        Some("run `suture remote add <name> <url>` to configure a remote")
    } else if lower.contains("not a suture repository") || lower.contains("not a repository") || lower.contains(".suture") {
        Some("run `suture init` to create a new repository")
    } else if lower.contains("network") || lower.contains("connection refused") || lower.contains("connect error") || lower.contains("could not resolve") || lower.contains("timeout") {
        Some("check that the remote URL is correct and the server is reachable")
    } else if lower.contains("permission") || lower.contains("denied") || lower.contains("unauthorized") {
        Some("run `suture remote login` to authenticate with the remote")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", args))
    }

    #[test]
    fn test_init_default() {
        let cli = parse(&["suture", "init"]);
        match cli.command {
            Commands::Init { path, r#type } => {
                assert_eq!(path, ".");
                assert!(r#type.is_none());
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_init_with_path() {
        let cli = parse(&["suture", "init", "/tmp/myrepo"]);
        match cli.command {
            Commands::Init { path, r#type } => {
                assert_eq!(path, "/tmp/myrepo");
                assert!(r#type.is_none());
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_init_with_type() {
        let cli = parse(&["suture", "init", "--type", "video"]);
        match cli.command {
            Commands::Init { r#type, .. } => {
                assert_eq!(r#type.as_deref(), Some("video"));
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_init_with_type_and_path() {
        let cli = parse(&["suture", "init", "my-project", "-t", "document"]);
        match cli.command {
            Commands::Init { path, r#type } => {
                assert_eq!(path, "my-project");
                assert_eq!(r#type.as_deref(), Some("document"));
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_commit_with_message() {
        let cli = parse(&["suture", "commit", "my message"]);
        match cli.command {
            Commands::Commit { message, all } => {
                assert_eq!(message, "my message");
                assert!(!all);
            }
            other => panic!("expected Commit, got {other:?}"),
        }
    }

    #[test]
    fn test_commit_with_all_flag() {
        let cli = parse(&["suture", "commit", "--all", "msg"]);
        match cli.command {
            Commands::Commit { message, all } => {
                assert_eq!(message, "msg");
                assert!(all);
            }
            other => panic!("expected Commit, got {other:?}"),
        }
    }

    #[test]
    fn test_diff_cached_flag() {
        let cli = parse(&["suture", "diff", "--cached"]);
        match cli.command {
            Commands::Diff {
                cached,
                from,
                to,
                integrity,
            } => {
                assert!(cached);
                assert!(from.is_none());
                assert!(to.is_none());
                assert!(!integrity);
            }
            other => panic!("expected Diff, got {other:?}"),
        }
    }

    #[test]
    fn test_diff_from_to() {
        let cli = parse(&["suture", "diff", "--from", "HEAD~1", "--to", "HEAD"]);
        match cli.command {
            Commands::Diff {
                cached,
                from,
                to,
                integrity,
            } => {
                assert!(!cached);
                assert_eq!(from.as_deref(), Some("HEAD~1"));
                assert_eq!(to.as_deref(), Some("HEAD"));
                assert!(!integrity);
            }
            other => panic!("expected Diff, got {other:?}"),
        }
    }

    #[test]
    fn test_log_graph() {
        let cli = parse(&["suture", "log", "--graph"]);
        match cli.command {
            Commands::Log { graph, .. } => assert!(graph),
            other => panic!("expected Log, got {other:?}"),
        }
    }

    #[test]
    fn test_log_oneline() {
        let cli = parse(&["suture", "log", "--oneline"]);
        match cli.command {
            Commands::Log { oneline, .. } => assert!(oneline),
            other => panic!("expected Log, got {other:?}"),
        }
    }

    #[test]
    fn test_branch_create() {
        let cli = parse(&["suture", "branch", "feature"]);
        match cli.command {
            Commands::Branch { name, .. } => assert_eq!(name.as_deref(), Some("feature")),
            other => panic!("expected Branch, got {other:?}"),
        }
    }

    #[test]
    fn test_branch_delete() {
        let cli = parse(&["suture", "branch", "--delete", "feature"]);
        match cli.command {
            Commands::Branch { delete, name, .. } => {
                assert!(delete);
                assert_eq!(name.as_deref(), Some("feature"));
            }
            other => panic!("expected Branch, got {other:?}"),
        }
    }

    #[test]
    fn test_tag_lightweight() {
        let cli = parse(&["suture", "tag", "v1.0"]);
        match cli.command {
            Commands::Tag { name, annotate, .. } => {
                assert_eq!(name.as_deref(), Some("v1.0"));
                assert!(!annotate);
            }
            other => panic!("expected Tag, got {other:?}"),
        }
    }

    #[test]
    fn test_tag_annotated() {
        let cli = parse(&["suture", "tag", "-a", "-m", "release", "v1.0"]);
        match cli.command {
            Commands::Tag { name, annotate, message, .. } => {
                assert_eq!(name.as_deref(), Some("v1.0"));
                assert!(annotate);
                assert_eq!(message.as_deref(), Some("release"));
            }
            other => panic!("expected Tag, got {other:?}"),
        }
    }

    #[test]
    fn test_tag_delete() {
        let cli = parse(&["suture", "tag", "--delete", "v1.0"]);
        match cli.command {
            Commands::Tag { delete, name, .. } => {
                assert!(delete);
                assert_eq!(name.as_deref(), Some("v1.0"));
            }
            other => panic!("expected Tag, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_dry_run() {
        let cli = parse(&["suture", "merge", "--dry-run", "feature"]);
        match cli.command {
            Commands::Merge { source, dry_run, strategy } => {
                assert_eq!(source, "feature");
                assert!(dry_run);
                assert_eq!(strategy, "semantic");
            }
            other => panic!("expected Merge, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_file_with_driver() {
        let cli = parse(&["suture", "merge-file", "--driver", "json", "base", "ours", "theirs", "-o", "out.json"]);
        match cli.command {
            Commands::MergeFile {
                driver, output, ..
            } => {
                assert_eq!(driver.as_deref(), Some("json"));
                assert_eq!(output.as_deref(), Some("out.json"));
            }
            other => panic!("expected MergeFile, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_file_auto_detect() {
        let cli = parse(&["suture", "merge-file", "base.json", "ours.json", "theirs.json"]);
        match cli.command {
            Commands::MergeFile {
                base, ours, theirs, driver, output, ..
            } => {
                assert_eq!(base, "base.json");
                assert_eq!(ours, "ours.json");
                assert_eq!(theirs, "theirs.json");
                assert!(driver.is_none());
                assert!(output.is_none());
            }
            other => panic!("expected MergeFile, got {other:?}"),
        }
    }

    #[test]
    fn test_stash_push() {
        let cli = parse(&["suture", "stash", "push", "-m", "work in progress"]);
        match cli.command {
            Commands::Stash { action } => match action {
                StashAction::Push { message } => {
                    assert_eq!(message.as_deref(), Some("work in progress"));
                }
                other => panic!("expected StashAction::Push, got {other:?}"),
            },
            other => panic!("expected Stash, got {other:?}"),
        }
    }

    #[test]
    fn test_stash_pop() {
        let cli = parse(&["suture", "stash", "pop"]);
        match cli.command {
            Commands::Stash { action } => match action {
                StashAction::Pop => {}
                other => panic!("expected StashAction::Pop, got {other:?}"),
            },
            other => panic!("expected Stash, got {other:?}"),
        }
    }

    #[test]
    fn test_remote_add() {
        let cli = parse(&["suture", "remote", "add", "origin", "https://example.com"]);
        match cli.command {
            Commands::Remote { action } => match action {
                RemoteAction::Add { name, url } => {
                    assert_eq!(name, "origin");
                    assert_eq!(url, "https://example.com");
                }
                other => panic!("expected RemoteAction::Add, got {other:?}"),
            },
            other => panic!("expected Remote, got {other:?}"),
        }
    }

    #[test]
    fn test_push_force() {
        let cli = parse(&["suture", "push", "--force", "origin"]);
        match cli.command {
            Commands::Push { remote, force, branch } => {
                assert_eq!(remote, "origin");
                assert!(force);
                assert!(branch.is_none());
            }
            other => panic!("expected Push, got {other:?}"),
        }
    }

    #[test]
    fn test_rebase_interactive() {
        let cli = parse(&["suture", "rebase", "-i", "main"]);
        match cli.command {
            Commands::Rebase { branch, interactive, .. } => {
                assert_eq!(branch, "main");
                assert!(interactive);
            }
            other => panic!("expected Rebase, got {other:?}"),
        }
    }

    #[test]
    fn test_config_set() {
        let cli = parse(&["suture", "config", "user.name=Alice"]);
        match cli.command {
            Commands::Config { key_value, global } => {
                assert_eq!(key_value, vec!["user.name=Alice"]);
                assert!(!global);
            }
            other => panic!("expected Config, got {other:?}"),
        }
    }

    #[test]
    fn test_notes_add() {
        let cli = parse(&["suture", "notes", "add", "HEAD", "-m", "review note"]);
        match cli.command {
            Commands::Notes { action } => match action {
                NotesAction::Add { commit, message } => {
                    assert_eq!(commit, "HEAD");
                    assert_eq!(message.as_deref(), Some("review note"));
                }
                other => panic!("expected NotesAction::Add, got {other:?}"),
            },
            other => panic!("expected Notes, got {other:?}"),
        }
    }

    #[test]
    fn test_worktree_add() {
        let cli = parse(&["suture", "worktree", "add", "../wt", "-b", "feature"]);
        match cli.command {
            Commands::Worktree { action } => match action {
                WorktreeAction::Add { path, b, .. } => {
                    assert_eq!(path, "../wt");
                    assert_eq!(b.as_deref(), Some("feature"));
                }
                other => panic!("expected WorktreeAction::Add, got {other:?}"),
            },
            other => panic!("expected Worktree, got {other:?}"),
        }
    }

    #[test]
    fn test_global_repo_path() {
        let cli = parse(&["suture", "-C", "/some/path", "status"]);
        assert_eq!(cli.repo_path.as_deref(), Some("/some/path"));
    }

    #[test]
    fn test_git_import() {
        let cli = parse(&["suture", "git", "import", "./my-project"]);
        match cli.command {
            Commands::Git { action } => match action {
                GitAction::Import { path } => assert_eq!(path.as_deref(), Some("./my-project")),
                other => panic!("expected GitAction::Import, got {other:?}"),
            },
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn test_git_import_default_path() {
        let cli = parse(&["suture", "git", "import"]);
        match cli.command {
            Commands::Git { action } => match action {
                GitAction::Import { path } => assert!(path.is_none()),
                other => panic!("expected GitAction::Import, got {other:?}"),
            },
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn test_git_log() {
        let cli = parse(&["suture", "git", "log", "."]);
        match cli.command {
            Commands::Git { action } => match action {
                GitAction::Log { path } => assert_eq!(path.as_deref(), Some(".")),
                other => panic!("expected GitAction::Log, got {other:?}"),
            },
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn test_git_status() {
        let cli = parse(&["suture", "git", "status"]);
        match cli.command {
            Commands::Git { action } => match action {
                GitAction::Status { path } => assert!(path.is_none()),
                other => panic!("expected GitAction::Status, got {other:?}"),
            },
            other => panic!("expected Git, got {other:?}"),
        }
    }
}
