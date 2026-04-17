# Suture Version

- **Current Version:** 2.0.0-alpha.1
- **Current Phase:** Direction B — VFS Prototype (v2.0)
- **Status:** In Progress
- **Last Updated:** 2026-04-17
- **Rust Edition:** 2024
- **Lean 4:** v4.29.1 (23 theorems proved)

## Strategic Roadmap

| Phase | Direction | Focus | Target Version |
|-------|-----------|-------|---------------|
| **A** | Product Polish | Hub as self-hosted GitLab/Gitea portal | v1.3 – v1.4 |
| **B** | Enterprise Infra | VFS loopback mount + Daemon + SHM | v2.0 – v2.1 |
| **C** | Ecosystem Growth | Drivers, plugins, language bindings | v2.5+ |

### Direction A — Product Polish (v1.3–v1.4)

- Hub web portal: file browser, repo detail pages, diff viewer, user registration
- Desktop app: finish Tauri scaffold into working cross-platform app
- Make Surable usable as a self-hosted service out of the box

### Direction B — Enterprise Infrastructure (v2.0–v2.1)

- FUSE/NFSv4 user-space VFS so NLEs see Suture repos as regular directories
- Background daemon with SHM for nanosecond status queries
- gRPC/QUIC transport, Raft consensus, S3 blob backend

### Direction C — Ecosystem Growth (v2.5+)

- More format drivers (PDF, databases, image/video metadata)
- Editor plugins (JetBrains, Neovim)
- Language bindings (Python/PyO3, Node.js)
- WASM plugin system

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 749+ passing | 0 failures across 21 crates |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 28 Criterion functions | repo ops, semantic merge, protocol, compression |
| Clippy | ✅ Zero warnings | `cargo clippy --workspace -- -D warnings` clean |
| Ed25519 signing | ✅ Wired into push | `suture key generate`, auto-sign on push |
| E2E tests | ✅ 7 integration tests | init→commit→branch→merge→gc→fsck→bisect→tag→stash |
| Lean 4 proofs | ✅ 23 theorems | TouchSet, commutativity, DAG, LCA, merge properties |
| HTTP integration | ✅ 33 tests | handshake, repos, patches, push/pull, V2, auth, mirrors, CRUD, search |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 271 | Core engine (CAS, DAG, patches, repo, engine, signing, merge, stash, reset, cherry-pick, rebase, blame, reflog, rm, mv, notes, gc, fsck, squash, patch composition, conflict classification) |
| suture-protocol | 55 | Wire protocol, V2 handshake, delta encoding, compression |
| suture-cli | 25 | CLI binary (37 commands) |
| suture-tui | 26 | Terminal UI (6 tabs: status, log, staging, diff, branches, help) |
| suture-hub | 137 | Hub daemon with SQLite, auth, replication, mirrors, branch protection, CRUD, search |
| suture-daemon | 21 | File watcher, auto-commit, auto-sync |
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
| suture-lsp | 11 | Language Server Protocol (hover, diagnostics) |
| suture-e2e | 27 | End-to-end workflow integration tests |
| suture-fuzz | 6 | Fuzz testing (CAS hash, patch serialization, merge, touch-set) |
| suture-bench | — | Criterion benchmarks (28 functions) |
| desktop-app | — | Tauri v2 scaffold (9 IPC commands) |
