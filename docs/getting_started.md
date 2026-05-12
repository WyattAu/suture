# Getting Started with Suture

## Prerequisites

Suture uses [Nix](https://nixos.org/) for reproducible builds. Install Nix with
the Determinate Systems installer:

```bash
curl --proto '=https' --tlsv1.2 -sSf -L \
  https://install.determinate.systems/nix | sh -s -- install
```

Alternatively, if you have a Rust toolchain with the same dependencies as the
project `flake.nix`, you can build directly with `cargo`.

## Installation

Clone the repository and enter the development shell:

```bash
git clone https://github.com/your-org/suture.git
cd suture
nix develop
```

Build the CLI:

```bash
cargo build --bin suture-cli
```

The binary will be at `target/debug/suture-cli`.

## Quick Start

### 1. Initialize a Repository

```bash
suture-cli init my-project
cd my-project
```

This creates a `.suture/` directory containing the object store and metadata
database.

### 2. Add Files

```bash
suture-cli add timeline.otio
```

### 3. Commit

```bash
suture-cli commit "Initial timeline"
```

The output shows the BLAKE3 patch ID (first 12 hex characters).

### 4. Create a Branch

```bash
suture-cli branch feature/audio-mix
```

### 5. Make Changes and Commit on the Branch

Switch context by opening the project in your NLE, make edits, then:

```bash
suture-cli add timeline.otio
suture-cli commit "Adjust audio levels"
```

### 6. View History

```bash
suture-cli log
```

Output:

```
* abc123def456… Adjust audio levels
  789abc012def… Initial timeline
```

### 7. Check Merge Status

```bash
suture-cli merge main feature/audio-mix
```

If the two branches edited different clips, the merge is clean. If the same
clip was modified on both branches, Suture reports a conflict.

## Basic Workflow Example

A typical session working on a video timeline:

```bash
# Initialize
suture-cli init my-documentary
cd my-documentary

# Import initial timeline and commit
cp ~/editor-export/v1.otio timeline.otio
suture-cli add timeline.otio
suture-cli commit "Import timeline from editor"

# Create a branch for a collaborator
suture-cli branch colorist/work

# On the main branch, trim the intro
# (edit timeline.otio in your NLE)
suture-cli add timeline.otio
suture-cli commit "Trim intro by 2 seconds"

# On the colorist branch, apply color grades
# (edit timeline.otio in your NLE)
suture-cli add timeline.otio
suture-cli commit "Apply color grade to acts 1-3"

# Check if the branches can merge cleanly
suture-cli merge main colorist/work
# Output: "Merge is clean (no conflicts)."
```

## Next Steps

- See [API Reference](api_reference.md) for programmatic usage
- See [Driver SDK](driver_sdk.md) to implement support for new file formats
- See the [Architecture spec](../architectural_spec.md) for the mathematical foundations and merge algebra
