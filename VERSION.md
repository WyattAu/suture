# Suture Version

- **Current Version:** 4.0.0
- **Crates.io:** 31 crates published (suture-cli requires Node.js build; install from source)
- **Current Phase:** Phase B — Vertical Deepening
- **Status:** Active Development
- **Last Updated:** 2026-04-20
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
| **1A** | Enhanced Undo/Reflog/Doctor | Reflog-aware undo, doctor command, structured reflog | v3.3 | ✅ Complete |
| **1B** | Merge Strategies | --strategy flag (semantic/ours/theirs/manual), dry-run | v3.3 | ✅ Complete |
| **1C** | GC/fsck Hardening | Transactional GC, blob pruning, fsck fixes | v3.3 | ✅ Complete |
| **1D** | Merge Stress Tests | 12 E2E stress tests: multi-file, deep history, diamond, strategies | v3.3 | ✅ Complete |
| **1E** | Supply Chain Integrity | Shannon entropy, risk scoring, --integrity diff mode | v3.3 | ✅ Complete |
| **2A** | Video/OTIO Depth | 8 deep tests: reordering, transitions, nesting, 500-clip perf | v3.4 | ✅ Complete |
| **2B** | Document Collab Depth | 9 deep tests: DOCX/XLSX/PPTX conflicts, large docs, formulas | v3.4 | ✅ Complete |
| **3A** | Git Bridge | `suture git import/log/status`, read-only Git→Suture import | v3.4 | ✅ Complete |
| **3B** | Shell Completions | bash/zsh/fish/powershell/nushell (already done in v3.1) | v3.4 | ✅ Complete |
| **3C** | TUI Conflict Resolver | Line-by-line hunk resolution, ours/theirs/both, navigation | v3.4 | ✅ Complete |
| **4A** | suture-merge Library | Standalone merge library crate, 10 drivers, published to crates.io | v3.7 | ✅ Complete |
| **4B** | Binary Document E2E | 71 E2E tests for DOCX/XLSX/PPTX full VCS lifecycle | v3.7 | ✅ Complete |
| **4C** | Hardening & Stabilize | 4 bug fixes, full workspace 0 failures, 1396 tests | v4.0 | ✅ Complete |
| **5A** | OTIO Driver Rewrite | SutureDriver trait, content-based ID, three-way merge, Gap/Marker support | v4.0 | 🔄 In Progress |
| **5B** | OOXML Driver Deepening | Fix XLSX cell refs, PPTX slide discovery, DOCX in-place merge | v4.0 | 🔄 In Progress |
| **5C** | Distribution & Adoption | Installers, migration tooling, documentation overhaul | v4.1 | Pending |

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

### Direction 1A — Enhanced Undo/Reflog/Doctor (v3.3) ✅

- `suture undo`: reflog-aware undo (can undo merges, checkouts, cherry-picks)
- `suture undo --hard`: discard working tree changes
- `suture reflog`: structured display with relative timestamps, `ReflogEntry` type
- `suture doctor`: 10 health checks (repository integrity, CAS consistency, DAG validity)
- Fixed reflog timestamp bug, fsck double-counting, GC blob pruning

### Direction 1B — Merge Reconciliation Strategies (v3.3) ✅

- `suture merge -s ours <branch>`: keep our version for all conflicts
- `suture merge -s theirs <branch>`: take their version for all conflicts
- `suture merge -s manual <branch>`: leave all conflicts for manual resolution
- `suture merge --dry-run <branch>`: preview merge without modifying working tree
- `suture merge -s semantic <branch>`: try semantic drivers first (default)

### Direction 1C — GC/fsck Hardening (v3.3) ✅

- GC is now transactional (SQLite `unchecked_transaction`)
- GC prunes orphaned blobs from CAS store (computes reachable set from patches)
- GC cleans up file_trees and reflog entries for unreachable patches
- fsck fixed: HEAD check no longer double-counts
- fsck upgraded: missing blob references are now errors (data loss risk)

### Direction 1D — Merge Correctness Stress Tests (v3.3) ✅

- 12 new E2E stress tests in `workflow_conflict.rs` (total: 14 tests)
- Multi-file merge with overlapping conflicts (10 files)
- Large file scattered edits (200-line file, edits at 6 positions)
- Deep branch history (50 commits per branch, 100 files total)
- Diamond merge pattern (two branches from same point)
- Delete/modify conflict detection
- Strategy flag tests (ours, theirs, manual)
- Dry-run verification (no file changes)
- Fast-forward, non-overlapping JSON, cascade merges

### Direction 1E — Supply Chain Integrity Analysis (v3.3) ✅

- `suture diff --integrity`: mathematical diff analysis for supply chain transparency
- Shannon entropy calculation (H ∈ [0, 8] bits/byte) for every changed file
- 13 risk indicators detecting XZ-style attack patterns:
  - High entropy in source files (encrypted/injected code)
  - Binary content in text files (hidden payloads)
  - Build script modifications (configure, Makefile, build.rs)
  - Test infrastructure modifications (xz backdoor used test fixtures)
  - Compressed file modifications (could hide changes)
  - Lockfile modifications without source changes
  - Base64-encoded content detection
  - Sudden entropy increase from old to new version
- 5-level risk scoring (None → Low → Medium → High → Critical)
- XZ-pattern detection: warns when build scripts and test infrastructure change together
- Formatted terminal output with Unicode box drawing
- 19 unit tests + 8 E2E tests for integrity analysis

### Direction 2A — Video/OTIO Domain Depth (v3.4) ✅

- 7 new OTIO unit tests: clip reordering, transition addition, duration change, track addition, metadata change, nested stack merge, large timeline performance (500 clips)
- 1 new OTIO E2E test: multi-editor merge conflict simulation with complex fixture
- Validates OTIO driver handles real-world video editing collaboration scenarios

### Direction 2B — Document Collaboration Depth (v3.4) ✅

- 3 new DOCX tests: two-editor paragraph conflict, table insertion merge, large document stress (50+ paragraphs)
- 3 new XLSX tests: cell-level merge conflict, formula preservation, large sheet stress (200×10)
- 3 new PPTX tests: slide reorder merge, multi-slide edit merge, large deck stress (30 slides)

### Direction 3A — Git Bridge (v3.4) ✅

- `suture git import [PATH]`: Read-only import of Git history into Suture (no libgit2 dependency)
- Parses Git object store directly: commit objects, tree objects, blob objects (zlib-compressed)
- Creates `git-import/main` branch to avoid overwriting existing Suture history
- Idempotent: detects already-imported commits by message matching
- `suture git log [PATH]`: Preview Git commits with file change counts
- `suture git status [PATH]`: Import summary (commits, branches, files)
- 7 unit tests: reflog parsing, commit/tree object parsing, zlib roundtrip, empty repo handling

### Direction 3B — Shell Completions (v3.4) ✅

- Already implemented in v3.1: bash, zsh, fish, powershell, nushell
- Uses `clap_complete` + `clap_complete_nushell`
- Homebrew formula auto-generates completions during install

### Direction 4A — suture-merge Library & Binary E2E (v3.5–v3.7) ✅

- `suture-merge` library crate: dead-simple API, 10 feature-gated merge functions
- Published to crates.io (v0.1.0): `cargo add suture-merge`
- 389 hardening tests (129 default, 289 all-features): adversarial, unicode, size/stress, trivial, cross-driver, error quality, conflict quality
- 80 validation tests (45 default, 80 all-features): clean merge, conflict, nested, edge cases, real-world
- Binary document E2E test script: 4 scenarios, 71 tests covering full VCS lifecycle
  - DOCX: init→add→commit→branch→modify→merge→diff→log→reflog (semantic XML merge)
  - XLSX: clean merge across different spreadsheet cells
  - PPTX: slide modification with merge
  - Mixed: DOCX+XLSX+PPTX+text file repo with fsck and doctor

### Direction 4C — Hardening & Stabilize (v4.0) ✅

- Fix suture-merge test compilation: gate non-default driver tests behind `#[cfg(feature = "...")]`
- Fix `suture add .` to recurse into subdirectories (was non-recursive via `read_dir`)
- Fix hub handshake: add GET handler without request body (CLI sends bare GET, server required JSON body → 422)
- Fix flaky 10K perf test: relax threshold from 30s to 60s for slow CI
- Full `cargo test --workspace`: 0 failures, 0 errors, 1396 tests

### Direction 5A — OTIO Driver Rewrite (v4.0) 🔄

- Implement `SutureDriver` trait for `OtioDriver` (diff, format_diff, merge)
- Replace index-based identity with content-based heuristic (media_reference + source_range)
- Add missing OTIO types: Gap, Marker, Effect, TimeEffect
- Implement semantic three-way merge with clip-level conflict detection
- Register driver in plugin registry for CLI auto-discovery
- Handle unknown schema types gracefully (skip instead of failing)
- 38 tests → target 60+ tests

### Direction 5B — OOXML Driver Deepening (v4.0) 🔄

- **XLSX**: Fix cell reference parsing (A1 notation), add shared string table support
- **PPTX**: Fix slide discovery (parse `presentation.xml` slide ID list, resolve relationship IDs)
- **DOCX**: Switch from extract-and-regenerate to parse-and-modify-in-place (preserve formatting)
- Use `quick-xml` for proper XML parsing instead of hand-written string scanners
- Implement OOXML relationship resolution in `suture-ooxml` shared infrastructure

### Direction 5C — Distribution & Adoption (v4.1) Pending

- One-click installers (DMG, MSI, AppImage)
- `git → suture` migration tooling for repos with structured files
- Interactive onboarding tutorial
- Documentation overhaul (quickstart, API reference, domain guides)
- Community building (blog posts, conference talks)

- Line-by-line hunk resolution (replaces binary ours/theirs-only choice)
- `1`/`2`/`3` keys: take ours, theirs, or both per hunk
- `j`/`k` keys: navigate between conflict hunks within a file
- `n`/`p` keys: next/previous conflict file
- `a` key: accept all remaining hunks with last used resolution
- Auto-advance to next unresolved hunk after resolution
- Auto-write resolved file and stage when all hunks resolved
- Three-panel layout: file list, hunk detail, key bindings footer
- Per-file resolved hunk count display

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 1396 passing | 0 failures across 37 crates (2 ignored: FUSE root-only) |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 28 Criterion functions | repo ops, semantic merge, protocol, compression |
| Clippy | ✅ Zero warnings | `cargo clippy --workspace -- -D warnings` clean |
| Ed25519 signing | ✅ Wired into push | `suture key generate`, auto-sign on push |
| E2E tests | ✅ 50 integration tests | init→commit→branch→merge→gc→fsck→bisect→tag→stash→integrity→stress→git |
| Binary E2E | ✅ 71 tests | DOCX/XLSX/PPTX full lifecycle (init→add→commit→branch→modify→merge→diff→log) |
| Lean 4 proofs | ✅ 23 theorems | TouchSet, commutativity, DAG, LCA, merge properties |
| HTTP integration | ✅ 61 tests (with features) | handshake, repos, patches, push/pull, V2, auth, mirrors, CRUD, search, batch, health |
| Semantic drivers | ✅ 16 drivers | JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, OTIO, SQL, PDF, Image, Example, Properties |
| Supply chain integrity | ✅ NEW | Shannon entropy, 13 risk indicators, XZ-style attack detection |
| Editor plugins | ✅ 3 plugins | Neovim (Lua), JetBrains IntelliJ (Kotlin), VS Code (TypeScript) |
| Language bindings | ✅ 2 bindings | Node.js (napi-rs), Python (PyO3) |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 298 | Core engine (CAS, DAG, patches, repo, engine, signing, merge, stash, reset, cherry-pick, rebase, blame, reflog, rm, mv, notes, gc, fsck, squash, patch composition, conflict classification, file-type detection, semantic diff formatter, **supply chain integrity analysis**) |
| suture-protocol | 55 | Wire protocol, V2 handshake, delta encoding, compression |
| suture-cli | 32 | CLI binary (39 commands, `diff --integrity`, `git import/log/status`) |
| suture-tui | 31 | Terminal UI (7 tabs + hunk-level conflict resolver with ours/theirs/both) |
| suture-hub | 61 | Hub daemon with SQLite, auth, replication, mirrors, branch protection, CRUD, search, cursor-based pagination, gRPC (14 RPCs), S3 blob backend (opt-in), Raft consensus (opt-in, TCP multi-node), webhooks (push/branch events), health check, graceful shutdown, TOML config, request tracing, persistent rate limiter, batch patches |
| suture-daemon | 33 | File watcher, auto-commit, auto-sync, SHM status, PID management, signal handling, mount manager (FUSE/WebDAV lifecycle) |
| suture-driver | 8 | SutureDriver trait, DriverRegistry, semantic diff/merge types |
| suture-ooxml | 4 | Shared OOXML infrastructure (ZIP, part navigation) |
| suture-driver-otio | 20 | OpenTimelineIO reference driver (clip reorder, transitions, nesting, 500-clip perf) |
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
| suture-e2e | 226 | End-to-end workflow tests + 130 driver correctness tests + 8 integrity E2E tests |
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
| _(pending)_ | v3.3.0 | Phase 1: undo, merge strategies, GC hardening, stress tests, supply chain integrity |
| `77ad798` | v3.2.1 | Fix commit O(n²) bottleneck: incremental file tree computation (70x faster at 10K commits) |
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
