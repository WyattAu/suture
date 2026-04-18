# Suture

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

Suture uses semantic drivers to merge JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, SQL, PDF, images, and video timelines at the structural level — not the line level. Two edits to different JSON keys, different DOCX paragraphs, or different spreadsheet cells never conflict.

## Quick Start

```bash
cargo install suture-cli

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
| [Video editors](docs/video-editors.md) | Version control for NLE timelines (Premiere, DaVinci Resolve) |
| [Document authors](docs/document-authors.md) | Merge Word docs, Excel sheets, PowerPoint decks |
| [Data science](docs/data-science.md) | Branch and merge Jupyter notebooks, CSVs, configs |
| DevOps | Semantic merge for Kubernetes YAML, Docker Compose, CI configs |
| Config-as-code | TOML, XML, properties files across environments |

## What Makes Suture Different

**Semantic merge for 16 formats:**

| Format | Extensions | Merge granularity |
|--------|-----------|-------------------|
| JSON | `.json` | Field-level (RFC 6901 paths) |
| YAML | `.yaml` `.yml` | Key-level |
| TOML | `.toml` | Table and key-aware |
| CSV | `.csv` | Row-level with header detection |
| XML | `.xml` | Element/attribute-aware |
| Markdown | `.md` | Section-aware |
| DOCX | `.docx` | Paragraph-level |
| XLSX | `.xlsx` | Cell-level |
| PPTX | `.pptx` | Slide-level |
| SQL | `.sql` | DDL schema diff |
| PDF | `.pdf` | Page-level text diff |
| Image | `.png` `.jpg` `.gif` `.bmp` `.webp` `.tiff` `.ico` `.avif` | Metadata diff |
| OTIO | `.otio` | OpenTimelineIO editorial merge |

Files without a driver fall back to line-based merge, same as Git.

**Full version control workflow:** init, add, commit, branch, merge, rebase, cherry-pick, push, pull, tag, blame, stash, worktree, and 25+ more commands.

**Mount as a filesystem:** FUSE and WebDAV mounts let any editor save directly into a Suture repo — every save creates a patch.

**Self-hosted collaboration:** Suture Hub provides a web UI, auth, push/pull, mirrors, and search.

## Install

```bash
cargo install suture-cli          # crates.io (Rust 1.85+)
brew install wyattau/tap/suture   # macOS / Linux
paru -S suture-git                # Arch Linux (AUR)
```

Or build from source:

```bash
git clone https://github.com/WyattAu/suture.git
cd suture && cargo build --release --bin suture
```

## Use With Git

Suture works as a [Git merge driver](docs/git_merge_driver.md) — add semantic merging to your existing Git repos:

```bash
git config merge.suture.name "suture"
git config merge.suture.driver "suture merge-file --driver %s %O %A %B -o %A"
```

## Learn More

- [Why Suture?](docs/why-suture.md) — the problem with binary version control
- [Suture vs. Git](docs/comparing-with-git.md) — honest comparison
- [Semantic merge explained](docs/semantic-merge.md) — how it works under the hood
- [Full CLI reference](docs/cli-reference.md)
- [Comparison with other VCS](docs/comparison.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Apache License 2.0. See [LICENSE](LICENSE).
