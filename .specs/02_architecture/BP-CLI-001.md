---
document_id: BP-CLI-001
version: 1.0.0
status: APPROVED
ieee_1016_compliant: true
component_id: COMP-CLI-001
component_type: Application
interfaces: [IF-CLI-001]
depends_on:
  yellow_papers: []
  blue_papers: [BP-PATCH-ALGEBRA-001, BP-PATCH-DAG-001, BP-CAS-001, BP-METADATA-001]
  external_libs: [clap-4.5]
created: 2026-03-27
---

# BP-CLI-001: Command-Line Interface

## BP-1: Design Overview

The Suture CLI is the primary user-facing interface for interacting with Suture repositories.
It is built on the `clap` crate (REQ-CLI-002) using derive macros for argument parsing. The
CLI consumes the `suture-core` library crate and delegates all operations to the core engine.

The CLI follows Git-like conventions for familiarity: `suture <command> [options] [args]`.
Error messages are human-readable with suggested remediation actions (REQ-CLI-003).

---

## BP-2: Design Decomposition

### 2.1 Command Hierarchy

```
suture
  ├── init [path]                    # Initialize a new repository
  ├── status [--live]               # Show working set and branch status
  ├── add <path>...                 # Stage file(s) for the next commit
  ├── commit [-m <message>]         # Create a new patch from staged changes
  ├── branch                        # Branch management
  │   ├── list                      # List all branches
  │   ├── create <name>             # Create a new branch
  │   ├── delete <name>             # Delete a branch
  │   ├── rename <old> <new>        # Rename a branch
  │   └── switch <name>             # Switch to a branch
  ├── merge <branch>                # Merge a branch into the current branch
  ├── log [--oneline] [-n <count>]  # Show commit history
  ├── diff [--staged] [<path>]      # Show changes (working set or staged)
  ├── config                        # Repository configuration
  │   ├── get <key>
  │   ├── set <key> <value>
  │   └── list
  ├── key                           # Ed25519 key management
  │   ├── generate
  │   ├── list
  │   └── rotate
  └── gc                            # Run garbage collection on unreferenced blobs
```

### 2.2 Module Structure

```
suture-cli/src/
  main.rs           -- Entry point, clap App definition
  commands/
    mod.rs
    init.rs
    status.rs
    add.rs
    commit.rs
    branch.rs
    merge.rs
    log.rs
    diff.rs
    config.rs
    key.rs
    gc.rs
  error.rs          -- User-facing error formatting
```

---

## BP-3: Design Rationale

### 3.1 clap with Derive Macros

The `clap` crate with derive macros provides:
- Compile-time verification of argument types and subcommand structure.
- Auto-generated `--help` and `--version` output.
- Shell completion generation (bash, zsh, fish, PowerShell).

### 3.2 Error Formatting

All errors are formatted through a central `error.rs` module that:
- Maps internal errors to human-readable messages.
- Appends suggested remediation actions (e.g., "Run `suture init` first.").
- Supports i18n via the `fluent` crate (REQ-CLI-005, deferred to v0.2).

---

## BP-4: Traceability

| Requirement | Satisfied By | Verification Method |
|-------------|-------------|-------------------|
| REQ-CLI-001 (min commands) | BP-2.1, command hierarchy | Integration tests |
| REQ-CLI-002 (clap derive) | BP-3.1 | Compile-time |
| REQ-CLI-003 (error messages) | BP-3.2, error.rs | Manual review |
| REQ-CLI-004 (--live status) | status.rs, async loop | Integration test |
| REQ-CLI-005 (i18n) | fluent crate | Deferred |
| REQ-CLI-006 (config subcommand) | BP-2.1 | Integration test |
| REQ-CLI-007 (key subcommand) | BP-2.1 | Integration test |
| REQ-CLI-008 (timing in verbose) | --verbose flag | Unit test |

---

## BP-5: Interface Design

### IF-CLI-001: Command Definitions

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "suture", version, about = "Universal Semantic Version Control")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output (including timing information).
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Suture repository.
    Init {
        /// Directory to initialize (default: current directory).
        path: Option<PathBuf>,
    },

    /// Show working set status and current branch.
    Status {
        /// Continuously update status display.
        #[arg(long)]
        live: bool,
    },

    /// Stage file(s) for the next commit.
    Add {
        /// File or directory paths to stage.
        paths: Vec<PathBuf>,
    },

    /// Create a new patch from staged changes.
    Commit {
        /// Commit message.
        #[arg(short, long)]
        message: String,

        /// Author name (overrides config).
        #[arg(long)]
        author: Option<String>,
    },

    /// Branch management.
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },

    /// Merge a branch into the current branch.
    Merge {
        /// Branch to merge into the current branch.
        branch: String,
    },

    /// Show commit history.
    Log {
        /// Show one commit per line.
        #[arg(long)]
        oneline: bool,

        /// Maximum number of commits to show.
        #[arg(short = 'n', long, default_value = "20")]
        count: usize,
    },

    /// Show changes in the working set or staging area.
    Diff {
        /// Show staged changes instead of unstaged.
        #[arg(long)]
        staged: bool,

        /// Specific path to diff (default: all).
        path: Option<PathBuf>,
    },

    /// Repository configuration management.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Ed25519 key management.
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },

    /// Run garbage collection on unreferenced blobs.
    Gc,
}

#[derive(Subcommand)]
pub enum BranchAction {
    /// List all branches.
    List,
    /// Create a new branch.
    Create { name: String },
    /// Delete a branch.
    Delete { name: String },
    /// Rename a branch.
    Rename { old_name: String, new_name: String },
    /// Switch to a branch.
    Switch { name: String },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Get a configuration value.
    Get { key: String },
    /// Set a configuration value.
    Set { key: String, value: String },
    /// List all configuration values.
    List,
}

#[derive(Subcommand)]
pub enum KeyAction {
    /// Generate a new Ed25519 key pair.
    Generate,
    /// List all known public keys.
    List,
    /// Rotate the current key (add new, deprecate old).
    Rotate,
}
```

---

## BP-6: Data Design

The CLI has no persistent data of its own. All state is read from and written to the
core engine (suture-core) via the MetadataStore, PatchDag, and BlobStore.

---

## BP-7: Component Design

```
suture-cli/
  Cargo.toml
  src/
    main.rs           -- Cli struct, command dispatch
    commands/
      mod.rs
      init.rs         -- MetadataStore::open(), .suture/ creation
      status.rs       -- MetadataStore::get_working_set(), branch info
      add.rs          -- Driver SDK: serialize(), MetadataStore::stage()
      commit.rs       -- Patch creation, BlobStore::put_blob(), DagHandle::write()
      branch.rs       -- DagHandle::write() for branch operations
      merge.rs        -- DagHandle::merge_branches(), conflict reporting
      log.rs          -- DagHandle::topo_order(), patch formatting
      diff.rs         -- Driver SDK: visual_diff()
      config.rs       -- MetadataStore::get/set_config()
      key.rs          -- Ed25519 key generation, listing, rotation
      gc.rs           -- BlobStore::gc(), DagHandle::ancestors()
    error.rs          -- CliError formatting with remediation hints
```

---

## BP-8: Deployment

### 8.1 Binary Distribution

```toml
[package]
name = "suture-cli"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "suture"
path = "src/main.rs"

[dependencies]
suture-core = { path = "../suture-core" }
clap = { version = "4.5", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
chrono = "0.4"
thiserror = "2"
```

### 8.2 Build Targets

- Linux x86_64 (glibc 2.31+)
- Linux aarch64 (glibc 2.31+)
- macOS aarch64
- Windows x86_64 (MSVC)

---

## BP-9: Formal Verification

The CLI is a thin layer over the core engine. Correctness is ensured by:
1. **Command contract tests**: Each subcommand has an integration test verifying the
   expected side effects on the core engine.
2. **Error path tests**: Every error condition in the core engine is tested for correct
   CLI error formatting.
3. **Round-trip tests**: `init → add → commit → log → diff` produces consistent state.

---

## BP-11: Compliance Matrix

| Requirement | Section | Status | Verification |
|-------------|---------|--------|-------------|
| REQ-CLI-001 | BP-2.1 | Satisfied | Integration tests |
| REQ-CLI-002 | BP-3.1 | Satisfied | Compile-time |
| REQ-CLI-003 | BP-3.2 | Satisfied | Manual review |
| REQ-CLI-004 | BP-5, --live | Satisfied | Integration test |
| REQ-CLI-005 | fluent crate | Deferred | v0.2 |
| REQ-CLI-006 | BP-5, Config | Satisfied | Integration test |
| REQ-CLI-007 | BP-5, Key | Satisfied | Integration test |
| REQ-CLI-008 | BP-5, --verbose | Satisfied | Unit test |

---

## BP-12: Quality Checklist

- [ ] All subcommands have integration tests verifying expected behavior.
- [ ] Error messages include remediation suggestions.
- [ ] `--help` output is clear and complete for all commands.
- [ ] Shell completions generate without errors (bash, zsh, fish).
- [ ] `--verbose` flag includes command execution timing.
- [ ] `--live` status mode updates continuously and exits cleanly on Ctrl+C.
- [ ] `suture init` creates a valid `.suture/` directory structure.
- [ ] `suture status` reports correct working set and branch state.
- [ ] `suture commit` creates a valid patch in the DAG.
- [ ] `suture merge` reports conflicts when expected, succeeds when clean.
- [ ] `suture log` displays patches in topological order.
- [ ] `suture diff` uses the appropriate driver for file type.
- [ ] `suture config get/set/list` round-trips correctly.
- [ ] `suture key generate` creates a valid Ed25519 key pair.
- [ ] `suture gc` removes unreferenced blobs and preserves referenced ones.
- [ ] `cargo clippy` passes with zero warnings.
- [ ] `cargo test` passes all CLI integration tests.

---

*End of BP-CLI-001*
