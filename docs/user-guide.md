# Suture User Guide

## What is Suture?

Suture is a version control system with semantic merge for structured files.
Unlike Git, which treats files as opaque blobs, Suture understands the internal
structure of JSON, YAML, TOML, CSV, XML, and 13 other formats, enabling
automatic conflict resolution.

Suture can also be used purely as a Git merge driver — no migration required.
Existing Git repos gain semantic merge in a single command.

## Quick Start

### Installation

```bash
# One-line install (Linux / macOS)
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/install.sh | sh

# Cargo
cargo install suture-cli

# Homebrew
brew install WyattAu/suture-merge-driver/suture-merge-driver

# npm (merge driver only)
npm install -g suture-merge-driver

# PyPI (merge driver only)
pip install suture-merge-driver

# Binary download
# https://github.com/WyattAu/suture/releases
```

### Basic Workflow

```bash
# Initialize a repository
suture init my-project
cd my-project

# Create and switch branches
suture branch feature/json-config
suture checkout feature/json-config

# Make changes, then commit
suture add config.json
suture commit "update config"

# View history
suture log --oneline --graph

# Merge — conflicts in structured files are resolved automatically
suture merge main

# Compare two branches
suture diff --from main --to feature

# Push/pull to a hub
suture remote add origin http://localhost:50051/my-project
suture push origin
suture pull origin
```

### Structured Merge

When two branches edit different keys in a JSON file, Git reports a conflict.
Suture resolves it automatically because it understands JSON structure.

**Git output:**
```
<<<<<<< HEAD
  "version": "5.1.0",
  "features": ["merge", "diff"]
=======
  "license": "AGPL-3.0"
>>>>>>> feature
```

**Suture output (automatic):**
```json
{
  "version": "5.1.0",
  "features": ["merge", "diff"],
  "license": "AGPL-3.0"
}
```

This works for all 18 supported formats. Suture detects the format from the
file extension and applies the appropriate merge strategy.

Merge strategies:
- `semantic` (default) — try semantic drivers, fall back to conflict markers
- `ours` — keep our version for all conflicts
- `theirs` — keep their version for all conflicts
- `manual` — skip semantic drivers, leave all conflicts as markers

```bash
suture merge -s ours feature
suture merge -s theirs feature
suture merge --dry-run feature   # preview without modifying working tree
```

### Remote Collaboration

```bash
# Add a remote hub
suture remote add origin http://hub.example.com/my-repo

# Authenticate
suture remote login origin

# Push and pull
suture push origin
suture push origin feature      # push a specific branch
suture pull origin
suture pull --rebase origin

# Fetch without merging
suture fetch origin
suture fetch --depth 10 origin  # shallow fetch

# Clone
suture clone http://hub.example.com/my-repo
suture clone http://hub.example.com/my-repo --depth 10

# List remote branches
suture ls-remote origin

# Mirror a remote
suture remote mirror http://upstream/repo upstream-name
```

### LFS (Large File Storage)

```bash
# Track large files by pattern
suture lfs track "*.mp4"
suture lfs track "*.png" --size-limit 5MB
suture lfs track "assets/*"

# Stop tracking
suture lfs untrack "*.mp4"

# List tracked patterns
suture lfs list

# Show LFS object summary
suture lfs status
```

### Merge Driver (Git Integration)

Add semantic merge to an existing Git repo in one line:

```bash
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash
```

Or manually:

```bash
suture git driver install    # install
suture git driver uninstall  # remove
suture git driver list       # check status
```

This configures `.gitattributes` so Git delegates merges of `*.json`, `*.yaml`,
`*.yml`, `*.toml`, `*.xml`, and `*.csv` files to Suture.

## Supported Formats

| Format | Extensions | Merge Strategy |
|--------|-----------|----------------|
| JSON | `.json` | Key-value merge |
| YAML | `.yml`, `.yaml` | Mapping merge |
| TOML | `.toml` | Table merge |
| CSV | `.csv` | Row-column merge |
| XML | `.xml` | Element merge |
| HTML | `.html`, `.htm` | DOM merge |
| Markdown | `.md`, `.markdown`, `.mdown`, `.mkd` | Section merge |
| SQL | `.sql` | Statement merge |
| SVG | `.svg` | Element merge |
| DOCX | `.docx` | OOXML merge |
| XLSX | `.xlsx` | Sheet merge |
| PPTX | `.pptx` | Slide merge |
| PDF | `.pdf` | Page merge |
| Image | `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.webp`, `.tiff`, `.ico`, `.avif` | Metadata merge |
| RSS/Atom | `.rss`, `.atom` | Feed merge |
| iCalendar | `.ics`, `.ifb` | Event merge |
| OTIO | `.otio` | Timeline merge |
| Properties | `.properties`, `.ini` | Key-value merge |

List available drivers:

```bash
suture drivers
```

## Command Reference

### Repository Commands

| Command | Description |
|---------|-------------|
| `suture init [path]` | Initialize a new repository |
| `suture clone <url> [dir]` | Clone a remote repository |
| `suture status` | Show working tree status |
| `suture log [branch]` | Show commit history |
| `suture diff` | Show differences between commits |
| `suture show <commit>` | Show detailed commit information |
| `suture fsck` | Verify repository integrity |

### File Operations

| Command | Description |
|---------|-------------|
| `suture add <paths>` | Stage files |
| `suture rm <paths>` | Remove files |
| `suture mv <src> <dst>` | Move or rename files |
| `suture restore <paths>` | Restore files from HEAD |
| `suture clean` | Remove untracked files |
| `suture grep <pattern>` | Search tracked files |
| `suture blame <path>` | Per-line commit attribution |

### Branching

| Command | Description |
|---------|-------------|
| `suture branch` | List branches |
| `suture branch <name>` | Create a branch |
| `suture branch -d <name>` | Delete a branch |
| `suture checkout <branch>` | Switch branches |
| `suture switch <branch>` | Switch branches (modern) |
| `suture merge <branch>` | Merge a branch |
| `suture rebase <branch>` | Rebase onto a branch |
| `suture tag <name>` | Create a tag |
| `suture tag -d <name>` | Delete a tag |
| `suture cherry-pick <commit>` | Apply a specific commit |
| `suture revert <commit>` | Revert a commit |
| `suture squash <n>` | Squash N commits |
| `suture undo [n]` | Undo last N commits |
| `suture rollback <commit>` | Create a reversing commit |

### Remote

| Command | Description |
|---------|-------------|
| `suture remote add <name> <url>` | Add a remote |
| `suture remote list` | List remotes |
| `suture remote remove <name>` | Remove a remote |
| `suture remote login <name>` | Authenticate |
| `suture push [remote] [branch]` | Push to remote |
| `suture pull [remote]` | Pull from remote |
| `suture fetch [remote]` | Fetch from remote |
| `suture ls-remote <url>` | List remote branches |

### Stash

| Command | Description |
|---------|-------------|
| `suture stash push [-m msg]` | Stash current changes |
| `suture stash pop` | Pop the most recent stash |
| `suture stash apply <n>` | Apply a specific stash |
| `suture stash list` | List stashes |
| `suture stash drop <n>` | Drop a stash |
| `suture stash clear` | Drop all stashes |

### LFS

| Command | Description |
|---------|-------------|
| `suture lfs track <pattern>` | Track files with LFS |
| `suture lfs untrack <pattern>` | Remove a tracking pattern |
| `suture lfs list` | List tracked patterns |
| `suture lfs status` | Show LFS object summary |

### Platform & Advanced

| Command | Description |
|---------|-------------|
| `suture config [key[=val]]` | Get/set configuration |
| `suture doctor [--fix]` | Check repository health |
| `suture key generate` | Generate a signing key |
| `suture verify <ref>` | Verify commit signatures |
| `suture bisect start <good> <bad>` | Binary search for bug-introducing commit |
| `suture worktree add <path>` | Create a worktree |
| `suture tui` | Launch terminal UI |
| `suture export <output>` | Export a clean snapshot |
| `suture archive -o <file>` | Create a repository archive |
| `suture sync` | Auto-commit and push/pull |
| `suture git import <path>` | Import Git history |
| `suture completions <shell>` | Generate shell completions |

## Integration Guides

### GitHub Actions

```yaml
- uses: WyattAu/suture/.github/actions/merge@main
  with:
    files: |
      package.json
      tsconfig.json
    base-ref: ${{ github.event.pull_request.base.sha }}
```

### GitLab CI

```yaml
semantic-merge:
  stage: merge
  image: ghcr.io/wyattau/suture:latest
  script:
    - suture merge-file --driver json base.json ours.json theirs.json -o merged.json
```

### VS Code Extension

Install from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=WyattAu.suture):

- Conflict highlighting for structured files
- One-click auto-merge
- Status bar integration

### LSP Protocol

The `suture-lsp` crate implements the Language Server Protocol for editor
integration. It provides semantic merge diagnostics and conflict resolution
capabilities directly in supported editors.

### Self-Hosted Hub

```bash
docker compose up -d
# Hub available at http://localhost:8080
```

See [Self-Hosting Guide](self-hosting.md) for Docker, binary, Kubernetes, and
systemd deployment options.
