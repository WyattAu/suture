# Suture Version

- **Current Version:** 2.9.0
- **Current Phase:** Direction G — Growth
- **Status:** Complete
- **Last Updated:** 2026-04-17
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

### Direction G — Growth (v2.9) ✅

- VS Code extension: 14 commands, SutureHelper class, output channel, quick pick, SVG icon
- Webhook system: CRUD routes, async fire-and-forget delivery, HMAC-SHA256 signing, push/branch events
- Desktop app: real web UI with 6 views, dark theme, commit modal, branch management
- Performance fix: repo_log O(n²) → O(n) via HashSet cycle detection
- Key benchmark finding: repo_log/1000 now completes (was timeout)

### Direction F — Production Hardening (v2.8) ✅

- S3 blob backend wired into hub: `BlobBackend` trait, `SqliteBlobBackend`, `S3BlobBackendAdapter`
- Raft consensus wired into hub: `RaftHub` wrapper, `HubCommand` enum, cluster config (both opt-in features)
- Raft 3-node cluster simulation with 8 integration tests
- FUSE integration tests: mount read/write/modify/delete/stat, WebDAV serve test
- S3 integration tests: 7 MinIO-compatible tests gated on env vars
- Benchmark analysis: 28 functions profiled, 5 optimization opportunities identified
- Key finding: `repo_log` O(n²) at scale, commit 652ms for 1000 files

- User documentation: quickstart, semantic merge guide, CLI reference, hub guide
- GitHub Pages landing page (dark terminal aesthetic)
- CONTRIBUTING.md updated for v2.7.0
- PR template with quality gate checklist
- Release workflow: 5-platform matrix (Linux x86/ARM, macOS x86/ARM, Windows)
- Homebrew formula with test block
- AUR PKGBUILD for Arch Linux
- crates.io publish guide with dependency order

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 898 passing | 0 failures across 28 crates (2 ignored: FUSE root-only) |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 28 Criterion functions | repo ops, semantic merge, protocol, compression |
| Clippy | ✅ Zero warnings | `cargo clippy --workspace -- -D warnings` clean |
| Ed25519 signing | ✅ Wired into push | `suture key generate`, auto-sign on push |
| E2E tests | ✅ 27 integration tests | init→commit→branch→merge→gc→fsck→bisect→tag→stash |
| Lean 4 proofs | ✅ 23 theorems | TouchSet, commutativity, DAG, LCA, merge properties |
| HTTP integration | ✅ 38 tests | handshake, repos, patches, push/pull, V2, auth, mirrors, CRUD, search |
| Semantic drivers | ✅ 16 drivers | JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, OTIO, SQL, PDF, Image, Example, Properties |
| Editor plugins | ✅ 3 plugins | Neovim (Lua), JetBrains IntelliJ (Kotlin), VS Code (TypeScript) |
| Language bindings | ✅ 2 bindings | Node.js (napi-rs), Python (PyO3) |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 271 | Core engine (CAS, DAG, patches, repo, engine, signing, merge, stash, reset, cherry-pick, rebase, blame, reflog, rm, mv, notes, gc, fsck, squash, patch composition, conflict classification) |
| suture-protocol | 55 | Wire protocol, V2 handshake, delta encoding, compression |
| suture-cli | 25 | CLI binary (37 commands) |
| suture-tui | 31 | Terminal UI (7 tabs: status, log, staging, diff, branches, remote, help) |
| suture-hub | 46 | Hub daemon with SQLite, auth, replication, mirrors, branch protection, CRUD, search, cursor-based pagination, gRPC (14 RPCs), S3 blob backend (opt-in), Raft consensus (opt-in), webhooks (push/branch events) |
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
| suture-e2e | 27 | End-to-end workflow integration tests |
| suture-fuzz | 6 | Fuzz testing (CAS hash, patch serialization, merge, touch-set) |
| suture-bench | — | Criterion benchmarks (28 functions) |
| suture-raft | 21 | Raft consensus protocol (election, replication, commit, 3-node cluster simulation) |
| suture-s3 | 26 | S3-compatible blob storage (AWS SigV4, path/virtual-hosted, MinIO, integration tests) |
| desktop-app | — | Tauri v2 scaffold (9 IPC commands) |
| jetbrains-plugin | — | IntelliJ Platform plugin (10 actions, VCS root detection, Kotlin) |
| suture-py | — | Python bindings (PyO3, notes, worktree, blame, bisect, remotes) |

## Git History

| Commit | Version | Description |
|--------|---------|-------------|
| `42c6162` | v2.9.0 | Update Cargo.lock for hub dependencies |
| `3167aad` | v2.9.0 | Desktop app: real web UI with 6 views |
| `fed070c` | v2.9.0 | VS Code extension (14 commands, TypeScript) |
| `c1727ae` | v2.9.0 | Webhook system (push/branch events, HMAC signing) |
| `cfb7f4d` | v2.9.0 | Fix repo_log O(n²) → O(n) performance |
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
