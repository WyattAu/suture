# Suture Version

- **Current Version:** 3.2.0
- **Current Phase:** Directions T–X — Ship, Validate, Iterate
- **Status:** Complete
- **Last Updated:** 2026-04-18
- **Rust Edition:** 2024
- **Lean 4:** v4.29.1 (23 theorems proved)

## Strategic Roadmap

| Phase | Direction | Focus | Target Version | Status |
|-------|-----------|-------|---------------|--------|
| **A** | Product Polish | Hub as self-hosted GitLab/Gitea portal | v1.3 – v1.4 | ✅ Complete |
| **B** | Enterprise Infra | VFS mount + Daemon + SHM + gRPC | v2.0 – v2.5 | ✅ Complete |
| **C** | Ecosystem Growth | Drivers, plugins, language bindings | v2.5+ | ✅ Complete |
| **D** | Hardening | Wire scaffolds, Raft, S3, mount manager, JetBrains, Python | v2.6 | ✅ Complete |
| **E** | Ship It | Distribution, docs, packaging, release automation | v2.7 | ✅ Complete |
| **F** | Production Hardening | S3/Raft integration, benchmarks, integration tests | v2.8 | ✅ Complete |
| **G** | Growth | VS Code, webhooks, desktop UI, perf fix | v2.9 | ✅ Complete |
| **H** | Validate & Ship | Release prep, shipping checklist, release script | v2.10 | ✅ Complete |
| **I** | Depth over Breadth | Wire S3 and Raft into hub's actual runtime | v2.10 | ✅ Complete |
| **J** | Make Raft Work | TCP transport wired, multi-node cluster, persist log | v3.0 | ✅ Complete |
| **K** | Production Readiness | Health check, graceful shutdown, config, tracing, rate limit | v3.0 | ✅ Complete |
| **L** | Performance & Scale | Batch patches, scale benchmarks | v3.0 | ✅ Complete |
| **M** | Real Desktop App | CLI commands, system tray, repo integration | v3.0 | ✅ Complete |
| **N** | Ecosystem Polish | VS Code LSP, interactive demo | v3.0 | ✅ Complete |
| **O** | Publish & Distribute | crates.io, Homebrew, AUR, install verification | v3.1 | ✅ Complete |
| **P** | Driver Audits | 63 correctness tests for DOCX/XLSX/PPTX/PDF/OTIO/Image | v3.1 | ✅ Complete |
| **Q** | Semantic Diff | File-type-aware diff, structured output, icons | v3.1 | ✅ Complete |
| **R** | Domain Workflows | --type flag, file-type icons, domain-aware init/status | v3.1 | ✅ Complete |
| **S** | Domain Marketing | README rewrite, 5 domain pages, why-suture, vs-git | v3.1 | ✅ Complete |
| **T** | Actually Ship | Publish prep, release build script, binary verification | v3.2 | ✅ Complete |
| **U** | CI/CD | GitHub Actions CI, release workflow, dependabot, issue templates | v3.2 | ✅ Complete |
| **V** | Driver Validation | 58 realistic tests for DOCX/XLSX/PPTX/PDF/OTIO/Image | v3.2 | ✅ Complete |
| **W** | Workflow Reliability | 7 E2E workflow tests (basic, merge, conflict, branch, history, stash) | v3.2 | ✅ Complete |
| **X** | Performance Baseline | 16 benchmarks, docs/performance.md, quick optimizations | v3.2 | ✅ Complete |

### Direction A — Product Polish (v1.3–v1.4) ✅

- Hub web portal: file browser, repo detail pages, diff viewer, user registration
- TUI: 7 tabs, checkout confirmation, merge conflict view, log graph
- Desktop app: Tauri v2 scaffold with IPC commands

### Direction B — Enterprise Infrastructure (v2.0–v2.5) ✅

- FUSE3 read/write VFS so NLEs see Suture repos as regular directories
- WebDAV cross-platform mount (macOS Finder, Windows Explorer)
- Background daemon with SHM for nanosecond status queries
- PID file management and signal handling (SIGTERM, SIGHUP)
- gRPC transport with 14 RPCs (Handshake, ListRepos, GetRepoInfo, CreateRepo, DeleteRepo, ListBranches, CreateBranch, DeleteBranch, ListPatches, GetBlob, Push, Pull, GetTree, Search)

### Direction C — Ecosystem Growth (v2.5) ✅

- SQL semantic driver: DDL parsing, schema diff, three-way merge
- PDF semantic driver: text extraction via lopdf, page-level diff/merge
- Image metadata driver: dimension/color detection, 10 formats
- Neovim plugin: 10 commands, gutter signs, float windows
- Node.js bindings: napi-rs native addon with TypeScript declarations

### Direction D — Hardening (v2.6) ✅

- gRPC server wired: all 14 RPCs with real tonic service
- Cursor-based pagination for hub API (backward compatible)
- Mount manager: FUSE/WebDAV lifecycle management in daemon
- Raft consensus: leader election, log replication, commit (suture-raft crate)
- S3 blob storage: AWS SigV4, path/virtual-hosted, MinIO compatible (suture-s3 crate)
- JetBrains IntelliJ plugin: 10 actions, VCS root detection, Kotlin/Gradle
- Python bindings enhanced: notes, worktree, blame, bisect, remotes, utilities
- `suture add .` bug fix: recursive directory expansion

### Direction E — Ship It (v2.7) ✅

- User documentation: quickstart, semantic merge guide, CLI reference, hub guide
- GitHub Pages landing page (dark terminal aesthetic)
- CONTRIBUTING.md updated for v2.7.0
- PR template with quality gate checklist
- Release workflow: 5-platform matrix (Linux x86/ARM, macOS x86/ARM, Windows)
- Homebrew formula with test block
- AUR PKGBUILD for Arch Linux
- crates.io publish guide with dependency order

### Direction F — Production Hardening (v2.8) ✅

- S3 blob backend wired into hub: `BlobBackend` trait, `SqliteBlobBackend`, `S3BlobBackendAdapter`
- Raft consensus wired into hub: `RaftHub` wrapper, `HubCommand` enum, cluster config (both opt-in features)
- Raft 3-node cluster simulation with 8 integration tests
- FUSE integration tests: mount read/write/modify/delete/stat, WebDAV serve test
- S3 integration tests: 7 MinIO-compatible tests gated on env vars
- Benchmark analysis: 28 functions profiled, 5 optimization opportunities identified

### Direction G — Growth (v2.9) ✅

- VS Code extension: 14 commands, SutureHelper class, output channel, quick pick, SVG icon
- Webhook system: CRUD routes, async fire-and-forget delivery, HMAC-SHA256 signing, push/branch events
- Desktop app: real web UI with 6 views, dark theme, commit modal, branch management
- Performance fix: repo_log O(n²) → O(n) via HashSet cycle detection

### Direction H — Validate & Ship (v2.10) ✅

- Shipping checklist: step-by-step pre-ship verification (tests, clippy, docs, features)
- Release notes: v2.9.0 changelog
- Release script: automated quality gates + git tagging

### Direction I — Depth over Breadth (v2.10) ✅

- S3 blob backend runtime wiring: CLI flags (--blob-backend, --s3-endpoint, --s3-bucket, etc.), server startup creates S3BlobBackendAdapter
- Raft TCP transport: `RaftTcpTransport` with 4-byte BE length + JSON wire format, listen/send_to_peer/receive, 4 unit tests
- Raft runtime manager: `RaftRuntime` with background tick loop, propose/apply channels, leader tracking, 3 tests
- Raft CLI flags: --raft, --raft-node-id, --raft-peers, --raft-port, --raft-election-timeout, --raft-heartbeat-interval
- Server blob routing: all blob store/get operations route through BlobBackend when set, fall back to SQLite
- All gated on opt-in features: `s3-backend`, `raft-cluster`

### Direction J — Make Raft Work (v3.0) ✅

- TCP transport wired into RaftRuntime: outgoing messages sent via TCP, incoming messages handled automatically
- Multi-node 3-cluster integration test over real TCP (leader election + log replication)
- Randomized election timeouts (Raft paper §5.2) to prevent split-vote livelock
- Persisted Raft log: `SqliteRaftLog` gated on `persist` feature (9 new tests, 30 total)
- `apply_raft_command()` on HubStorage: committed commands applied automatically (3 new tests)
- 12 hub raft tests (up from 8)

### Direction K — Production Readiness (v3.0) ✅

- Health check endpoint: `GET /healthz` returning `{status: "ok"}`
- Graceful shutdown: ctrlc signal handling, drain connections, close DB
- TOML config file: `--config` flag, CLI args override file defaults
- Request tracing middleware: `X-Request-Id` UUID v4 via tower-http
- Persistent rate limiter: SQLite-backed rate_limits table (survives restarts)

### Direction L — Performance & Scale (v3.0) ✅

- Batch patch endpoint: `POST /repos/{repo_id}/patches/batch` — multiple patches in one request
- Scale benchmarks: 1000 repos, 100 patches, 100 blobs (Criterion)

### Direction M — Real Desktop App (v3.0) ✅

- CLI commands wired to real `suture` binary: init, status, branch, log, commit
- System tray: show/refresh/quit via Tauri tray-icon feature

### Direction N — Ecosystem Polish (v3.0) ✅

- VS Code extension: LSP client connection, workspace activation for `.suture` dirs
- Interactive demo: 10-step animated workflow with SVG DAG visualization

### Direction O — Publish & Distribute (v3.1) ✅

- All 28 publishable crates have proper metadata (description, license, categories)
- Publish script (`scripts/publish.sh`) with dry-run and --real modes, dependency order
- Install verification script (`scripts/verify-install.sh`)
- Homebrew formula and AUR PKGBUILD updated to v3.0.0
- `packaging/PUBLISH.md` rewritten with correct dependency order

### Direction P — Driver Correctness Audits (v3.1) ✅

- 63 new correctness tests across 6 driver test files
- DOCX: two-editor merge, conflict detection, diff formatting (11 tests)
- XLSX: cell-level merge, conflict detection, diff formatting (9 tests)
- PPTX: slide-level merge, addition/removal detection (10 tests)
- OTIO: multi-track timeline, JSON diff, touch set cascade (10 tests)
- Image: metadata diff, binary change detection, dimension merge (11 tests)
- PDF: text extraction, page-level diff, multi-page merge (12 tests)
- Known limitations documented: PPTX/XLSX single-line XML, DOCX positional diff

### Direction Q — Semantic Diff Visualization (v3.1) ✅

- `FileType` detection module: 14 types + `auto_detect_repo_type()`
- `SemanticDiffFormatter`: file-type-aware headers, structured output
- CLI diff shows driver type label (e.g., `=== data.json [JSON]`)
- Image diffs include file size comparison
- `suture diff` auto-selects semantic driver when available

### Direction R — Domain-Specific Workflows (v3.1) ✅

- `suture init --type video|document|data` with auto-detect from existing files
- `suture status` shows file-type icons: 📄📊📽️🎬🖼️📋
- `.suture/config` stores repo type for workflow customization

### Direction S — Domain Marketing (v3.1) ✅

- README rewritten: semantic-merge-first narrative (126 lines)
- `docs/why-suture.md`: problem statement, ASCII diagrams, 16 supported formats
- `docs/video-editors.md`: OTIO timeline version control for NLE workflows
- `docs/document-authors.md`: DOCX/XLSX/PPTX merge for document collaboration
- `docs/data-science.md`: experiment branching with semantic merge
- `docs/comparing-with-git.md`: honest comparison, positioned as complementary
- `docs/index.html`: updated with domain cards and semantic diff visual

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 1056 passing | 0 failures across 28 crates (2 ignored: FUSE root-only) |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 28 Criterion functions | repo ops, semantic merge, protocol, compression |
| Clippy | ✅ Zero warnings | `cargo clippy --workspace -- -D warnings` clean |
| Ed25519 signing | ✅ Wired into push | `suture key generate`, auto-sign on push |
| E2E tests | ✅ 27 integration tests | init→commit→branch→merge→gc→fsck→bisect→tag→stash |
| Lean 4 proofs | ✅ 23 theorems | TouchSet, commutativity, DAG, LCA, merge properties |
| HTTP integration | ✅ 61 tests (with features) | handshake, repos, patches, push/pull, V2, auth, mirrors, CRUD, search, batch, health |
| Semantic drivers | ✅ 16 drivers | JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, OTIO, SQL, PDF, Image, Example, Properties |
| Editor plugins | ✅ 3 plugins | Neovim (Lua), JetBrains IntelliJ (Kotlin), VS Code (TypeScript) |
| Language bindings | ✅ 2 bindings | Node.js (napi-rs), Python (PyO3) |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 279 | Core engine (CAS, DAG, patches, repo, engine, signing, merge, stash, reset, cherry-pick, rebase, blame, reflog, rm, mv, notes, gc, fsck, squash, patch composition, conflict classification, file-type detection, semantic diff formatter) |
| suture-protocol | 55 | Wire protocol, V2 handshake, delta encoding, compression |
| suture-cli | 25 | CLI binary (37 commands) |
| suture-tui | 31 | Terminal UI (7 tabs: status, log, staging, diff, branches, remote, help) |
| suture-hub | 61 | Hub daemon with SQLite, auth, replication, mirrors, branch protection, CRUD, search, cursor-based pagination, gRPC (14 RPCs), S3 blob backend (opt-in), Raft consensus (opt-in, TCP multi-node), webhooks (push/branch events), health check, graceful shutdown, TOML config, request tracing, persistent rate limiter, batch patches |
| suture-daemon | 33 | File watcher, auto-commit, auto-sync, SHM status, PID management, signal handling, mount manager (FUSE/WebDAV lifecycle) |
| suture-driver | 8 | SutureDriver trait, DriverRegistry, semantic diff/merge types |
| suture-ooxml | 4 | Shared OOXML infrastructure (ZIP, part navigation) |
| suture-driver-otio | 13 | OpenTimelineIO reference driver |
| suture-driver-json | 47 | JSON semantic driver |
| suture-driver-yaml | 30 | YAML semantic driver |
| suture-driver-toml | 30 | TOML semantic driver |
| suture-driver-csv | 27 | CSV semantic driver |
| suture-driver-xml | 31 | XML semantic driver |
| suture-driver-markdown | 41 | Markdown semantic driver |
| suture-driver-docx | 7 | DOCX semantic driver |
| suture-driver-xlsx | 5 | XLSX semantic driver |
| suture-driver-pptx | 7 | PPTX semantic driver |
| suture-driver-sql | 18 | SQL DDL semantic driver (schema diff, three-way merge) |
| suture-driver-pdf | 12 | PDF semantic driver (text extraction, page-level diff/merge) |
| suture-driver-image | 12 | Image metadata driver (dimensions, color type, 10 formats) |
| suture-vfs | 28 | FUSE3 read/write mount, WebDAV server, inode allocation, path translation (2 ignored integration) |
| suture-node | 0 | Node.js native addon (napi-rs) |
| suture-lsp | 11 | Language Server Protocol (hover, diagnostics) |
| suture-e2e | 197 | End-to-end workflow tests + 121 driver correctness tests (realistic + unit) |
| suture-fuzz | 6 | Fuzz testing (CAS hash, patch serialization, merge, touch-set) |
| suture-bench | — | Criterion benchmarks (44 functions: 28 core + 16 perf baselines) |
| suture-raft | 30 | Raft consensus protocol (election, replication, commit, 3-node cluster simulation, persisted log) |
| suture-s3 | 26 | S3-compatible blob storage (AWS SigV4, path/virtual-hosted, MinIO, integration tests) |
| desktop-app | — | Tauri v2 scaffold (9 IPC commands) |
| jetbrains-plugin | — | IntelliJ Platform plugin (10 actions, VCS root detection, Kotlin) |
| suture-py | — | Python bindings (PyO3, notes, worktree, blame, bisect, remotes) |

## Git History

| Commit | Version | Description |
|--------|---------|-------------|
| `77ad798` | v3.2.0 | Directions T–X: ship, CI/CD, real-world drivers, workflow tests, benchmarks |
| `50526ec` | v3.1.0 | Directions O–S: publish prep, driver audits, semantic diff, domain docs |
| `6a389a4` | v3.0.0 | Directions J–N: Raft E2E, production readiness, perf, desktop, ecosystem |
| `356b7e8` | v2.10.0 | Release v2.10.0: Directions H+I complete |
| `546ee5c` | v2.10.0 | Add shipping checklist, release notes, release script |
| `525bb08` | v2.10.0 | Wire S3 and Raft runtime into suture-hub binary |
| `b4249a4` | v2.9.0 | Release v2.9.0: Direction G Growth complete |
| `42c6162` | v2.9.0 | Update Cargo.lock for hub dependencies |
| `3167aad` | v2.9.0 | Desktop app: real web UI with 6 views |
| `fed070c` | v2.9.0 | VS Code extension (14 commands, TypeScript) |
| `c1727ae` | v2.9.0 | Webhook system (push/branch events, HMAC signing) |
| `cfb7f4d` | v2.9.0 | Fix repo_log O(n²) → O(n) performance |
| `852302b` | v2.8.0 | Release v2.8.0: Direction F Production Hardening complete |
| `f2be791` | v2.8.0 | Update Cargo.lock for new dependencies |
| `eddcedd` | v2.8.0 | S3 integration tests (MinIO-compatible) |
| `16aa2e4` | v2.8.0 | FUSE and WebDAV integration tests |
| `e0eb423` | v2.8.0 | Benchmark analysis with optimization recommendations |
| `b36642d` | v2.8.0 | Raft 3-node cluster simulation (8 tests) |
| `ed90f7c` | v2.8.0 | Wire S3 and Raft into hub (pluggable backends) |
| `30bcc51` | v2.7.0 | Release v2.7.0: Direction E Ship It complete |
| `b1440c6` | v2.7.0 | Packaging: Homebrew, AUR, crates.io publish guide |
| `d0410c0` | v2.7.0 | User docs: quickstart, semantic merge, CLI reference, hub, landing page |
| `116d7a6` | v2.7.0 | CONTRIBUTING.md, PR template, release workflow fix |
| `42677f7` | v2.6.0 | Update Cargo.lock for new crates |
| `52ea825` | v2.6.0 | Enhance Python bindings: notes, worktree, blame, bisect, remotes |
| `016d1ae` | v2.6.0 | JetBrains IntelliJ plugin (10 actions, VCS root detection) |
| `64033e6` | v2.6.0 | S3 blob storage backend (AWS SigV4, MinIO compatible) |
| `92519cf` | v2.6.0 | Raft consensus protocol (election, replication, commit) |
| `2834ce4` | v2.6.0 | Mount manager for FUSE/WebDAV lifecycle management |
| `dedcbed` | v2.5.0-post | Update VERSION.md: gRPC wired, cursor pagination |
| `fb73de5` | v2.5.0-post | Cursor-based pagination for hub API endpoints |
| `042800d` | v2.5.0-post | Wire up gRPC server with all 14 RPCs |
| `213aa2a` | v2.5.0-post | Fix: suture add . directory expansion |
| `5cfcbbf` | v2.5.0-post | Polish: README rewrite, CI migration, CLI version |
| `02f603d` | v2.5.0-alpha.3 | gRPC transport scaffold with proto definition |
| `df1980d` | v2.5.0-alpha.2 | Neovim plugin + Node.js bindings (napi-rs) |
| `fe95255` | v2.5.0-alpha.1 | Three new semantic drivers — SQL, PDF, Image |
| `998811a` | v2.1.0-alpha.1 | Daemon SHM status, PID management, signal handling |
| `ff10493` | v2.0.0-alpha.3 | WebDAV cross-platform mount + Desktop app |
| `d31b00c` | v2.0.0-alpha.2 | FUSE read-write VFS — file saves create patches |
| `5087ee2` | v2.0.0-alpha.1 | FUSE read-only VFS prototype (Direction B start) |
| `96daac2` | v1.3.1 | Portal completion — file tree API, web UI rebuild, TUI remote/conflict/log graph |
| `bbd30c4` | v1.3.0 | Hub API expansion — 9 new CRUD routes, 10 HTTP tests, 6 web UI bugfixes |
