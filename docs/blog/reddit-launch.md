# Suture 5.1.0 – Semantic merge for 18 file formats, git merge driver, TUI, LSP, WASM plugins, Raft consensus, SaaS platform

I've been building Suture for the past 5 weeks. It's a semantic merge engine that parses structured files into their native representations and merges at the structural level instead of line-by-line.

## The problem
Git's line-based merge reports conflicts when two people edit different keys in the same JSON/YAML/TOML object. Suture parses the files, identifies non-overlapping changes, and merges them automatically — only flagging true conflicts.

## What's in 5.1.0
- **18 format drivers:** JSON, YAML, TOML, XML, CSV, SQL, HTML, Markdown, SVG, Properties, DOCX, XLSX, PPTX, PDF, RSS/Atom, iCalendar, OpenTimelineIO, PLIST
- **Git merge driver:** One curl command installs it as a git merge driver for JSON/YAML/TOML/XML/CSV
- **TUI:** Dashboard, patch browser, 3-way merge view
- **VS Code extension:** LSP with diagnostics, completions, hover, go-to-symbol, and real-time merge diagnostics
- **WASM plugins:** wasmtime v28, fuel-based timeouts, 16MB memory limit, `SutureWasmPlugin` trait
- **Raft consensus:** Pre-vote, log compaction, snapshots, leadership transfer, membership changes
- **GitHub Action:** Semantic merge in CI/CD pipelines (`uses: WyattAu/suture/.github/actions/merge@v5`)
- **FUSE mount:** Browse repos as filesystem
- **SaaS platform:** Full auth (Google/GitHub OAuth), Stripe billing, merge API, deployed to Fly.io

## Pricing
- **Free:** Unlimited public repos, community support
- **Pro ($9/seat/month):** Unlimited private repos, merge API access, priority support
- **Enterprise ($29/seat/month):** SSO, audit logging, WASM plugins, SLA, dedicated support

## Architecture
39 crates, 1,594 tests, 0 failures, 0 clippy warnings. Zero known security or performance issues. Core is the `SutureDriver` trait — four methods (`parse`, `diff`, `format_diff`, `merge`). Each format implements this trait. The merge engine works on an intermediate tree representation.

## Performance
- JSON: 10µs (10 keys) → 400µs (1K keys) → 4ms (10K keys)
- YAML: 15µs (10 mappings) → 80µs (50) → 300µs (200)
- CSV: 100µs (10 rows × 5 cols)

## Install
```bash
curl -sSL https://suture.dev/install.sh | sh
```

Also available via Cargo, Homebrew, npm, PyPI, and binary releases.

## Try it
- **Live demo:** https://wyattau.github.io/suture/#/merge (client-side, no signup)
- **Source:** https://github.com/WyattAu/suture
- **Crates.io:** https://crates.io/crates/suture-cli
- **Platform:** https://suture.dev

Happy to answer questions about the architecture, the merge algorithm, or anything else.
