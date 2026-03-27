use clap::{Parser, Subcommand};
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
    /// Compute merge plan (dry-run)
    Merge {
        /// Source branch
        branch_a: String,
        /// Target branch
        branch_b: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { path } => cmd_init(&path),
        Commands::Status => cmd_status(),
        Commands::Add { paths, all } => cmd_add(&paths, all),
        Commands::Commit { message } => cmd_commit(&message),
        Commands::Branch { name, target } => cmd_branch(&name, target.as_deref()),
        Commands::Log { branch } => cmd_log(branch.as_deref()),
        Commands::Checkout { branch } => cmd_checkout(&branch),
        Commands::Diff { from, to } => cmd_diff(from.as_deref(), to.as_deref()),
        Commands::Revert { commit, message } => cmd_revert(&commit, message.as_deref()),
        Commands::Merge { branch_a, branch_b } => cmd_merge(&branch_a, &branch_b),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn cmd_init(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = PathBuf::from(path);
    let repo = suture_core::repository::Repository::init(&repo_path, "local")?;
    println!(
        "Initialized empty Suture repository in {}",
        repo_path.display()
    );
    drop(repo);
    Ok(())
}

fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
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

fn cmd_add(paths: &[String], all: bool) -> Result<(), Box<dyn std::error::Error>> {
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

fn cmd_commit(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patch_id = repo.commit(message)?;
    println!("Committed: {}", patch_id);
    Ok(())
}

fn cmd_branch(name: &str, target: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    repo.create_branch(name, target)?;
    println!("Created branch '{}'", name);
    Ok(())
}

fn cmd_log(branch: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.log(branch)?;

    if patches.is_empty() {
        println!("No commits.");
        return Ok(());
    }

    for (i, patch) in patches.iter().enumerate() {
        if i == 0 {
            println!("* {} {}", patch.id, patch.message);
        } else {
            println!("  {} {}", patch.id, patch.message);
        }
    }

    Ok(())
}

fn cmd_checkout(branch: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    repo.checkout(branch)?;
    println!("Switched to branch '{}'", branch);
    Ok(())
}

fn cmd_diff(from: Option<&str>, to: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
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

fn cmd_revert(commit: &str, message: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let patch_id = suture_core::Hash::from_hex(commit)?;
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let revert_id = repo.revert(&patch_id, message)?;
    println!("Reverted: {}", revert_id);
    Ok(())
}

fn cmd_merge(branch_a: &str, branch_b: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let result = repo.merge_plan(branch_a, branch_b)?;

    if result.is_clean {
        println!("Merge is clean (no conflicts).");
    } else {
        println!("Merge has {} conflict(s):", result.conflicts.len());
        for conflict in &result.conflicts {
            println!("  Conflict at: {:?}", conflict.conflict_addresses);
        }
    }

    Ok(())
}
