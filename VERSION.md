# Suture Version

- **Current Version:** 2.5.0
- **Current Phase:** Direction C — Ecosystem Growth (v2.5)
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

### Direction A — Product Polish (v1.3–v1.4) ✅

- Hub web portal: file browser, repo detail pages, diff viewer, user registration
- TUI: 7 tabs, checkout confirmation, merge conflict view, log graph
- Desktop app: Tauri v2 scaffold with IPC commands

### Direction B — Enterprise Infrastructure (v2.0–v2.5) ✅

- FUSE3 read/write VFS so NLEs see Suture repos as regular directories
- WebDAV cross-platform mount (macOS Finder, Windows Explorer)
- Background daemon with SHM for nanosecond status queries
- PID file management and signal handling (SIGTERM, SIGHUP)
- gRPC transport scaffold with proto definition (14 RPCs)

### Direction C — Ecosystem Growth (v2.5) ✅

- SQL semantic driver: DDL parsing, schema diff, three-way merge
- PDF semantic driver: text extraction via lopdf, page-level diff/merge
- Image metadata driver: dimension/color detection, 10 formats
- Neovim plugin: 10 commands, gutter signs, float windows
- Node.js bindings: napi-rs native addon with TypeScript declarations

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 835 passing | 0 failures across 26 crates |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 28 Criterion functions | repo ops, semantic merge, protocol, compression |
| Clippy | ✅ Zero warnings | `cargo clippy --workspace -- -D warnings` clean |
| Ed25519 signing | ✅ Wired into push | `suture key generate`, auto-sign on push |
| E2E tests | ✅ 27 integration tests | init→commit→branch→merge→gc→fsck→bisect→tag→stash |
| Lean 4 proofs | ✅ 23 theorems | TouchSet, commutativity, DAG, LCA, merge properties |
| HTTP integration | ✅ 38 tests | handshake, repos, patches, push/pull, V2, auth, mirrors, CRUD, search |
| Semantic drivers | ✅ 16 drivers | JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, OTIO, SQL, PDF, Image, Example, Properties |
| Editor plugins | ✅ 1 plugin | Neovim (Lua) |
| Language bindings | ✅ 1 binding | Node.js (napi-rs) |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 271 | Core engine (CAS, DAG, patches, repo, engine, signing, merge, stash, reset, cherry-pick, rebase, blame, reflog, rm, mv, notes, gc, fsck, squash, patch composition, conflict classification) |
| suture-protocol | 55 | Wire protocol, V2 handshake, delta encoding, compression |
| suture-cli | 25 | CLI binary (37 commands) |
| suture-tui | 31 | Terminal UI (7 tabs: status, log, staging, diff, branches, remote, help) |
| suture-hub | 38 | Hub daemon with SQLite, auth, replication, mirrors, branch protection, CRUD, search, gRPC scaffold |
| suture-daemon | 27 | File watcher, auto-commit, auto-sync, SHM status, PID management, signal handling |
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
| desktop-app | — | Tauri v2 scaffold (9 IPC commands) |

## Git History

| Commit | Version | Description |
|--------|---------|-------------|
| `02f603d` | v2.5.0-alpha.3 | gRPC transport scaffold with proto definition |
| `df1980d` | v2.5.0-alpha.2 | Neovim plugin + Node.js bindings (napi-rs) |
| `fe95255` | v2.5.0-alpha.1 | Three new semantic drivers — SQL, PDF, Image |
| `998811a` | v2.1.0-alpha.1 | Daemon SHM status, PID management, signal handling |
| `ff10493` | v2.0.0-alpha.3 | WebDAV cross-platform mount + Desktop app |
| `d31b00c` | v2.0.0-alpha.2 | FUSE read-write VFS — file saves create patches |
| `5087ee2` | v2.0.0-alpha.1 | FUSE read-only VFS prototype (Direction B start) |
| `96daac2` | v1.3.1 | Portal completion — file tree API, web UI rebuild, TUI remote/conflict/log graph |
| `bbd30c4` | v1.3.0 | Hub API expansion — 9 new CRUD routes, 10 HTTP tests, 6 web UI bugfixes |
