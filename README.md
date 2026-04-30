# Suture

![Build](https://github.com/WyattAu/suture/actions/workflows/ci.yml/badge.svg)
![Release](https://github.com/WyattAu/suture/actions/workflows/release.yml/badge.svg)

Version control that understands your files.

Git is to text files what Suture is to Word docs, spreadsheets, and video timelines.

```text
  Before (Git)                              After (Suture)
  ─────────────────────                     ─────────────────────
  Two people edit config.json:              Same scenario:
  one changes "host", the other "port".     Suture merges both keys — zero conflicts.

  $ git merge staging                       $ suture merge staging
  CONFLICT (content): Merge conflict        Clean merge. 2 patches applied.
  in config.json
  <<<<<<< HEAD
    "host": "prod",
    "port": 8080
  =======
    "host": "staging"
    "port": 3000
  >>>>>>> staging
```

Suture uses semantic drivers to merge JSON, YAML, TOML, CSV, XML, Markdown, HTML, SVG, DOCX, XLSX, PPTX, SQL, PDF, images, iCalendar, RSS/Atom feeds, and video timelines at the structural level — not the line level. Two edits to different JSON keys, different DOCX paragraphs, or different spreadsheet cells never conflict.

## Quick Start

```bash
# Install (builds in ~2 minutes)
cargo install suture-cli

# Or download a prebuilt binary from GitHub Releases
# https://github.com/WyattAu/suture/releases

# Git merge driver (install once, works with any repo)
npm install -g suture-merge-driver    # Node.js
pip install suture-merge-driver       # Python

suture init
suture config user.name "Your Name"

echo '{"host": "localhost", "port": 3000}' > config.json
suture add . && suture commit "base config"

suture branch staging && suture checkout staging
echo '{"host": "staging", "port": 3000}' > config.json
suture add . && suture commit "point to staging"

suture checkout main
echo '{"host": "localhost", "port": 8080}' > config.json
suture add . && suture commit "change port"

suture merge staging
# {"host": "staging", "port": 8080}  ← both changes, no conflict
```

## Who Is This For?

| Domain | Pain point Suture solves |
|--------|--------------------------|
| [Defence & compliance](docs/onboarding-defence.md) | Audit trails (`log --audit`), classification detection (`diff --classification`), signed commits |
| [Video editors](docs/video-editors.md) | Version control for NLE timelines (Premiere, DaVinci Resolve) |
| [Document authors](docs/document-authors.md) | Merge Word docs, Excel sheets, PowerPoint decks |
| [Content creators](README.md#suture-sync) | `suture sync` replaces Google Drive — auto-commit, pull, push |
| [Data science](docs/data-science.md) | Branch and merge Jupyter notebooks, CSVs, configs |
| DevOps | Semantic merge for Kubernetes YAML, Docker Compose, CI configs |

## What Makes Suture Different

**Semantic merge for 20+ formats:**

| Format | Extensions | Merge granularity |
|--------|-----------|-------------------|
| JSON | `.json` | Field-level (RFC 6901 paths) |
| YAML | `.yaml` `.yml` | Key-level |
| TOML | `.toml` | Table and key-aware |
| CSV | `.csv` | Row-level with header detection |
| XML | `.xml` | Element/attribute-aware |
| Markdown | `.md` | Section-aware |
| HTML | `.html` | DOM-aware |
| SVG | `.svg` | Element-aware |
| DOCX | `.docx` | Paragraph-level |
| XLSX | `.xlsx` | Cell-level |
| PPTX | `.pptx` | Slide-level |
| SQL | `.sql` | DDL schema diff |
| PDF | `.pdf` | Page-level text diff |
| Image | `.png` `.jpg` `.gif` `.bmp` `.webp` `.tiff` `.ico` `.avif` | Metadata diff |
| OTIO | `.otio` | OpenTimelineIO editorial merge |
| iCalendar | `.ics` | Event-level merge |
| RSS/Atom | `.rss` `.atom` | Feed and entry-aware |

Files without a driver fall back to line-based merge, same as Git.

**50+ CLI commands:** init, add, commit, branch, merge, rebase, cherry-pick, push, pull, clone, tag, blame, stash, worktree, remote, log, diff, show, status, grep, archive, export, sync, verify, describe, bisect, apply, clean, doctor, fsck, gc, notes, reflog, completions, and more.

**Google Drive replacement:** `suture sync` auto-commits, pulls, and pushes — one command replaces cloud folder sync with full version history.

**Audit & compliance:** `suture log --audit` exports structured audit trails (JSON/CSV/text). `suture diff --classification` detects NATO/US/UK/AU security marking changes. Ed25519 commit signing with `suture verify`.

**Client delivery:** `suture export` creates clean snapshots for handoff. `suture diff --summary` produces human-readable change reports.

**Mount as a filesystem:** FUSE and WebDAV mounts let any editor save directly into a Suture repo — every save creates a patch.

**Self-hosted collaboration:** Suture Hub provides a web UI, auth, push/pull, mirrors, and search.

## Install

### Prebuilt binaries (recommended)

Download from [GitHub Releases](https://github.com/WyattAu/suture/releases) — Linux x86_64, macOS x86_64/aarch64, Windows x86_64.

### From source

```bash
git clone https://github.com/WyattAu/suture.git
cd suture && cargo build --release --bin suture
# Binary at target/release/suture
```

### Package managers

```bash
cargo install suture-cli                        # crates.io
brew tap WyattAu/suture-merge-driver           # Homebrew tap
brew install suture-merge-driver               # macOS / Linux
paru -S suture-git                             # Arch Linux (AUR)
npm install -g suture-merge-driver             # Node.js
pip install suture-merge-driver                # Python
```

## Key Commands

```bash
suture init                        # Initialize a repository
suture add . && suture commit "msg" # Stage and commit
suture branch feature && suture checkout feature  # Branching
suture merge feature               # Semantic merge
suture push origin main            # Push to remote
suture pull                        # Pull from remote
suture sync                        # Auto-commit + pull + push
suture log --graph                 # Visual commit history
suture diff --summary              # Human-readable change summary
suture log --audit                 # Structured audit trail
suture diff --classification        # Classification marking changes
suture verify HEAD                 # Verify commit signature
suture stash push -m "WIP"         # Stash work in progress
suture export ./delivery           # Clean snapshot for client
suture doctor --fix                # Auto-remediate common issues
```

## Use With Git

Suture works as a [Git merge driver](docs/git_merge_driver.md) — add semantic merging to your existing Git repos in 30 seconds:

```bash
# In your Git repo:
suture git driver install
git add .gitattributes .suture/git-merge-driver.sh
git commit -m "Configure suture semantic merge driver"
```

That's it. Future merges on JSON, YAML, DOCX, XLSX, and 17 other file types will use semantic merge automatically. See the [5-minute quickstart](docs/merge-driver-quickstart.md) for details.

## Blog

- [Why Git Merge Fails on JSON](docs/blog/why-git-merge-fails-on-json.md) — line-based vs semantic merging, with before/after examples
- [Semantic Merge Explained](docs/blog/semantic-merge-explained.md) — key-path diffing, array strategies, three-way merge theory
- [Semantic Merge for 17 File Formats](docs/blog/semantic-merge-for-17-file-formats.md)
- [Semantic Merge for Binary Documents](docs/blog/semantic-merge-for-binary-documents.md)
- [Stop Having Merge Conflicts in JSON](docs/blog/stop-having-merge-conflicts-in-json.md)

## Learn More

- [5-Minute Git Merge Driver Quickstart](docs/merge-driver-quickstart.md) — the fastest way to get started
- [Quick Start Guide](docs/quickstart.md) — full standalone usage
- [Why Suture?](docs/why-suture.md) — the problem with binary version control
- [Suture vs. Git](docs/comparing-with-git.md) — honest comparison
- [Semantic merge explained](docs/semantic-merge.md) — how it works under the hood
- [Full CLI reference](docs/cli-reference.md)
- [Comparison with other VCS](docs/comparison.md)

## Stats

- **37 crates** in the workspace
- **17 semantic drivers** for structured file formats
- **1,400+ tests** across the workspace
- **50+ CLI commands**
- **v5.0.1** — unified version across all packages (crates.io, npm, PyPI, Homebrew)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Apache License 2.0. See [LICENSE](LICENSE).
