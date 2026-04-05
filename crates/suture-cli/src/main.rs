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

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Suture repository
    #[command(after_long_help = "\
EXAMPLES:
    suture init                # Initialize in current directory
    suture init my-project     # Initialize in a new directory")]
    Init {
        /// Repository path (default: current directory)
        #[arg(default_value = ".")]
        path: String,
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
    suture add --all           # Stage all modified/deleted files")]
    Add {
        /// File paths to add (ignored when --all is used)
        paths: Vec<String>,
        /// Add all files (respecting .sutureignore)
        #[arg(short, long)]
        all: bool,
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
    suture diff                # Working tree vs staging area
    suture diff --cached       # Staging area vs HEAD
    suture diff --from main    # Compare main branch to working tree
    suture diff --from main --to feature  # Compare two branches")]
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
    suture merge feature       # Merge 'feature' into current branch")]
    Merge {
        /// Source branch to merge into HEAD
        source: String,
    },
    /// Perform three-way file merge (standalone, no branch merge needed)
    #[command(after_long_help = "\
EXAMPLES:
    suture merge-file base.txt ours.txt theirs.txt
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
    suture config user.name=Alice  # Set a config value")]
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
    /// Binary search for bug-introducing commit
    Bisect {
        #[command(subcommand)]
        action: BisectAction,
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

#[derive(Subcommand)]
pub(crate) enum IgnoreAction {
    /// List current ignore patterns
    List,
    /// Check if a path matches any ignore pattern
    Check {
        /// Path to check
        path: String,
    },
}

#[derive(Subcommand)]
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

#[derive(Subcommand)]
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

#[derive(Subcommand)]
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

#[derive(Subcommand)]
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

#[derive(Subcommand)]
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

#[derive(Subcommand)]
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
        Commands::Init { path } => cmd::init::cmd_init(&path).await,
        Commands::Status => cmd::status::cmd_status().await,
        Commands::Ignore { action } => {
            let args = match action {
                IgnoreAction::List => cmd::ignore::IgnoreArgs::List,
                IgnoreAction::Check { path } => cmd::ignore::IgnoreArgs::Check { path },
            };
            cmd::ignore::cmd_ignore(&args).await
        }
        Commands::Add { paths, all } => cmd::add::cmd_add(&paths, all).await,
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
        Commands::Diff { from, to, cached } => {
            cmd::diff::cmd_diff(from.as_deref(), to.as_deref(), cached).await
        }
        Commands::Revert { commit, message } => {
            cmd::revert::cmd_revert(&commit, message.as_deref()).await
        }
        Commands::Merge { source } => cmd::merge::cmd_merge(&source).await,
        Commands::MergeFile {
            base,
            ours,
            theirs,
            label_ours,
            label_theirs,
        } => {
            cmd::merge_file::cmd_merge_file(
                &base,
                &ours,
                &theirs,
                label_ours.as_deref(),
                label_theirs.as_deref(),
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
        Commands::Config { key_value } => cmd::config::cmd_config(&key_value).await,
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
                        "unsupported shell: '{}' (supported: bash, zsh, fish, powershell, nushell)",
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
        Commands::Bisect { action } => cmd::bisect::cmd_bisect(&action).await,
        Commands::Squash { count, message } => {
            cmd::squash::cmd_squash(count, message.as_deref()).await
        }
        Commands::Version => cmd::version::cmd_version().await,
        Commands::Tui => cmd::tui::cmd_tui().await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
