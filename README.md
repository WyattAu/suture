<div align="center">
  <h1>Suture</h1>
  <p><strong>Semantic merge for every format.</strong></p>
  <p>Automatically resolve merge conflicts in JSON, YAML, TOML, XML, CSV, and 13+ more structured file formats.</p>
  
  [Install](#installation) · [Quick Start](#quick-start) · [Docs](docs/user-guide.md) · [API](docs/api-reference.md) · [Pricing](#pricing)
  
  [![Tests](https://github.com/WyattAu/suture/actions/workflows/ci.yml/badge.svg)](https://github.com/WyattAu/suture/actions/workflows/ci.yml)
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

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/install.sh | sh
```

| Method | Command |
|--------|---------|
| **Cargo** | `cargo install suture-cli` |
| **Homebrew** | `brew install WyattAu/suture-merge-driver/suture-merge-driver` |
| **npm** | `npm install -g suture-merge-driver` |
| **PyPI** | `pip install suture-merge-driver` |
| **Binary** | [GitHub Releases](https://github.com/WyattAu/suture/releases) |

## Git Merge Driver (5 seconds)

```bash
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash
```

That's it. Git will now automatically merge:
`*.json` `*.yaml` `*.yml` `*.toml` `*.xml` `*.csv`

## Quick Start

```bash
suture init my-project && cd my-project
suture branch feature/json-config
suture checkout feature/json-config
# ... edit config.json ...
suture add config.json
suture commit "update config"
suture merge main          # structured conflicts resolved automatically
suture remote add origin http://localhost:50051/my-project
suture push origin
```

## Supported Formats

| Format | Extensions | Strategy |
|--------|-----------|----------|
| JSON | `.json` | Key-value merge |
| YAML | `.yaml` `.yml` | Mapping merge |
| TOML | `.toml` | Table merge |
| XML | `.xml` | Element merge |
| CSV | `.csv` | Row-column merge |
| SQL | `.sql` | Statement merge |
| HTML | `.html` `.htm` | DOM tree merge |
| Markdown | `.md` `.markdown` | Section merge |
| SVG | `.svg` | Element merge |
| DOCX | `.docx` | OOXML merge |
| XLSX | `.xlsx` | Sheet merge |
| PPTX | `.pptx` | Slide merge |
| PDF | `.pdf` | Page merge |
| Image | `.png` `.jpg` `.jpeg` `.gif` `.webp` | Metadata merge |
| RSS/Atom | `.rss` `.atom` | Feed merge |
| iCalendar | `.ics` | Event merge |
| OTIO | `.otio` | Timeline merge |
| Properties | `.properties` `.ini` | Key-value merge |

## CLI Commands

`init` · `clone` · `status` · `add` · `rm` · `mv` · `commit` · `log` · `diff` · `show` · `branch` · `checkout` · `merge` · `rebase` · `tag` · `stash` · `push` · `pull` · `fetch` · `remote` · `lfs` · `blame` · `grep` · `cherry-pick` · `revert` · `reset` · `bisect` · `worktree` · `config` · `doctor` · `key` · `verify` · `export` · `archive` · `tui` · `git import` · `sync`

```bash
suture --help          # full command list
suture drivers         # list available merge drivers
suture completions bash > ~/.bash_completion.d/suture
```

## API

```bash
curl -X POST https://api.suture.dev/api/merge \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "driver": "json",
    "base": "{\"name\": \"base\"}",
    "ours": "{\"name\": \"ours\"}",
    "theirs": "{\"name\": \"theirs\"}"
  }'
```

[Full API Reference](docs/api-reference.md)

## Pricing

| Plan | Price | Merges | Storage | Features |
|------|-------|--------|---------|----------|
| Free | $0 | 100/mo | 100 MB | 5 core drivers |
| Pro | $9/user/mo | 10,000/mo | 10 GB | All drivers, analytics |
| Enterprise | $29/user/mo | Unlimited | 100 GB | SSO, audit, WASM plugins, SLA |

Self-hosted is always free (AGPL-3.0).

## Platform Features

- **Semantic merge API** — REST endpoint for all 17 format drivers
- **WASM plugin system** — custom merge drivers in any language
- **OAuth** — Google and GitHub sign-in
- **Organizations** — teams with role-based access (owner/admin/member/viewer)
- **Analytics** — merge metrics, conflict rates, driver usage
- **Stripe billing** — checkout sessions, customer portal, webhook handling
- **Rate limiting** — per-endpoint protection

## Self-Hosted Hub

```bash
docker compose up -d
# Hub available at http://localhost:8080
```

See [Self-Hosting Guide](docs/self-hosting.md) for Docker, binary, Kubernetes, and systemd deployment.

## GitHub Actions

```yaml
- uses: WyattAu/suture/.github/actions/merge@main
  with:
    files: |
      package.json
      tsconfig.json
    base-ref: ${{ github.event.pull_request.base.sha }}
```

## VS Code Extension

Install from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=WyattAu.suture):
conflict highlighting, one-click auto-merge, status bar integration.

## Documentation

[User Guide](docs/user-guide.md) · [API Reference](docs/api-reference.md) · [Self-Hosting](docs/self-hosting.md) · [Deployment](docs/deploy-runbook.md) · [CI Integration](docs/ci-integration.md)

## Architecture

```
suture/
├── crates/
│   ├── suture-core/            # Merge engine (355 tests)
│   ├── suture-driver-*/        # 17 format drivers
│   ├── suture-cli/             # CLI (115 tests)
│   ├── suture-hub/             # Coordination server (75 tests)
│   ├── suture-platform/        # Hosted SaaS (REST API, billing, auth)
│   ├── suture-raft/            # Consensus (30 tests)
│   ├── suture-vfs/             # FUSE filesystem
│   ├── suture-wasm-plugin/     # WASM plugin system
│   ├── suture-tui/             # Terminal UI (37 tests)
│   └── suture-lsp/             # Language server (25 tests)
├── desktop-app/                # Tauri desktop app
├── vscode-extension/           # VS Code extension
├── templates/                  # .gitattributes templates
├── scripts/                    # Install scripts
└── deploy/                     # Helm chart, Docker
```

## License

- **Self-hosted:** AGPL-3.0 (free forever)
- **Commercial:** [Suture Commercial License](LICENSE-COMMERCIAL)
