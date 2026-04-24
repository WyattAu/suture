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
        /// Repository template to bootstrap from (video, document, data, report)
        #[arg(long)]
        template: Option<String>,
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
    suture log --since \"2 weeks ago\"  # Filter by date
    suture log --audit         # Export structured audit trail (compliance format)
    suture log --audit --format json    # Audit trail in JSON format")]
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
        /// Show which files changed in each commit
        #[arg(long)]
        stat: bool,
        /// Show patch content (diff) for each commit
        #[arg(long)]
        diff: bool,
        /// Export structured audit trail (compliance format)
        #[arg(long)]
        audit: bool,
        /// Output format for --audit (json, csv, text)
        #[arg(long, default_value = "text")]
        audit_format: String,
        /// Verify commit signatures
        #[arg(long)]
        verify: bool,
        /// Filter by diff status: A (added), D (deleted), M (modified), or combinations like AD
        #[arg(long)]
        diff_filter: Option<String>,
        /// Limit number of commits shown (default: 100, 0 = unlimited)
        #[arg(long, default_value_t = 100)]
        limit: usize,
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
    /// Switch to a different branch (modern alternative to checkout)
    #[command(after_long_help = "\
EXAMPLES:
    suture switch main          # Switch to 'main'
    suture switch -c feature   # Create and switch to 'feature'
    suture switch -c feat main # Create 'feat' from 'main'")]
    Switch {
        /// Branch name to switch to
        branch: Option<String>,
        /// Create a new branch before switching
        #[arg(short = 'c', long)]
        create: Option<String>,
    },
    /// Restore working tree files (modern alternative to checkout -- <path>)
    #[command(after_long_help = "\
EXAMPLES:
    suture restore file.txt          # Restore file from HEAD
    suture restore --staged file.txt # Unstage a file (restore index from HEAD)
    suture restore --source HEAD~2 file.txt  # Restore from a specific commit")]
    Restore {
        /// Restore from a specific commit (default: HEAD)
        #[arg(short, long)]
        source: Option<String>,
        /// Files to restore
        paths: Vec<String>,
        /// Restore staged files (unstage)
        #[arg(long)]
        staged: bool,
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
    suture diff --integrity HEAD~5..HEAD     # Integrity check on recent commits
    suture diff --summary                    # Human-readable change summary (no diff output)")]
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
        /// Show only names of changed files
        #[arg(long)]
        name_only: bool,
        /// Detect classification marking changes (defence/compliance)
        #[arg(long)]
        classification: bool,
        /// Human-readable change summary (no diff output)
        #[arg(long)]
        summary: bool,
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
    /// Apply a patch/diff file to the working tree
    #[command(after_long_help = "\
EXAMPLES:
    suture apply fix.patch        # Apply a patch file
    suture apply -R fix.patch     # Apply in reverse
    suture apply --stat fix.patch # Show summary only")]
    Apply {
        /// Path to the patch/diff file
        patch_file: String,
        /// Apply in reverse
        #[arg(short = 'R', long)]
        reverse: bool,
        /// Show summary instead of applying
        #[arg(long)]
        stat: bool,
    },
    /// Apply a specific commit onto the current branch
    #[command(after_long_help = "\
EXAMPLES:
    suture cherry-pick abc123  # Apply commit onto current branch
    suture cherry-pick -n abc123  # Apply without committing")]
    CherryPick {
        /// Commit hash to cherry-pick
        commit: String,
        /// Apply changes to working tree and staging area without committing
        #[arg(short = 'n', long)]
        no_commit: bool,
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
    suture blame src/main.rs          # Show line-by-line attribution at HEAD
    suture blame src/main.rs --at HEAD~3  # Show attribution as of HEAD~3")]
    Blame {
        /// File path to blame
        path: String,
        /// Blame as of a specific commit (default: HEAD)
        #[arg(long)]
        at: Option<String>,
        /// Only show lines in range (e.g., -L 10,20)
        #[arg(short = 'L', long = "lines")]
        lines: Option<String>,
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
        /// Sort tags by 'date' (newest first) or 'name' (alphabetical)
        #[arg(long)]
        sort: Option<String>,
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
        /// Stash changes before pulling and pop after
        #[arg(long)]
        autostash: bool,
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
    /// List references (branches) on a remote Hub
    #[command(after_long_help = "\
EXAMPLES:
    suture ls-remote http://localhost:50051/my-repo
    suture ls-remote origin")]
    LsRemote {
        /// Remote URL or remote name (e.g., 'origin')
        remote_or_url: String,
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
    suture show abc123         # Show specific commit
    suture show --stat HEAD    # Show with file change statistics")]
    Show {
        /// Commit hash or branch name
        commit: String,
        /// Show file change statistics
        #[arg(long)]
        stat: bool,
    },
    /// Show the reference log (HEAD movements)
    Reflog {
        /// Show full patch details for each reflog entry
        #[arg(long)]
        show: bool,
    },
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
    /// Verify commit signatures
    #[command(after_long_help = "\
EXAMPLES:
    suture verify             # Verify HEAD commit signature
    suture verify abc123      # Verify specific commit
    suture verify -v HEAD     # Show key details")]
    Verify {
        /// Commit ref to verify (default: HEAD)
        #[arg(default_value = "HEAD")]
        commit_ref: String,
        /// Show key details (author, fingerprint, patch id)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Garbage collect unreachable objects
    Gc {
        /// Show what would be pruned without actually deleting
        #[arg(long)]
        dry_run: bool,
        /// Also prune old reflog entries and repack blobs
        #[arg(long)]
        aggressive: bool,
    },
    /// Search for a pattern in tracked files
    #[command(after_long_help = "\
EXAMPLES:
    suture grep 'TODO'                  # Search working tree for 'TODO'
    suture grep --fixed-string 'foo bar'  # Literal string search (no regex)
    suture grep -i 'error'               # Case-insensitive search
    suture grep -l 'import'              # Only show matching file names
    suture grep -- '*.rs'               # Only search .rs files")]
    Grep {
        /// Search pattern (regex by default)
        pattern: String,
        /// Search in specific files or paths (default: all tracked files)
        #[arg(default_value = ".")]
        paths: Vec<String>,
        /// Case-insensitive search
        #[arg(short = 'i', long)]
        ignore_case: bool,
        /// Show only file names (not matching lines)
        #[arg(short = 'l', long)]
        files_only: bool,
        /// Show line numbers
        #[arg(short = 'n', long, default_value_t = true)]
        line_number: bool,
        /// Fixed string matching (no regex)
        #[arg(short = 'F', long)]
        fixed_string: bool,
        /// Show N lines of context around matches
        #[arg(short = 'U', long)]
        context: Option<usize>,
    },
    /// Verify repository integrity
    Fsck {
        /// Also verify blob integrity, parent chains, and branch refs
        #[arg(long)]
        full: bool,
    },
    /// Check repository health and configuration
    Doctor {
        /// Automatically fix detected issues
        #[arg(long)]
        fix: bool,
    },
    /// Inspect the tamper-evident audit log
    Audit {
        /// Verify chain integrity
        #[arg(long)]
        verify: bool,
        /// Display all entries
        #[arg(long)]
        show: bool,
        /// Show entry count
        #[arg(long)]
        count: bool,
        /// Show last N entries (default: 10)
        #[arg(long)]
        tail: Option<usize>,
    },
    /// Remove untracked files from the working tree
    #[command(after_long_help = "\
EXAMPLES:
    suture clean                # Remove all untracked files
    suture clean -n             # Preview what would be deleted
    suture clean -d             # Also remove empty untracked directories
    suture clean build/         # Only clean files under build/")]
    Clean {
        /// Show what would be deleted without actually deleting
        #[arg(short = 'n', long)]
        dry_run: bool,
        /// Also remove empty untracked directories
        #[arg(short, long)]
        dirs: bool,
        /// Paths to clean (default: all untracked)
        paths: Vec<String>,
    },
    /// Describe a commit using the nearest tag
    #[command(after_long_help = "\
EXAMPLES:
    suture describe             # Describe HEAD
    suture describe HEAD~3      # Describe a specific commit
    suture describe --all       # Search all tags")]
    Describe {
        /// Commit ref to describe (default: HEAD)
        #[arg(default_value = "HEAD")]
        commit_ref: String,
        /// Search all tags (not just annotated)
        #[arg(long)]
        all: bool,
        /// Only search tags (default behavior)
        #[arg(long)]
        tags: bool,
    },
    /// Parse revision names to hashes
    #[command(after_long_help = "\
EXAMPLES:
    suture rev-parse HEAD       # Resolve HEAD to full hash
    suture rev-parse --short HEAD  # Abbreviated hash
    suture rev-parse --verify main  # Verify ref exists")]
    RevParse {
        /// Refs to parse
        refs: Vec<String>,
        /// Output abbreviated hash
        #[arg(long)]
        short: bool,
        /// Verify the ref exists (error if not)
        #[arg(long)]
        verify: bool,
    },
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
    /// Manage repository hooks
    #[command(after_long_help = "\
EXAMPLES:
    suture hook list                # List all hooks with their scripts
    suture hook run pre-commit      # Manually trigger a hook
    suture hook edit pre-commit     # Open hook in $EDITOR")]
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Bulk classification marking scanning and compliance reporting
    Classification {
        #[command(subcommand)]
        action: ClassificationAction,
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
    /// Synchronize with remote — commit changes and push/pull
    #[command(after_long_help = "\
EXAMPLES:
    suture sync                     # Auto-commit staged+unstaged, then push/pull
    suture sync --no-push           # Auto-commit but don't push
    suture sync --pull-only         # Only pull from remote
    suture sync --message 'WIP'     # Custom commit message")]
    Sync {
        /// Auto-commit but don't push
        #[arg(long)]
        no_push: bool,
        /// Only pull from remote (don't commit or push)
        #[arg(long)]
        pull_only: bool,
        /// Custom commit message (default: auto-generated)
        #[arg(short = 'm', long)]
        message: Option<String>,
        /// Remote to sync with (default: origin)
        #[arg(default_value = "origin")]
        remote: String,
    },
    /// Launch terminal UI
    Tui,
    /// Export a clean snapshot without repository metadata
    #[command(after_long_help = "\
EXAMPLES:
    suture export ../client-delivery       # Export HEAD to a directory
    suture export ../v2 main              # Export specific branch
    suture export ../snapshot v1.0        # Export a tag
    suture export --zip ../delivery.zip   # Export as zip file
    suture export --template ./tpl --client Acme ./out
    suture export --include-meta ./full-export")]
    Export {
        /// Output directory or zip file path
        output: String,
        /// Export as zip instead of directory
        #[arg(long)]
        zip: bool,
        /// Commit ref to export (default: HEAD)
        #[arg(long)]
        at: Option<String>,
        /// Custom template directory (files to include in export)
        #[arg(long)]
        template: Option<String>,
        /// Include .suture metadata in export
        #[arg(long)]
        include_meta: bool,
        /// Client name (creates {output}/{client}/ subdirectory)
        #[arg(long)]
        client: Option<String>,
    },
    /// Generate reports about the repository
    Report {
        #[command(subcommand)]
        report_type: ReportType,
    },
    /// Batch operations for managing multiple files or clients
    Batch {
        #[command(subcommand)]
        action: BatchAction,
    },
    /// OTIO timeline operations
    Timeline {
        #[command(subcommand)]
        action: TimelineAction,
    },
    /// Create an archive of the repository
    #[command(after_long_help = "\
EXAMPLES:
    suture archive -o project.tar.gz         # Archive HEAD as tar.gz
    suture archive --format zip -o out.zip   # Archive HEAD as zip
    suture archive main -o release.tar.gz    # Archive a specific branch")]
    Archive {
        /// Commit or branch to archive (default: HEAD)
        commit: Option<String>,
        /// Output file path
        #[arg(short, long)]
        output: String,
        /// Archive format (default: auto-detect from output extension)
        #[arg(short, long)]
        format: Option<String>,
        /// Prefix directory in the archive (default: repo name)
        #[arg(long)]
        prefix: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum TimelineAction {
    /// Import an OTIO timeline into the repo
    Import {
        /// Path to .otio file to import
        file: String,
        /// Commit message (default: auto-generated from timeline metadata)
        message: Option<String>,
    },
    /// Export the current timeline to OTIO format
    Export {
        /// Output path for .otio file
        output: String,
        /// Commit ref to export from (default: HEAD)
        #[arg(long)]
        at: Option<String>,
    },
    /// Show timeline summary (clips, duration, tracks)
    Summary {
        /// Commit ref (default: HEAD)
        #[arg(long, default_value = "HEAD")]
        at: String,
    },
    /// Diff two timeline versions
    Diff {
        /// Base commit ref
        #[arg(long, default_value = "HEAD~1")]
        from: String,
        /// Target commit ref
        #[arg(long, default_value = "HEAD")]
        to: String,
        /// Show clip-level details
        #[arg(long)]
        detailed: bool,
    },
    /// List timeline-related files in the repo
    List {
        /// Only show .otio files
        #[arg(long)]
        otio_only: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum ReportType {
    /// Generate a change summary report (what changed between two refs)
    Change {
        /// From ref (default: previous tag or HEAD~10)
        #[arg(long)]
        from: Option<String>,
        /// To ref (default: HEAD)
        #[arg(long)]
        to: Option<String>,
        /// Output format (text, markdown, html)
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Output file (default: stdout)
        #[arg(long)]
        output: Option<String>,
    },
    /// Generate a contributor/activity report
    Activity {
        /// Number of days to cover (default: 30)
        #[arg(long, default_value = "30")]
        days: u64,
        /// Output format
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Generate a file statistics report
    Stats {
        /// Commit ref (default: HEAD)
        #[arg(long, default_value = "HEAD")]
        at: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum BatchAction {
    /// Stage files matching a pattern
    Stage {
        /// Glob pattern (e.g., "*.mp4", "thumbnails/*")
        pattern: String,
    },
    /// Commit files matching a pattern
    Commit {
        /// Glob pattern
        pattern: String,
        /// Commit message
        message: String,
    },
    /// Export multiple clients at once
    ExportClients {
        /// Base output directory
        output: String,
        /// Client names (space-separated)
        clients: Vec<String>,
    },
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
    /// Stash current changes (alias for push)
    #[command(
        after_long_help = "EXAMPLES:\n    suture stash save             # Stash current changes\n    suture stash save -m \"WIP\"     # Stash with message"
    )]
    Save {
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
    /// Create and checkout a new branch from a stash entry
    #[command(after_long_help = "EXAMPLES:\n    suture stash branch feature     # Create branch from latest stash\n    suture stash branch fix 2       # Create branch from stash index 2")]
    Branch {
        /// Branch name to create
        name: String,
        /// Stash index (default: 0 = latest)
        #[arg(default_value_t = 0)]
        index: usize,
    },
    /// Show stash contents
    #[command(after_long_help = "EXAMPLES:\n    suture stash show           # Show latest stash\n    suture stash show 2         # Show stash at index 2")]
    Show {
        /// Stash index (default: 0 = latest)
        #[arg(default_value_t = 0)]
        index: usize,
    },
    /// Drop all stash entries
    #[command(after_long_help = "EXAMPLES:\n    suture stash clear           # Drop all stashes\n    suture stash clear --dry-run  # Preview what would be dropped")]
    Clear {
        /// Show how many would be dropped without actually dropping
        #[arg(long)]
        dry_run: bool,
    },
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
    /// Rename a remote
    #[command(after_long_help = "EXAMPLES:\n    suture remote rename upstream origin")]
    Rename {
        /// Current remote name
        old_name: String,
        /// New name
        new_name: String,
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
        /// Append to the last note instead of creating a new one
        #[arg(short, long)]
        append: bool,
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
    /// Prune worktree entries whose directories no longer exist
    Prune,
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
    /// Manage Suture as a Git merge driver
    #[command(after_long_help = "\
EXAMPLES:
    suture git driver install    # Install Suture as a Git merge driver
    suture git driver uninstall  # Remove the Suture merge driver
    suture git driver list       # Show current driver status")]
    Driver {
        #[command(subcommand)]
        action: DriverAction,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum DriverAction {
    /// Install Suture as a Git merge driver in the current repo
    Install,
    /// Remove the Suture merge driver from the current repo
    Uninstall,
    /// Show current merge driver status
    List,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum ClassificationAction {
    /// Scan all commits for classification marking changes
    Scan {
        /// Only scan commits since this ref (default: all)
        #[arg(long)]
        since: Option<String>,
        /// Output format
        #[arg(long, default_value = "text")]
        format: String,
        /// Only show events of this type (added, removed, upgraded, downgraded)
        #[arg(long)]
        filter: Option<String>,
    },
    /// Generate classification compliance report
    Report {
        /// Output file (default: stdout)
        #[arg(long)]
        output: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum HookAction {
    /// List all configured hooks
    List,
    /// Manually execute a hook
    Run {
        /// Hook name (e.g., pre-commit, pre-push)
        name: String,
    },
    /// Create or edit a hook in $EDITOR
    Edit {
        /// Hook name (e.g., pre-commit, pre-push)
        name: String,
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
        Commands::Init { path, r#type, template } => {
            cmd::init::cmd_init(&path, r#type.as_deref(), template.as_deref()).await
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
            stat,
            diff,
            audit,
            audit_format,
            verify,
            diff_filter,
            limit,
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
                stat,
                diff,
                audit,
                &audit_format,
                verify,
                diff_filter.as_deref(),
                limit,
            )
            .await
        }
        Commands::Checkout { branch, new_branch } => {
            cmd::checkout::cmd_checkout(branch.as_deref(), new_branch.as_deref()).await
        }
        Commands::Switch { branch, create } => {
            cmd::checkout::cmd_checkout(branch.as_deref(), create.as_deref()).await
        }
        Commands::Restore {
            source,
            paths,
            staged,
        } => cmd::restore::cmd_restore(source.as_deref(), &paths, staged).await,
        Commands::Mv {
            source,
            destination,
        } => cmd::mv::cmd_mv(&source, &destination).await,
        Commands::Diff {
            from,
            to,
            cached,
            integrity,
            name_only,
            classification,
            summary,
        } => {
            cmd::diff::cmd_diff(from.as_deref(), to.as_deref(), cached, integrity, name_only, classification, summary).await
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
        Commands::Apply { patch_file, reverse, stat } => cmd::apply::cmd_apply(&patch_file, reverse, stat).await,
        Commands::CherryPick { commit, no_commit } => cmd::cherry_pick::cmd_cherry_pick(&commit, no_commit).await,
        Commands::Rebase {
            branch,
            interactive,
            resume,
            abort,
        } => cmd::rebase::cmd_rebase(&branch, interactive, resume, abort).await,
        Commands::Blame { path, at, lines } => cmd::blame::cmd_blame(&path, at.as_deref(), lines.as_deref()).await,
        Commands::Tag {
            name,
            target,
            delete,
            list,
            annotate,
            message,
            sort,
        } => {
            cmd::tag::cmd_tag(
                name.as_deref(),
                target.as_deref(),
                delete,
                list,
                annotate,
                message.as_deref(),
                sort.as_deref(),
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
        Commands::Pull { remote, rebase, autostash } => cmd::pull::cmd_pull(&remote, rebase, autostash).await,
        Commands::Fetch { remote, depth } => cmd::fetch::cmd_fetch(&remote, depth).await,
        Commands::Clone { url, dir, depth } => {
            cmd::clone::cmd_clone(&url, dir.as_deref(), depth).await
        }
        Commands::LsRemote { remote_or_url } => {
            cmd::ls_remote::cmd_ls_remote(&remote_or_url).await
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
        Commands::Show { commit, stat } => cmd::show::cmd_show(&commit, stat).await,
        Commands::Reflog { show } => cmd::reflog::cmd_reflog(show).await,
        Commands::Drivers => cmd::drivers::cmd_drivers().await,
        Commands::Shortlog { branch, number } => {
            cmd::shortlog::cmd_shortlog(branch.as_deref(), number).await
        }
        Commands::Notes { action } => cmd::notes::cmd_notes(&action).await,
        Commands::Worktree { action } => cmd::worktree::cmd_worktree(&action).await,
        Commands::Gc { dry_run, aggressive } => cmd::gc::cmd_gc(dry_run, aggressive).await,
        Commands::Grep {
            pattern,
            paths,
            ignore_case,
            files_only,
            line_number,
            fixed_string,
            context,
        } => {
            cmd::grep::cmd_grep(
                &pattern,
                &paths,
                ignore_case,
                files_only,
                line_number,
                fixed_string,
                context,
            )
            .await
        }
        Commands::Fsck { full } => cmd::fsck::cmd_fsck(full).await,
        Commands::Doctor { fix } => cmd::doctor::cmd_doctor(fix).await,
        Commands::Audit { verify, show, count, tail } => cmd::audit::cmd_audit(verify, show, count, tail).await,
        Commands::Clean { dry_run, dirs, paths } => cmd::clean::cmd_clean(dry_run, dirs, &paths).await,
        Commands::Describe { commit_ref, all, tags } => cmd::describe::cmd_describe(&commit_ref, all, tags).await,
        Commands::RevParse { refs, short, verify } => cmd::rev_parse::cmd_rev_parse(&refs, short, verify).await,
        Commands::Bisect { action } => cmd::bisect::cmd_bisect(&action).await,
        Commands::Hook { action } => {
            let hook_action = match action {
                HookAction::List => cmd::hook::HookAction::List,
                HookAction::Run { name } => cmd::hook::HookAction::Run { name: name.clone() },
                HookAction::Edit { name } => cmd::hook::HookAction::Edit { name: name.clone() },
            };
            cmd::hook::cmd_hook(&hook_action).await
        }
        Commands::Classification { action } => cmd::classification::cmd_classification(&action).await,
        Commands::Git { action } => {
            let git_action = match action {
                GitAction::Import { path } => cmd::git::GitAction::Import { path },
                GitAction::Log { path } => cmd::git::GitAction::Log { path },
                GitAction::Status { path } => cmd::git::GitAction::Status { path },
                GitAction::Driver { action } => cmd::git::GitAction::Driver {
                    action: match action {
                        DriverAction::Install => cmd::git::DriverAction::Install,
                        DriverAction::Uninstall => cmd::git::DriverAction::Uninstall,
                        DriverAction::List => cmd::git::DriverAction::List,
                    },
                },
            };
            cmd::git::cmd_git(git_action).await
        }
        Commands::Squash { count, message } => {
            cmd::squash::cmd_squash(count, message.as_deref()).await
        }
        Commands::Sync {
            remote,
            no_push,
            pull_only,
            message,
        } => {
            cmd::sync::cmd_sync(&remote, no_push, pull_only, message.as_deref()).await
        }
        Commands::Undo { steps, hard } => cmd::undo::cmd_undo(steps, hard).await,
        Commands::Verify { commit_ref, verbose } => {
            cmd::verify::cmd_verify(&commit_ref, verbose).await
        }
        Commands::Version => cmd::version::cmd_version().await,
        Commands::Tui => cmd::tui::cmd_tui().await,
        Commands::Export {
            output,
            at,
            zip,
            template,
            include_meta,
            client,
        } => {
            cmd::export::cmd_export(&output, at.as_deref(), zip, template.as_deref(), include_meta, client.as_deref()).await
        }
        Commands::Archive {
            commit,
            output,
            format,
            prefix,
        } => {
            cmd::archive::cmd_archive(
                commit.as_deref(),
                &output,
                format.as_deref(),
                prefix.as_deref(),
            )
            .await
        }
        Commands::Report { report_type } => {
            let rt = match report_type {
                ReportType::Change { from, to, format, output } => {
                    cmd::report::ReportType::Change {
                        from: from.clone(),
                        to: to.clone(),
                        format: format.clone(),
                        output: output.clone(),
                    }
                }
                ReportType::Activity { days, format } => {
                    cmd::report::ReportType::Activity {
                        days,
                        format: format.clone(),
                    }
                }
                ReportType::Stats { at } => {
                    cmd::report::ReportType::Stats {
                        at: at.clone().unwrap_or_else(|| "HEAD".to_string()),
                    }
                }
            };
            cmd::report::cmd_report(&rt).await
        }
        Commands::Batch { action } => {
            let ba = match action {
                BatchAction::Stage { pattern } => {
                    cmd::batch::BatchAction::Stage {
                        pattern: pattern.clone(),
                    }
                }
                BatchAction::Commit { pattern, message } => {
                    cmd::batch::BatchAction::Commit {
                        pattern: pattern.clone(),
                        message: message.clone(),
                    }
                }
                BatchAction::ExportClients { clients, output } => {
                    cmd::batch::BatchAction::ExportClients {
                        clients: clients.clone(),
                        output: output.clone(),
                    }
                }
            };
            cmd::batch::cmd_batch(&ba).await
        }
        Commands::Timeline { action } => cmd::timeline::cmd_timeline(&action).await,
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
pub(crate) fn cwd_guard() -> std::sync::MutexGuard<'static, ()> {
    static CWD_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    CWD_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::path::Path;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", args))
    }

    #[test]
    fn test_init_default() {
        let cli = parse(&["suture", "init"]);
        match cli.command {
            Commands::Init { path, r#type, template } => {
                assert_eq!(path, ".");
                assert!(r#type.is_none());
                assert!(template.is_none());
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_init_with_path() {
        let cli = parse(&["suture", "init", "/tmp/myrepo"]);
        match cli.command {
            Commands::Init { path, r#type, template } => {
                assert_eq!(path, "/tmp/myrepo");
                assert!(r#type.is_none());
                assert!(template.is_none());
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_init_with_type() {
        let cli = parse(&["suture", "init", "--type", "video"]);
        match cli.command {
            Commands::Init { r#type, template, .. } => {
                assert_eq!(r#type.as_deref(), Some("video"));
                assert!(template.is_none());
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_init_with_type_and_path() {
        let cli = parse(&["suture", "init", "my-project", "-t", "document"]);
        match cli.command {
            Commands::Init { path, r#type, template } => {
                assert_eq!(path, "my-project");
                assert_eq!(r#type.as_deref(), Some("document"));
                assert!(template.is_none());
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_init_with_template() {
        let cli = parse(&["suture", "init", "--template", "report"]);
        match cli.command {
            Commands::Init { template, r#type, .. } => {
                assert_eq!(template.as_deref(), Some("report"));
                assert!(r#type.is_none());
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn test_init_with_type_and_template() {
        let cli = parse(&["suture", "init", "--type", "data", "--template", "report"]);
        match cli.command {
            Commands::Init { r#type, template, .. } => {
                assert_eq!(r#type.as_deref(), Some("data"));
                assert_eq!(template.as_deref(), Some("report"));
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
                name_only,
                classification,
                summary,
            } => {
                assert!(cached);
                assert!(from.is_none());
                assert!(to.is_none());
                assert!(!integrity);
                assert!(!name_only);
                assert!(!classification);
                assert!(!summary);
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
                name_only,
                classification,
                summary,
            } => {
                assert!(!cached);
                assert_eq!(from.as_deref(), Some("HEAD~1"));
                assert_eq!(to.as_deref(), Some("HEAD"));
                assert!(!integrity);
                assert!(!name_only);
                assert!(!classification);
                assert!(!summary);
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
                NotesAction::Add { commit, message, append: _ } => {
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

    #[test]
    fn test_git_driver_install() {
        let cli = parse(&["suture", "git", "driver", "install"]);
        match cli.command {
            Commands::Git { action } => match action {
                GitAction::Driver { action } => match action {
                    DriverAction::Install => {}
                    other => panic!("expected DriverAction::Install, got {other:?}"),
                },
                other => panic!("expected GitAction::Driver, got {other:?}"),
            },
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn test_git_driver_uninstall() {
        let cli = parse(&["suture", "git", "driver", "uninstall"]);
        match cli.command {
            Commands::Git { action } => match action {
                GitAction::Driver { action } => match action {
                    DriverAction::Uninstall => {}
                    other => panic!("expected DriverAction::Uninstall, got {other:?}"),
                },
                other => panic!("expected GitAction::Driver, got {other:?}"),
            },
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn test_git_driver_list() {
        let cli = parse(&["suture", "git", "driver", "list"]);
        match cli.command {
            Commands::Git { action } => match action {
                GitAction::Driver { action } => match action {
                    DriverAction::List => {}
                    other => panic!("expected DriverAction::List, got {other:?}"),
                },
                other => panic!("expected GitAction::Driver, got {other:?}"),
            },
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn test_stash_show() {
        let cli = parse(&["suture", "stash", "show"]);
        match cli.command {
            Commands::Stash { action } => match action {
                StashAction::Show { index } => {
                    assert_eq!(index, 0);
                }
                other => panic!("expected StashAction::Show, got {other:?}"),
            },
            other => panic!("expected Stash, got {other:?}"),
        }
    }

    #[test]
    fn test_stash_show_with_index() {
        let cli = parse(&["suture", "stash", "show", "3"]);
        match cli.command {
            Commands::Stash { action } => match action {
                StashAction::Show { index } => {
                    assert_eq!(index, 3);
                }
                other => panic!("expected StashAction::Show, got {other:?}"),
            },
            other => panic!("expected Stash, got {other:?}"),
        }
    }

    #[test]
    fn test_clean_dry_run() {
        let cli = parse(&["suture", "clean", "-n"]);
        match cli.command {
            Commands::Clean { dry_run, dirs, paths } => {
                assert!(dry_run);
                assert!(!dirs);
                assert!(paths.is_empty());
            }
            other => panic!("expected Clean, got {other:?}"),
        }
    }

    #[test]
    fn test_clean_with_dirs() {
        let cli = parse(&["suture", "clean", "-d", "-n", "build/"]);
        match cli.command {
            Commands::Clean { dry_run, dirs, paths } => {
                assert!(dry_run);
                assert!(dirs);
                assert_eq!(paths, vec!["build/"]);
            }
            other => panic!("expected Clean, got {other:?}"),
        }
    }

    #[test]
    fn test_blame_line_range() {
        let cli = parse(&["suture", "blame", "-L", "10,20", "src/main.rs"]);
        match cli.command {
            Commands::Blame { path, lines, .. } => {
                assert_eq!(path, "src/main.rs");
                assert_eq!(lines.as_deref(), Some("10,20"));
            }
            other => panic!("expected Blame, got {other:?}"),
        }
    }

    #[test]
    fn test_blame_line_range_long() {
        let cli = parse(&["suture", "blame", "--lines", "5,15", "file.txt"]);
        match cli.command {
            Commands::Blame { lines, .. } => {
                assert_eq!(lines.as_deref(), Some("5,15"));
            }
            other => panic!("expected Blame, got {other:?}"),
        }
    }

    #[test]
    fn test_describe() {
        let cli = parse(&["suture", "describe"]);
        match cli.command {
            Commands::Describe { commit_ref, all, tags } => {
                assert_eq!(commit_ref, "HEAD");
                assert!(!all);
                assert!(!tags);
            }
            other => panic!("expected Describe, got {other:?}"),
        }
    }

    #[test]
    fn test_describe_with_ref() {
        let cli = parse(&["suture", "describe", "--all", "HEAD~3"]);
        match cli.command {
            Commands::Describe { commit_ref, all, .. } => {
                assert_eq!(commit_ref, "HEAD~3");
                assert!(all);
            }
            other => panic!("expected Describe, got {other:?}"),
        }
    }

    #[test]
    fn test_rev_parse() {
        let cli = parse(&["suture", "rev-parse", "HEAD"]);
        match cli.command {
            Commands::RevParse { refs, short, verify } => {
                assert_eq!(refs, vec!["HEAD"]);
                assert!(!short);
                assert!(!verify);
            }
            other => panic!("expected RevParse, got {other:?}"),
        }
    }

    #[test]
    fn test_rev_parse_short() {
        let cli = parse(&["suture", "rev-parse", "--short", "HEAD"]);
        match cli.command {
            Commands::RevParse { short, .. } => {
                assert!(short);
            }
            other => panic!("expected RevParse, got {other:?}"),
        }
    }

    #[test]
    fn test_rev_parse_verify() {
        let cli = parse(&["suture", "rev-parse", "--verify", "main"]);
        match cli.command {
            Commands::RevParse { verify, refs, .. } => {
                assert!(verify);
                assert_eq!(refs, vec!["main"]);
            }
            other => panic!("expected RevParse, got {other:?}"),
        }
    }

    #[test]
    fn test_verify_parse() {
        let cli = parse(&["suture", "verify"]);
        match cli.command {
            Commands::Verify { commit_ref, verbose } => {
                assert_eq!(commit_ref, "HEAD");
                assert!(!verbose);
            }
            other => panic!("expected Verify, got {other:?}"),
        }
    }

    #[test]
    fn test_verify_with_ref() {
        let cli = parse(&["suture", "verify", "abc123"]);
        match cli.command {
            Commands::Verify { commit_ref, verbose } => {
                assert_eq!(commit_ref, "abc123");
                assert!(!verbose);
            }
            other => panic!("expected Verify, got {other:?}"),
        }
    }

    #[test]
    fn test_verify_verbose() {
        let cli = parse(&["suture", "verify", "-v", "HEAD"]);
        match cli.command {
            Commands::Verify { commit_ref, verbose } => {
                assert_eq!(commit_ref, "HEAD");
                assert!(verbose);
            }
            other => panic!("expected Verify, got {other:?}"),
        }
    }

    #[test]
    fn test_log_verify_flag() {
        let cli = parse(&["suture", "log", "--verify"]);
        match cli.command {
            Commands::Log { verify, .. } => assert!(verify),
            other => panic!("expected Log, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_verify_unsigned_commit() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "testuser").unwrap();
        repo.set_config("user.name", "testuser").unwrap();
        let file_path = dir_path.join("hello.txt");
        std::fs::write(&file_path, "hello").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("test commit").unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::verify::cmd_verify("HEAD", false).await;
        std::env::set_current_dir(&prev).unwrap();
        assert!(result.is_ok());
        drop(dir);
    }

    #[tokio::test]
    async fn test_verify_signed_commit() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "testuser").unwrap();
        repo.set_config("user.name", "testuser").unwrap();

        let keypair = suture_core::signing::SigningKeypair::generate();
        let keys_dir = dir_path.join(".suture").join("keys");
        std::fs::create_dir_all(&keys_dir).unwrap();
        std::fs::write(keys_dir.join("default.ed25519"), keypair.private_key_bytes()).unwrap();
        repo.set_config("signing.key", "default").unwrap();

        let file_path = dir_path.join("hello.txt");
        std::fs::write(&file_path, "hello").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("test commit").unwrap();

        let (_, head_id) = repo.head().unwrap();
        let patch = repo.dag().get_patch(&head_id).unwrap();
        repo.meta().store_public_key(&patch.author, &keypair.public_key_bytes()).unwrap();

        let canonical = suture_core::signing::canonical_patch_bytes(
            &patch.operation_type.to_string(),
            &patch.touch_set.addresses(),
            &patch.target_path,
            &patch.payload,
            &patch.parent_ids,
            &patch.author,
            &patch.message,
            patch.timestamp,
        );
        let sig = keypair.sign(&canonical);
        repo.meta().store_signature(&patch.id.to_hex(), &sig.to_bytes()).unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::verify::cmd_verify("HEAD", false).await;
        std::env::set_current_dir(&prev).unwrap();
        assert!(result.is_ok());
        drop(dir);
    }

    // ── Defence Workflow Tests ──

    #[tokio::test]
    async fn test_defence_audit_trail_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Defence Analyst").unwrap();

        for (name, msg) in [
            ("requirements.txt", "Add system requirements v1.0"),
            ("test_plan.md", "Add test plan for radar module"),
            ("risk_assessment.txt", "Add risk assessment for subsystem"),
        ] {
            std::fs::write(dir_path.join(name), "content").unwrap();
            repo.add(name).unwrap();
            repo.commit(msg).unwrap();
        }

        repo.create_branch("feature/radar-v2", None).unwrap();
        repo.checkout("feature/radar-v2").unwrap();

        std::fs::write(dir_path.join("radar_v2_spec.txt"), "spec").unwrap();
        repo.add("radar_v2_spec.txt").unwrap();
        repo.commit("Add radar v2 specification").unwrap();

        std::fs::write(dir_path.join("radar_v2_spec.txt"), "spec updated").unwrap();
        repo.add("radar_v2_spec.txt").unwrap();
        repo.commit("Update radar v2 spec with signal params").unwrap();

        repo.checkout("main").unwrap();
        repo.execute_merge("feature/radar-v2").unwrap();

        let keypair = suture_core::signing::SigningKeypair::generate();
        let keys_dir = dir_path.join(".suture").join("keys");
        std::fs::create_dir_all(&keys_dir).unwrap();
        std::fs::write(keys_dir.join("default.ed25519"), keypair.private_key_bytes()).unwrap();
        repo.set_config("signing.key", "default").unwrap();

        std::fs::write(dir_path.join("signed_doc.txt"), "classified content").unwrap();
        repo.add("signed_doc.txt").unwrap();
        repo.commit("Add signed classified document").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::log::cmd_log(None, false, false, false, None, None, false, None, None, false, false, true, "text", false, None, 0).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_defence_classification_detection() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Analyst").unwrap();

        std::fs::write(dir_path.join("report.txt"), "UNCLASSIFIED\n\nReport body here.").unwrap();
        repo.add("report.txt").unwrap();
        repo.commit("Add unclassified report").unwrap();

        let (_, head_id) = repo.head().unwrap();
        let head_hex = head_id.to_hex();

        std::fs::write(dir_path.join("report.txt"), "SECRET\n\nUpdated report body.").unwrap();
        repo.add("report.txt").unwrap();
        repo.commit("Update report classification to SECRET").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::diff::cmd_diff(Some(&head_hex), None, false, false, false, true, false).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_defence_signed_commit_verify() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Defence Officer").unwrap();

        let keypair = suture_core::signing::SigningKeypair::generate();
        let keys_dir = dir_path.join(".suture").join("keys");
        std::fs::create_dir_all(&keys_dir).unwrap();
        std::fs::write(keys_dir.join("default.ed25519"), keypair.private_key_bytes()).unwrap();
        repo.set_config("signing.key", "default").unwrap();

        std::fs::write(dir_path.join("mission_brief.txt"), "Mission brief content").unwrap();
        repo.add("mission_brief.txt").unwrap();
        let patch_id = repo.commit("Add mission brief").unwrap();
        let patch = repo.dag().get_patch(&patch_id).unwrap();
        repo.meta().store_public_key(&patch.author, &keypair.public_key_bytes()).unwrap();
        let canonical = suture_core::signing::canonical_patch_bytes(
            &patch.operation_type.to_string(),
            &patch.touch_set.addresses(),
            &patch.target_path,
            &patch.payload,
            &patch.parent_ids,
            &patch.author,
            &patch.message,
            patch.timestamp,
        );
        let sig = keypair.sign(&canonical);
        repo.meta().store_signature(&patch.id.to_hex(), &sig.to_bytes()).unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::verify::cmd_verify("HEAD", false).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    // ── Film Workflow Tests ──

    #[tokio::test]
    async fn test_film_branching_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Director").unwrap();

        std::fs::write(dir_path.join("scene_01.txt"), "Scene 1: Opening shot\nCamera pans across landscape").unwrap();
        repo.add("scene_01.txt").unwrap();
        repo.commit("Add scene 01 initial version").unwrap();

        repo.create_branch("vfx/explosion", None).unwrap();
        repo.checkout("vfx/explosion").unwrap();

        std::fs::write(dir_path.join("scene_01.txt"), "Scene 1: Opening shot\nCamera pans across landscape\n[VFX: explosion at frame 240]\n[COMPOSITING: smoke overlay]").unwrap();
        repo.add("scene_01.txt").unwrap();
        repo.commit("Add VFX explosion notes to scene 01").unwrap();

        repo.checkout("main").unwrap();

        repo.create_branch("editor/cut_2", None).unwrap();
        repo.checkout("editor/cut_2").unwrap();

        std::fs::write(dir_path.join("scene_01.txt"), "Scene 1: Opening shot\nCamera pans across landscape\n[EDITOR: trim first 10 frames]\n[EDITOR: cross-dissolve to scene 02]").unwrap();
        repo.add("scene_01.txt").unwrap();
        repo.commit("Add editor cut notes to scene 01").unwrap();

        repo.checkout("main").unwrap();
        let result = repo.execute_merge("vfx/explosion");
        assert!(result.is_ok());

        drop(repo);
        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_film_blame_history() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Writer").unwrap();

        let content = "Line 1: FADE IN\nLine 2: INT. OFFICE - DAY\nLine 3: John sits at desk\nLine 4: JOHN\nLine 5: (typing)\n";
        std::fs::write(dir_path.join("script.txt"), content).unwrap();
        repo.add("script.txt").unwrap();
        repo.commit("Add initial script draft").unwrap();

        let updated = "Line 1: FADE IN\nLine 2: INT. OFFICE - DAY\nLine 3: John stands and paces\nLine 4: JOHN\nLine 5: (typing)\n";
        std::fs::write(dir_path.join("script.txt"), updated).unwrap();
        repo.add("script.txt").unwrap();
        repo.commit("Revise line 3 - change action").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result_full = cmd::blame::cmd_blame("script.txt", None, None).await;
        assert!(result_full.is_ok());

        let result_range = cmd::blame::cmd_blame("script.txt", None, Some("2,4")).await;
        assert!(result_range.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    // ── YouTube/PE Workflow Tests ──

    #[tokio::test]
    async fn test_youtube_export_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "ContentCreator").unwrap();

        std::fs::create_dir_all(dir_path.join("drafts")).unwrap();
        std::fs::write(dir_path.join("drafts/thumbnail_ideas.txt"), "Idea 1: bold text\nIdea 2: face closeup").unwrap();
        std::fs::write(dir_path.join("drafts/script_draft.md"), "# Video Script\n## Intro\nHey everyone!").unwrap();
        std::fs::write(dir_path.join("drafts/budget.csv"), "item,cost\ncamera,500\nediting,200").unwrap();
        repo.add("drafts/thumbnail_ideas.txt").unwrap();
        repo.add("drafts/script_draft.md").unwrap();
        repo.add("drafts/budget.csv").unwrap();
        repo.commit("Add video production assets").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let export_dir = dir.path().join("client_delivery");
        let result = cmd::export::cmd_export(export_dir.to_str().unwrap(), None, false, None, false, None).await;
        assert!(result.is_ok());
        assert!(export_dir.join("drafts/thumbnail_ideas.txt").exists());
        assert!(export_dir.join("drafts/script_draft.md").exists());
        assert!(export_dir.join("drafts/budget.csv").exists());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_youtube_sync_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "YouTuber").unwrap();

        std::fs::write(dir_path.join("video_notes.txt"), "Video planning notes").unwrap();
        repo.add("video_notes.txt").unwrap();
        repo.commit("Add video notes").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::sync::cmd_sync("origin", true, false, None).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_youtube_diff_summary() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Producer").unwrap();

        std::fs::write(dir_path.join("script.txt"), "Original script content").unwrap();
        std::fs::write(dir_path.join("notes.txt"), "Production notes v1").unwrap();
        repo.add("script.txt").unwrap();
        repo.add("notes.txt").unwrap();
        repo.commit("Add initial production files").unwrap();

        let (_, first_id) = repo.head().unwrap();
        let first_hex = first_id.to_hex();

        std::fs::write(dir_path.join("script.txt"), "Updated script content with changes").unwrap();
        std::fs::write(dir_path.join("notes.txt"), "Production notes v2 updated").unwrap();
        repo.add("script.txt").unwrap();
        repo.add("notes.txt").unwrap();
        repo.commit("Update production files").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::diff::cmd_diff(Some(&first_hex), Some("HEAD"), false, false, false, false, true).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    // ── General Workflow Tests ──

    #[tokio::test]
    async fn test_collaboration_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Collaborator").unwrap();

        std::fs::write(dir_path.join("README.md"), "# Project\n\nGetting started guide.").unwrap();
        repo.add("README.md").unwrap();
        repo.commit("Initial commit").unwrap();

        repo.create_branch("feature/add-auth", None).unwrap();
        repo.checkout("feature/add-auth").unwrap();

        std::fs::write(dir_path.join("auth.rs"), "fn login() { unimplemented!() }").unwrap();
        repo.add("auth.rs").unwrap();
        repo.commit("Add authentication module").unwrap();

        repo.checkout("main").unwrap();
        repo.execute_merge("feature/add-auth").unwrap();
        repo.create_tag("v1.0", None).unwrap();
        repo.set_config("tag.v1.0.message", "release 1.0").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();

        let _ = cmd::describe::cmd_describe("HEAD", false, false).await;
        let _ = cmd::verify::cmd_verify("HEAD", false).await;
        let _ = cmd::log::cmd_log(None, false, false, false, None, None, false, None, None, true, false, false, "text", false, None, 0).await;
        let _ = cmd::show::cmd_show("HEAD", true).await;

        std::fs::write(dir_path.join("README.md"), "# Project\n\nUpdated getting started.").unwrap();
        cmd::add::cmd_add(&["README.md".to_string()], false, false).await.unwrap();

        let _ = cmd::stash::cmd_stash(&StashAction::Push { message: Some("WIP readme update".to_string()) }).await;
        let _ = cmd::stash::cmd_stash(&StashAction::List).await;
        let _ = cmd::stash::cmd_stash(&StashAction::Show { index: 0 }).await;
        let _ = cmd::stash::cmd_stash(&StashAction::Pop).await;
        let _ = cmd::reflog::cmd_reflog(false).await;

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_undo_redo_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Developer").unwrap();

        for i in 1..=3 {
            let name = format!("file_{i}.txt");
            std::fs::write(dir_path.join(&name), format!("content {i}")).unwrap();
            repo.add(&name).unwrap();
            repo.commit(&format!("Add file {i}")).unwrap();
        }

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();

        let result = cmd::undo::cmd_undo(None, false).await;
        assert!(result.is_ok());
        let _ = cmd::reflog::cmd_reflog(false).await;
        let _ = cmd::doctor::cmd_doctor(false).await;
        let _ = cmd::fsck::cmd_fsck(false).await;

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_clean_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Developer").unwrap();

        std::fs::write(dir_path.join("tracked.txt"), "tracked").unwrap();
        repo.add("tracked.txt").unwrap();
        repo.commit("Add tracked file").unwrap();

        std::fs::write(dir_path.join("untracked_a.txt"), "junk a").unwrap();
        std::fs::write(dir_path.join("untracked_b.txt"), "junk b").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();

        let dry_run = cmd::clean::cmd_clean(true, false, &[]).await;
        assert!(dry_run.is_ok());
        assert!(dir_path.join("untracked_a.txt").exists());
        assert!(dir_path.join("untracked_b.txt").exists());

        let clean = cmd::clean::cmd_clean(false, false, &[]).await;
        assert!(clean.is_ok());
        assert!(!dir_path.join("untracked_a.txt").exists());
        assert!(!dir_path.join("untracked_b.txt").exists());
        assert!(dir_path.join("tracked.txt").exists());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_audit_verify_clean_chain() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "auditor").unwrap();
        repo.set_config("user.name", "auditor").unwrap();

        for i in 1..=3 {
            let name = format!("file_{}.txt", i);
            std::fs::write(dir_path.join(&name), format!("content {}", i)).unwrap();
            repo.add(&name).unwrap();
            let patch_id = repo.commit(&format!("commit {}", i)).unwrap();
            let audit_dir = repo.root().join(".suture").join("audit").join("chain.log");
            let audit = suture_core::audit::AuditLog::open(&audit_dir).unwrap();
            let details = serde_json::json!({
                "patch_id": patch_id.to_hex(),
                "message": format!("commit {}", i),
            })
            .to_string();
            audit.append("auditor", "commit", &details).unwrap();
        }

        let audit_path = dir_path.join(".suture").join("audit").join("chain.log");
        assert!(audit_path.exists());

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();

        let result = cmd::audit::cmd_audit(true, false, false, None).await;
        assert!(result.is_ok());

        let audit = suture_core::audit::AuditLog::open(&audit_path).unwrap();
        let (total, first_invalid) = audit.verify_chain().unwrap();
        assert_eq!(total, 3);
        assert!(first_invalid.is_none());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_audit_log_commits() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "committer").unwrap();
        repo.set_config("user.name", "committer").unwrap();

        let audit_path = dir_path.join(".suture").join("audit").join("chain.log");
        let audit = suture_core::audit::AuditLog::open(&audit_path).unwrap();

        for msg in ["first commit", "second commit"] {
            std::fs::write(dir_path.join("a.txt"), msg).unwrap();
            repo.add("a.txt").unwrap();
            let patch_id = repo.commit(msg).unwrap();
            let details = serde_json::json!({
                "patch_id": patch_id.to_hex(),
                "message": msg,
            })
            .to_string();
            audit.append("committer", "commit", &details).unwrap();
        }

        assert!(audit_path.exists());

        let entries = audit.entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sequence, 0);
        assert_eq!(entries[0].action, "commit");
        assert_eq!(entries[1].sequence, 1);
        assert_eq!(entries[1].action, "commit");

        drop(repo);
        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[test]
    fn test_classification_scan_parse() {
        let cli = parse(&["suture", "classification", "scan", "--format", "json"]);
        match cli.command {
            Commands::Classification { action } => match action {
                ClassificationAction::Scan { format, since, filter } => {
                    assert_eq!(format, "json");
                    assert!(since.is_none());
                    assert!(filter.is_none());
                }
                other => panic!("expected Scan, got {other:?}"),
            },
            other => panic!("expected Classification, got {other:?}"),
        }
    }

    #[test]
    fn test_classification_scan_with_since_and_filter() {
        let cli = parse(&["suture", "classification", "scan", "--since", "main", "--filter", "upgraded"]);
        match cli.command {
            Commands::Classification { action } => match action {
                ClassificationAction::Scan { since, filter, .. } => {
                    assert_eq!(since.as_deref(), Some("main"));
                    assert_eq!(filter.as_deref(), Some("upgraded"));
                }
                other => panic!("expected Scan, got {other:?}"),
            },
            other => panic!("expected Classification, got {other:?}"),
        }
    }

    #[test]
    fn test_classification_report_parse() {
        let cli = parse(&["suture", "classification", "report", "--output", "report.txt"]);
        match cli.command {
            Commands::Classification { action } => match action {
                ClassificationAction::Report { output } => {
                    assert_eq!(output.as_deref(), Some("report.txt"));
                }
                other => panic!("expected Report, got {other:?}"),
            },
            other => panic!("expected Classification, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_classification_scan() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Analyst").unwrap();

        std::fs::write(dir_path.join("report.txt"), "UNCLASSIFIED\n\nInitial report.").unwrap();
        repo.add("report.txt").unwrap();
        repo.commit("Add unclassified report").unwrap();

        std::fs::write(dir_path.join("report.txt"), "SECRET\n\nUpdated classified content.").unwrap();
        repo.add("report.txt").unwrap();
        repo.commit("Upgrade report to SECRET").unwrap();

        std::fs::write(dir_path.join("report.txt"), "TOP SECRET\n\nHighly classified content.").unwrap();
        repo.add("report.txt").unwrap();
        repo.commit("Upgrade report to TOP SECRET").unwrap();

        std::fs::write(dir_path.join("report.txt"), "Some normal text without markings").unwrap();
        repo.add("report.txt").unwrap();
        repo.commit("Remove classification markings").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();

        let result = cmd::classification::cmd_classification(
            &ClassificationAction::Scan {
                since: None,
                format: "text".to_string(),
                filter: None,
            },
        )
        .await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_classification_report() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "Analyst").unwrap();

        std::fs::write(dir_path.join("doc1.txt"), "UNCLASSIFIED\nDoc one").unwrap();
        repo.add("doc1.txt").unwrap();
        repo.commit("Add doc1").unwrap();

        std::fs::write(dir_path.join("doc1.txt"), "CONFIDENTIAL\nDoc one updated").unwrap();
        repo.add("doc1.txt").unwrap();
        repo.commit("Classify doc1 as CONFIDENTIAL").unwrap();

        std::fs::write(dir_path.join("doc2.txt"), "SECRET\nDoc two classified").unwrap();
        repo.add("doc2.txt").unwrap();
        repo.commit("Add doc2 as SECRET").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();

        let output_path = dir.path().join("compliance_report.txt");
        let result = cmd::classification::cmd_classification(
            &ClassificationAction::Report {
                output: Some(output_path.to_str().unwrap().to_string()),
            },
        )
        .await;
        assert!(result.is_ok());
        assert!(output_path.exists());
        let report_content = std::fs::read_to_string(&output_path).unwrap();
        assert!(report_content.contains("CLASSIFICATION COMPLIANCE REPORT"));
        assert!(report_content.contains("Chain of Custody"));

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[test]
    fn test_timeline_parse() {
        let cli = parse(&["suture", "timeline", "summary"]);
        match cli.command {
            Commands::Timeline { action } => match action {
                TimelineAction::Summary { at } => {
                    assert_eq!(at, "HEAD");
                }
                other => panic!("expected Summary, got {other:?}"),
            },
            other => panic!("expected Timeline, got {other:?}"),
        }
    }

    #[test]
    fn test_timeline_import_parse() {
        let cli = parse(&["suture", "timeline", "import", "my_timeline.otio", "add timeline"]);
        match cli.command {
            Commands::Timeline { action } => match action {
                TimelineAction::Import { file, message } => {
                    assert_eq!(file, "my_timeline.otio");
                    assert_eq!(message.as_deref(), Some("add timeline"));
                }
                other => panic!("expected Import, got {other:?}"),
            },
            other => panic!("expected Timeline, got {other:?}"),
        }
    }

    #[test]
    fn test_report_parse() {
        let cli = parse(&["suture", "report", "change", "--from", "main", "--to", "HEAD", "--format", "markdown"]);
        match cli.command {
            Commands::Report { report_type } => match report_type {
                ReportType::Change { from, to, format, output } => {
                    assert_eq!(from.as_deref(), Some("main"));
                    assert_eq!(to.as_deref(), Some("HEAD"));
                    assert_eq!(format, "markdown");
                    assert!(output.is_none());
                }
                other => panic!("expected ReportType::Change, got {other:?}"),
            },
            other => panic!("expected Report, got {other:?}"),
        }
    }

    #[test]
    fn test_report_activity_parse() {
        let cli = parse(&["suture", "report", "activity", "--days", "14", "--format", "text"]);
        match cli.command {
            Commands::Report { report_type } => match report_type {
                ReportType::Activity { days, format } => {
                    assert_eq!(days, 14);
                    assert_eq!(format, "text");
                }
                other => panic!("expected ReportType::Activity, got {other:?}"),
            },
            other => panic!("expected Report, got {other:?}"),
        }
    }

    #[test]
    fn test_report_stats_parse() {
        let cli = parse(&["suture", "report", "stats"]);
        match cli.command {
            Commands::Report { report_type } => match report_type {
                ReportType::Stats { at } => {
                    assert_eq!(at.as_deref(), Some("HEAD"));
                }
                other => panic!("expected ReportType::Stats, got {other:?}"),
            },
            other => panic!("expected Report, got {other:?}"),
        }
    }

    #[test]
    fn test_batch_parse() {
        let cli = parse(&["suture", "batch", "stage", "*.mp4"]);
        match cli.command {
            Commands::Batch { action } => match action {
                BatchAction::Stage { pattern } => {
                    assert_eq!(pattern, "*.mp4");
                }
                other => panic!("expected BatchAction::Stage, got {other:?}"),
            },
            other => panic!("expected Batch, got {other:?}"),
        }
    }

    #[test]
    fn test_batch_commit_parse() {
        let cli = parse(&["suture", "batch", "commit", "*.txt", "add text files"]);
        match cli.command {
            Commands::Batch { action } => match action {
                BatchAction::Commit { pattern, message } => {
                    assert_eq!(pattern, "*.txt");
                    assert_eq!(message, "add text files");
                }
                other => panic!("expected BatchAction::Commit, got {other:?}"),
            },
            other => panic!("expected Batch, got {other:?}"),
        }
    }

    #[test]
    fn test_batch_export_clients_parse() {
        let cli = parse(&["suture", "batch", "export-clients", "./deliveries", "Acme", "Beta"]);
        match cli.command {
            Commands::Batch { action } => match action {
                BatchAction::ExportClients { clients, output } => {
                    assert_eq!(output, "./deliveries");
                    assert_eq!(clients, vec!["Acme", "Beta"]);
                }
                other => panic!("expected BatchAction::ExportClients, got {other:?}"),
            },
            other => panic!("expected Batch, got {other:?}"),
        }
    }

    #[test]
    fn test_export_template_parse() {
        let cli = parse(&[
            "suture", "export", "./out", "--template", "./tpl",
            "--client", "Acme", "--include-meta", "--at", "v1.0",
        ]);
        match cli.command {
            Commands::Export {
                output,
                zip,
                at,
                template,
                include_meta,
                client,
            } => {
                assert_eq!(output, "./out");
                assert!(!zip);
                assert_eq!(at.as_deref(), Some("v1.0"));
                assert_eq!(template.as_deref(), Some("./tpl"));
                assert!(include_meta);
                assert_eq!(client.as_deref(), Some("Acme"));
            }
            other => panic!("expected Export, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_batch_stage_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let repo = suture_core::repository::Repository::init(&dir_path, "ContentCreator").unwrap();

        std::fs::write(dir_path.join("video_01.mp4"), "fake video data 1").unwrap();
        std::fs::write(dir_path.join("video_02.mp4"), "fake video data 2").unwrap();
        std::fs::write(dir_path.join("readme.txt"), "not a video").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();

        let result = cmd::batch::cmd_batch(&cmd::batch::BatchAction::Stage {
            pattern: "*.mp4".to_string(),
        })
        .await;
        assert!(result.is_ok());

        let repo = suture_core::repository::Repository::open(Path::new(".")).unwrap();
        let status = repo.status().unwrap();
        let staged_paths: Vec<&str> = status.staged_files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(staged_paths.contains(&"video_01.mp4"));
        assert!(staged_paths.contains(&"video_02.mp4"));
        assert!(!staged_paths.contains(&"readme.txt"));

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_timeline_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "editor").unwrap();
        repo.set_config("user.name", "Film Editor").unwrap();

        let otio_content = r#"{
            "OTIO_SCHEMA": "0.15.0",
            "name": "Scene 1 - Hero Shot",
            "metadata": {"code": 25},
            "tracks": [
                {"kind": "Video", "name": "V1"},
                {"kind": "Audio", "name": "A1"}
            ]
        }"#;
        std::fs::write(dir_path.join("scene.otio"), otio_content).unwrap();
        repo.add("scene.otio").unwrap();
        repo.commit("Add scene timeline").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::timeline::cmd_timeline(&TimelineAction::List { otio_only: false }).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_report_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "analyst").unwrap();
        repo.set_config("user.name", "Data Analyst").unwrap();

        std::fs::write(dir_path.join("data.csv"), "a,b\n1,2\n3,4\n").unwrap();
        repo.add("data.csv").unwrap();
        repo.commit("Add dataset").unwrap();

        std::fs::write(dir_path.join("data.csv"), "a,b\n1,2\n3,5\n").unwrap();
        repo.add("data.csv").unwrap();
        repo.commit("Fix data point").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::report::cmd_report(&cmd::report::ReportType::Stats { at: "HEAD".to_string() }).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_batch_stage_pattern_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let repo = suture_core::repository::Repository::init(&dir_path, "user").unwrap();

        for i in 0..5 {
            std::fs::write(dir_path.join(format!("file_{i}.txt")), format!("content {i}")).unwrap();
        }

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::batch::cmd_batch(&cmd::batch::BatchAction::Stage {
            pattern: "file_*.txt".to_string(),
        }).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_audit_verify_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "auditor").unwrap();
        repo.set_config("user.name", "Security Officer").unwrap();

        std::fs::write(dir_path.join("doc.txt"), "UNCLASSIFIED\nContent here\n").unwrap();
        repo.add("doc.txt").unwrap();
        let patch_id = repo.commit("Add classified document").unwrap();

        let audit_path = dir_path.join(".suture").join("audit").join("chain.log");
        let audit = suture_core::audit::AuditLog::open(&audit_path).unwrap();
        let details = serde_json::json!({
            "patch_id": patch_id.to_hex(),
            "message": "Add classified document",
        }).to_string();
        audit.append("Security Officer", "commit", &details).unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::audit::cmd_audit(true, false, false, None).await;
        assert!(result.is_ok());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }

    #[tokio::test]
    async fn test_clean_dry_run_workflow() {
        let _cwd = cwd_guard();
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let prev = std::env::current_dir().unwrap();
        let mut repo = suture_core::repository::Repository::init(&dir_path, "user").unwrap();

        std::fs::write(dir_path.join("tracked.txt"), "kept").unwrap();
        repo.add("tracked.txt").unwrap();
        repo.commit("Initial").unwrap();

        std::fs::write(dir_path.join("untracked.txt"), "will be cleaned").unwrap();
        std::fs::write(dir_path.join("temp.log"), "temporary").unwrap();

        drop(repo);
        std::env::set_current_dir(&dir_path).unwrap();
        let result = cmd::clean::cmd_clean(true, false, &[]).await;
        assert!(result.is_ok());

        assert!(dir_path.join("untracked.txt").exists());
        assert!(dir_path.join("temp.log").exists());

        std::env::set_current_dir(&prev).unwrap();
        drop(dir);
    }
}
