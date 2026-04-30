<div align="center">
  <h1>Suture</h1>
  <p><strong>Semantic merge for every format.</strong></p>
  <p>Automatically resolve merge conflicts in JSON, YAML, TOML, XML, CSV, and 12+ more structured file formats.</p>
  
  [Try the Demo](https://suture.dev/#/merge) · [Install](#installation) · [Docs](https://suture.dev/#/api) · [Pricing](https://suture.dev/#/billing)
  
  [![crates.io](https://img.shields.io/crates/v/suture-merge-driver.svg)](https://crates.io/crates/suture-merge-driver)
  [![npm](https://img.shields.io/npm/v/suture-merge-driver.svg)](https://www.npmjs.com/package/suture-merge-driver)
  [![PyPI](https://img.shields.io/pypi/v/suture-merge-driver.svg)](https://pypi.org/project/suture-merge-driver/)
  [![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE-AGPL)
</div>

---

## Why Suture?

Git's merge is line-based. When two branches change different keys in a JSON file, Git reports a conflict — even though there's no actual conflict.

**Before (Git):**
```
<<<<<<< HEAD
  "version": "5.1.0",
  "features": ["merge", "diff", "platform"]
=======
  "license": "AGPL-3.0"
>>>>>>> feature
```

**After (Suture):**
```json
{
  "version": "5.1.0",
  "features": ["merge", "diff", "platform"],
  "license": "AGPL-3.0"
}
```

## Install as Git Merge Driver (5 seconds)

```bash
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash
```

That's it. Git will now automatically merge:
`*.json` `*.yaml` `*.yml` `*.toml` `*.xml` `*.csv`

## Installation

| Method | Command |
|--------|---------|
| **Cargo** | `cargo install suture-merge-driver` |
| **Homebrew** | `brew install WyattAu/suture-merge-driver/suture-merge-driver` |
| **npm** | `npm install -g suture-merge-driver` |
| **PyPI** | `pip install suture-merge-driver` |
| **Binary** | [GitHub Releases](https://github.com/WyattAu/suture/releases) |

## Supported Formats

| Format | Extensions | Features |
|--------|-----------|----------|
| JSON | `.json` | Deep merge, arrays, conflicts |
| YAML | `.yaml` `.yml` | Mappings, anchors, arrays |
| TOML | `.toml` | Tables, arrays, inline tables |
| XML | `.xml` | Elements, attributes |
| CSV | `.csv` | Row-based by key column |
| SQL | `.sql` | Statement-level |
| HTML | `.html` | DOM tree merge |
| Markdown | `.md` | Section-based |
| SVG | `.svg` | Element merge |
| Properties | `.properties` `.ini` | Key-value |
| DOCX | `.docx` | Binary (merge_raw) |
| XLSX | `.xlsx` | Binary (merge_raw) |
| PPTX | `.pptx` | Binary (merge_raw) |
| PDF | `.pdf` | Binary (merge_raw) |
| Image | `.png` `.jpg` | Binary (merge_raw) |
| RSS/Atom | `.rss` `.atom` | Feed merge |
| iCal | `.ics` | Calendar merge |
| OTIO | `.otio` | Timeline merge |

## CLI

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

# Merge automatically resolves conflicts
suture merge main

# Push/pull to a hub
suture remote add origin https://hub.example.com
suture push origin
suture pull origin
```

## API

```bash
curl -X POST https://merge.suture.dev/api/merge \
  -H "Content-Type: application/json" \
  -d '{
    "driver": "json",
    "base": "{\"name\": \"base\"}",
    "ours": "{\"name\": \"ours\"}",
    "theirs": "{\"name\": \"theirs\"}"
  }'
```

[Full API Documentation](https://suture.dev/#/api)

## GitHub Actions

```yaml
- uses: WyattAu/suture/.github/actions/merge@main
  with:
    files: |
      package.json
      tsconfig.json
    base-ref: ${{ github.event.pull_request.base.sha }}
```

## Self-Hosted Hub

```bash
docker compose up -d
# Hub available at http://localhost:8080
```

See [Self-Hosting Guide](docs/self-hosting.md) for Docker, binary, Kubernetes, and systemd deployment.

## Platform

[merge.suture.dev](https://suture.dev) — Hosted semantic merge API

| Plan | Price | Merges | Storage | Features |
|------|-------|--------|---------|----------|
| Free | $0 | 100/mo | 100 MB | 5 core drivers |
| Pro | $9/user/mo | 10,000/mo | 10 GB | All drivers, analytics |
| Enterprise | $29/user/mo | Unlimited | 100 GB | SSO, audit, SLA |

Self-hosted is always free (AGPL-3.0).

## VS Code Extension

Install from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=WyattAu.suture):

- Conflict highlighting
- One-click auto-merge
- Status bar integration

## Architecture

```
suture/                          # Monorepo root
├── crates/
│   ├── suture-core/            # Merge engine (342 tests)
│   ├── suture-driver-*/        # 18 format drivers
│   ├── suture-cli/             # CLI (115 tests)
│   ├── suture-hub/             # Coordination server (50 tests)
│   ├── suture-platform/        # Hosted SaaS
│   ├── suture-raft/            # Consensus (48 tests)
│   ├── suture-vfs/             # FUSE filesystem
│   ├── suture-wasm-plugin/     # WASM plugin system
│   ├── suture-tui/             # Terminal UI (37 tests)
│   └── suture-lsp/             # Language server (25 tests)
├── desktop-app/                # Tauri desktop app
├── vscode-extension/           # VS Code extension
├── templates/                  # .gitattributes templates
├── scripts/                    # Install scripts
├── deploy/                     # Helm chart, Docker
└── docs/                       # Guides, blog, SEO
```

## License

- **Self-hosted:** AGPL-3.0 (free forever)
- **Commercial:** [Suture Commercial License](LICENSE-COMMERCIAL)
