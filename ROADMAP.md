# Suture Production Roadmap

**Version:** 5.4.0 (Phase 1-3 complete)
**Date:** 2026-05-12
**Author:** Full monorepo audit and architectural review
**Status:** Phase 1 ✅ | Phase 2 ✅ | Phase 3 ✅ | Phase 4 pending

## 0. Current State (Updated 2026-05-12)

---

## 0. Current State

### 0.1 Quantitative Baseline

| Metric | Value |
|--------|-------|
| Workspace crates | 43 |
| Rust LoC | 108,029 |
| Test functions | 1,747 |
| Test failures | 0 |
| Clippy warnings | 0 |
| Semantic drivers | 17 |
| CLI subcommands | 58+ |
| Lean 4 proofs | 8 theorems |
| Unsafe blocks (production) | 51 |
| CI workflows | 8 |
| crates.io crates | 37 |
| Editor plugins | 3 (Neovim, JetBrains, VS Code) |
| Language bindings | 2 (Node.js, Python) |

### 0.2 Maturity Assessment

| Layer | Status | Notes |
|-------|--------|-------|
| Core VCS engine | Production-ready | 355 tests, 21 proptest suites, formal proofs |
| Semantic merge | Production-ready | 17 drivers, 56 E2E correctness/realistic tests |
| CLI | Production-ready | 58+ commands, shell completions, man pages |
| Hub (HTTP + gRPC) | Production-ready | Auth, webhooks, S3, Raft, batch patches |
| Wire protocol | Production-ready | V2 handshake, Zstd compression, delta encoding |
| VFS (FUSE3 + WebDAV) | Production-ready | Read/write FUSE, WebDAV server |
| TUI | Production-ready | 7 tabs, hunk-level conflict resolver |
| Desktop App | Scaffolded | Tauri v2, 25+ commands, no CI |
| SaaS Platform | Functional | Stripe billing, OAuth, orgs, merge API |
| Connectors | Scaffolded | Airtable (functional), Google Sheets, Notion |
| WASM plugins | Experimental | ABI, fuel limits, sandbox |
| Python bindings | Excluded | PyO3, not in CI |
| Helm chart | Scaffolded | Basic templates, no README docs |

### 0.3 Technical Debt Register

| ID | Severity | Description | Effort |
|----|----------|-------------|--------|
| TD-1 | Critical | CLI CWD mutex forces --test-threads=1 in all CI | 3d |
| TD-2 | Critical | FUSE unsafe impl Send/Sync soundness | 2d |
| TD-3 | Critical | SHM unsafe impl Send/Sync -- no repr(C) proof | 0.5d |
| TD-4 | Critical | from_utf8_unchecked in 8+ binary drivers -- needs debug-assert | 1d |
| TD-5 | High | suture-py excluded from workspace/CI | 2d |
| TD-6 | High | desktop-app excluded from workspace/CI | 3d |
| TD-7 | High | No MSRV policy or rust-toolchain.toml pin | 0.5d |
| TD-8 | High | 51 unsafe blocks need SAFETY audit and comments | 3d |
| TD-9 | Medium | Code coverage measured but no threshold enforcement | 1d |
| TD-10 | Medium | No performance regression gating in CI | 2d |
| TD-11 | Medium | requirements.md lists deferred features as if planned | 0.5d |
| TD-12 | Low | 5 E2E tests ignored (FUSE needs root) | 2d |
| TD-13 | Low | Desktop app not in workspace | 1d |

---

## Phase 1: Hardening and Soundness (v5.4)

**Goal:** Eliminate all soundness concerns and critical technical debt.

**Duration:** 2 weeks
**Exit criteria:** Zero critical TD items. All unsafe blocks documented. Tests run with --test-threads > 1.

### 1.1 Unsafe Audit

Audit all 51 unsafe blocks. For each, add a `// SAFETY:` comment explaining the invariant. Remove any that can be replaced with safe Rust.

**Hotspots:**
- `suture-daemon/src/shm.rs` (9 blocks) -- memory-mapped IPC
- `suture-plugin-sdk/src/lib.rs` (10 blocks) -- WASM host ABI
- `suture-core/src/metadata/global_config.rs` (6 blocks) -- global config
- `suture-vfs/src/fuse/read_write.rs` (4 blocks) -- FUSE callbacks

### 1.2 Soundness Fixes

| Task | Details |
|------|---------|
| TD-1: Eliminate CWD mutex | Refactor CLI integration tests to use `tempdir()` per test with unique CWD. Remove static `Mutex<()>`. Target: `--test-threads=4` in CI. |
| TD-2: FUSE Send/Sync | Audit FUSE callback threading model. libfuse3 guarantees single-threaded callback dispatch per filesystem. Document or remove unsafe impl. |
| TD-3: SHM repr(C) | Add `#[repr(C)]` to `ShmStatus`. Add `const_assert!` for layout stability. Document cross-process layout requirements. |
| TD-4: Binary driver UTF-8 | Add `debug_assert!(std::str::from_utf8(bytes).is_ok())` before every `from_utf8_unchecked` call. In release, the invariant holds because we control ZIP extraction. |

### 1.3 CI Hardening

| Task | Details |
|------|---------|
| Re-include suture-e2e in main test job | Remove exclusion, add --test-threads=1 only for suture-cli |
| Re-include suture-daemon in main test job | Already has 32 passing tests |
| Add clippy --all-features to CI | Currently only default features checked |
| Add cargo doc to CI | Catch documentation build failures |

---

## Phase 2: Requirements Reconciliation (v5.5)

**Goal:** Align documentation with reality. No aspirational claims.

**Duration:** 1 week

### 2.1 Documentation Cleanup

| Task | Details |
|------|---------|
| Reconcile requirements.md | Mark all deferred features (FlatBuffers, QUIC, NFSv4, PostgreSQL, Redis, ProjFS, iceoryx) with `[DEFERRED]` and rationale ADR |
| Update all stale version references | Grep for `5.0.0`, `5.1.0`, `0.8.1` across all .md files |
| Update deploy-runbook.md | Bump version references to 5.3.1 |
| Verify all internal links | Automated link checker in CI |
| Remove librust_out.rmeta from .gitignore | Already removed; add pattern to .gitignore |

### 2.2 ADRs to Write

| ADR | Topic |
|-----|-------|
| ADR-015 | Why SQLite over PostgreSQL (simplicity, single-node, Raft for scale) |
| ADR-016 | Why TCP+Zstd over QUIC (adequate latency, simpler implementation) |
| ADR-017 | Why bincode over FlatBuffers (sufficient performance, lower complexity) |
| ADR-018 | Why FUSE3+WebDAV over NFSv4/SMB3 (user-space, cross-platform) |

---

## Phase 3: Performance Engineering (v6.0)

**Goal:** Establish quantitative baselines and optimize hot paths.

**Duration:** 4 weeks

### 3.1 Benchmark Infrastructure

| Task | Details |
|------|---------|
| Add Criterion to CI | Run suture-bench on every PR, fail if regression > 10% |
| Baseline all operations | Record p50/p95/p99 for init, add, commit, log, merge, push, pull |
| Publish performance.md | Auto-generated from CI benchmarks |

### 3.2 Optimization Targets

| Operation | Current | Target | Approach |
|-----------|---------|--------|----------|
| `suture init` | <100ms | <50ms | Pre-allocate SQLite pages |
| `suture add .` (10K files) | O(n) | O(n), <2s | Parallel file hashing with rayon |
| `suture commit` | <500ms | <200ms | Incremental file tree (already partial) |
| `suture log` (10K commits) | O(n) | O(n), <500ms | Lazy patch deserialization |
| `suture merge` (100 files) | O(n*m) | O(n+m) | Parallel per-file merge |
| `suture push` (1K patches) | Linear | Linear, >50MB/s | Batch blob compression |
| Semantic merge (DOCX 100 paragraphs) | O(p^2) | O(p) | Index-based paragraph lookup |

### 3.3 Scale Testing

| Scenario | Target |
|----------|--------|
| 100K files, 10K commits | All operations < 30s |
| 1K patches, 100K blobs push/pull | > 50MB/s throughput |
| 500-clip OTIO timeline merge | < 5s |

---

## Phase 4: Ecosystem and Distribution (v6.1)

**Goal:** Make suture-merge the standard semantic merge library.

**Duration:** 4 weeks

### 4.1 Library Maturity

| Task | Details |
|------|---------|
| suture-merge API stabilization | Commit to semver. Document stability guarantees. |
| suture-merge v1.0 | If API is stable enough, publish as 1.0. |
| Add merge_sql(), merge_ical(), merge_feed() | Currently missing from library crate |
| Publish all 37 crates | Automated dry-run in CI |

### 4.2 Language Bindings

| Task | Details |
|------|---------|
| suture-py: re-include in CI | Fix PyO3 build, add to test matrix |
| suture-py: publish to PyPI | Automated build and upload |
| suture-node: CI verification | Already in workspace, add dedicated test job |

### 4.3 Editor Integration

| Task | Details |
|------|---------|
| VS Code: real-time merge preview | Show semantic diff inline |
| JetBrains: merge conflict resolution UI | Integrate with IDEA merge tool |
| Neovim: stable release | Tag and publish to MELPA/lazy.nvim |

### 4.4 Distribution

| Task | Details |
|------|---------|
| Homebrew formula update | v6.1 with test block |
| AUR PKGBUILD update | Arch Linux |
| Nix flake update | Pin to v6.1 |
| Docker image multi-arch | linux/amd64, linux/arm64 |
| Install script verification | Test on Ubuntu, macOS, Fedora, Arch |

---

## Phase 5: Enterprise Infrastructure (v7.0)

**Goal:** Hub deployment story is production-grade.

**Duration:** 8 weeks

### 5.1 Hub Hardening

| Task | Details |
|------|---------|
| Backup/restore tooling | `suture hub backup` / `suture hub restore` (SQLite dump + blob export) |
| Prometheus metrics endpoint | `/metrics` with request latency, active connections, repo count |
| Per-user rate limiting | Currently global; scope to authenticated user |
| Replication lag visibility | Raft commit index vs applied index |
| API versioning | `/api/v1/` prefix with deprecation headers |
| OAuth2 improvements | Scope-based tokens, refresh token rotation |
| Hub configuration schema | TOML schema validation on startup |

### 5.2 S3 Backend

| Task | Details |
|------|---------|
| Multipart upload | For blobs > 100MB |
| Automatic retry with exponential backoff | Transient failure handling |
| Bucket lifecycle policies | Automatic blob expiration |

### 5.3 Raft Hardening

| Task | Details |
|------|---------|
| Log compaction | Already implemented; test at scale (1M entries) |
| Membership changes | Already implemented; add CLI commands |
| Snapshot transfer | Optimize large snapshot propagation |
| Leader step-down on network partition | Verify correctness |

### 5.4 Observability

| Task | Details |
|------|---------|
| Structured JSON logging | Replace eprintln with tracing-subscriber |
| Distributed tracing | OpenTelemetry integration |
| Error taxonomy | Structured error codes with machine-readable format |
| Health check endpoint | Already exists; add deep health (DB, S3, Raft) |

---

## Phase 6: Advanced Merge (v7.1)

**Goal:** Expand semantic merge to harder cases.

**Duration:** 6 weeks

### 6.1 Document Merge Depth

| Task | Details |
|------|---------|
| DOCX: track-changes-aware merge | Detect and preserve Word track changes |
| XLSX: formula-aware merge | Detect formula conflicts (not just value) |
| PPTX: animation/timing merge | Preserve slide animations across merges |
| OOXML: comment/annotation merge | Preserve reviewer comments |

### 6.2 New Format Support

| Format | Priority | Notes |
|--------|----------|-------|
| Lockfile (Cargo.lock, package-lock.json) | High | Already have merge strategy scaffold |
| Config files (INI, dotenv, .properties) | Medium | Simple key-value merge |
| Email (MBOX, EML) | Low | Thread-level merge |
| Spreadsheet formulas (XLSX deep) | Medium | AST-level formula diff/merge |

### 6.3 Programmatic Merge API

| Task | Details |
|------|---------|
| Merge conflict callback | Allow callers to resolve conflicts programmatically |
| Custom merge strategies | User-defined conflict resolution per file type |
| Merge plugins | Load strategy from WASM plugin at runtime |

---

## Phase 7: Scale and Reliability (v8.0)

**Goal:** Prove suture works at enterprise scale.

**Duration:** 8 weeks

### 7.1 Large Repository Support

| Task | Details |
|------|---------|
| Partial clone (sparse checkout) | Download only requested paths |
| Shallow clone | Depth-limited history fetch |
| Pack files | Combine small blobs into packed files (like Git pack) |
| Repository size limits | Configurable max repo/blob size with enforcement |

### 7.2 Garbage Collection

| Task | Details |
|------|---------|
| Background GC | Incremental GC running concurrently |
| GC scheduling | Automatic trigger on repo size threshold |
| Parallel fsck | Multi-threaded integrity checking |
| Repair mode | `suture fsck --fix` to correct inconsistencies |

### 7.3 Access Control

| Task | Details |
|------|---------|
| Per-repo permissions | Owner, collaborator, reader roles |
| Per-branch protection | Protected branches, required reviews |
| Team management | Organizations with team-level repo access |
| Audit logging | All permission changes logged |

### 7.4 Reliability

| Task | Details |
|------|---------|
| Concurrent push handling | Hub handles multiple simultaneous pushes |
| Hub-to-hub mirroring | Repository replication across hubs |
| Webhook retry queue | Dead letter queue for failed deliveries |
| Graceful degradation | Degrade features under load, not fail |

---

## Phase 8: Formal Verification Expansion (v8.1)

**Goal:** Expand Lean 4 proof coverage for critical algorithms.

**Duration:** 4 weeks

### 8.1 Current Proofs

| Property | Status |
|----------|--------|
| Touch-set conflict equivalence | Proven |
| Disjoint commutativity | Proven |
| Merge symmetry | Proven |
| Identity element | Proven |
| Merge determinism | Proven |
| Diff determinism | Proven |
| Patch composition associativity | Proven |
| Reflog append-only invariant | Proven |

### 8.2 Target Proofs

| Property | Effort |
|----------|--------|
| Patch-DAG acyclicity invariant | 3d |
| LCA (lowest common ancestor) correctness | 5d |
| Three-way merge completeness | 3d |
| Blob CAS consistency (no aliasing) | 2d |
| Ed25519 signature non-forgeability (assuming curve) | 5d |
| Raft election safety (at most one leader per term) | 5d |
| Conflict marker well-formedness | 2d |
| GC reachability correctness (no live blob pruned) | 3d |

---

## Phase 9: Platform Deepening (v9.0)

**Goal:** Native integrations with professional tools.

**Duration:** 6 weeks

### 9.1 Video/Post-Production

| Task | Details |
|------|---------|
| DaVinci Resolve plugin | Via Python/PyO3 bridge to Resolve scripting API |
| Premiere Pro panel | CEP/UXP extension using suture-merge DLL |
| Final Cut Pro XML round-trip | FCPXML import/export |
| Avid AAF interchange | AAF media metadata merge |

### 9.2 Document/Office

| Task | Details |
|------|---------|
| Excel add-in | Office.js COM add-in for merge preview |
| Google Docs API | Server-side merge via Google Docs API |
| LibreOffice macro | Basic integration for ODF merge |

### 9.3 Data/DevOps

| Task | Details |
|------|---------|
| Airtable connector | Already scaffolded; wire into CLI |
| Google Sheets connector | Already scaffolded; wire into CLI |
| Notion connector | Already scaffolded; wire into CLI |
| Terraform state merge | Semantic merge for .tfstate JSON |
| Kubernetes manifest merge | YAML-aware merge for K8s resources |

---

## Phase 10: Desktop App (v9.1)

**Goal:** Native desktop application for non-developer users.

**Duration:** 4 weeks

### 10.1 Re-include in Workspace

| Task | Details |
|------|---------|
| Fix Tauri build in workspace | Resolve dependency conflicts |
| Add CI matrix | macOS + Windows + Linux build |
| Add smoke tests | Basic init/commit/branch/merge through UI |

### 10.2 Feature Completion

| Task | Details |
|------|---------|
| Real-time sync status | Show push/pull progress |
| Visual merge conflict resolution | Side-by-side editor |
| Repository browser | File tree with history sidebar |
| System tray notifications | Background sync alerts |

---

## Phase 11: WASM Plugin Ecosystem (v9.2)

**Goal:** User-extensible merge drivers via WASM.

**Duration:** 4 weeks

### 11.1 SDK Stabilization

| Task | Details |
|------|---------|
| Publish suture-plugin-sdk to crates.io | Stable API for plugin authors |
| WASM plugin documentation | Tutorial: write a custom merge driver |
| Example plugins | TOML, INI, protobuf as WASM plugins |

### 11.2 Runtime Hardening

| Task | Details |
|------|---------|
| Fuel metering | Enforce CPU time limits per plugin |
| Memory limits | Enforce memory limits per plugin |
| Plugin signing | Verify plugin authenticity |
| Plugin marketplace | Registry for community plugins |

---

## Phase 12: v1.0 Release (v10.0)

**Goal:** Ship a stable, documented, well-supported v1.0.

**Duration:** 4 weeks

### 12.1 Stability

| Task | Details |
|------|---------|
| API stability audit | Semver check on all 37 public crate APIs |
| Breaking change detection | cargo-semver-checks in CI |
| Compatibility matrix | Test on Rust 1.75+, Ubuntu 22.04+, macOS 13+, Windows 10+ |

### 12.2 Documentation

| Task | Details |
|------|---------|
| API reference (rustdoc) | Complete docs for all public types |
| Migration guide | v0.x to v1.0 upgrade path |
| Troubleshooting guide | Common issues and solutions |
| Architecture decision records | All ADRs published |
| Video tutorials | 3-5 minute walkthroughs for core workflows |

### 12.3 Release Infrastructure

| Task | Details |
|------|---------|
| Automated release | GitHub Actions: build, sign, upload, publish |
| SHA256 checksums | For all binary artifacts |
| GPG signing | Detached signatures for release binaries |
| Release notes automation | Changelog to release notes |

---

## Version Timeline

| Version | Focus | Est. Date |
|---------|-------|-----------|
| v5.4 | Hardening and soundness | ✅ 2026-05-12 |
| v5.5 | Requirements reconciliation | ✅ 2026-05-12 |
| v6.0 | Performance engineering | ✅ 2026-05-12 |
| v6.1 | Ecosystem and distribution | 2026-06-14 |
| v7.0 | Enterprise infrastructure | 2026-09-22 |
| v7.1 | Advanced merge | 2026-11-03 |
| v8.0 | Scale and reliability | 2027-01-12 |
| v8.1 | Formal verification expansion | 2027-02-09 |
| v9.0 | Platform deepening | 2027-03-23 |
| v9.1 | Desktop app | 2027-04-20 |
| v9.2 | WASM plugin ecosystem | 2027-05-18 |
| v10.0 | v1.0 release | 2027-06-15 |

---

## Metrics Targets

| Metric | v5.4.0 (now) | v6.0 | v8.0 | v10.0 |
|--------|-------------|------|------|-------|
| Tests | 1,747 | 1,900 | 2,200 | 2,500 |
| Branch coverage (critical) | Unknown | >80% | >90% | >95% |
| Lean 4 proofs | 8 | 8 | 16 | 20 |
| Semantic drivers | 17 | 18 | 22 | 25 |
| crates.io crates | 37 | 37 | 40 | 42 |
| CLI commands | 58 | 60 | 65 | 70 |
| Unsafe blocks | 33 | 25 | 20 | 15 |
| Unsafe blocks with SAFETY | 33 | 33 | 33 | 33 |
| Clippy warnings | 0 | 0 | 0 | 0 |
| CI pipeline time | ~15m | <10m | <12m | <12m |
| suture-merge downloads/mo | TBD | 1K | 5K | 20K |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Rust edition upgrade breaks compilation | Medium | High | Pin rust-toolchain.toml, test with nightly |
| SQLite WAL corruption on crash | Low | Critical | WAL checkpoint on shutdown, fsck on startup |
| BLAKE3 collision (theoretical) | Negligible | Critical | Monitor research, plan migration |
| Raft split-brain | Low | Critical | Persist election state, BTreeMap ordering |
| WASM sandbox escape | Low | Critical | Limit host imports, review wasmtime advisories |
| Supply chain attack via dependency | Medium | High | cargo audit in CI, lockfile pinning, entropy analysis |
| Driver regression | Medium | High | Property-based tests, E2E lifecycle tests |
| Performance regression | Medium | Medium | Criterion in CI with regression gating |
| Desktop app Tauri breaking changes | Medium | Medium | Pin Tauri version, test before upgrade |
| crates.io publish failure | Low | Low | Dry-run in CI, manual review before publish |

---

## Strategic Decisions

### What NOT to Do

| Decision | Rationale |
|----------|-----------|
| Do not migrate to PostgreSQL | SQLite + Raft covers single-node and distributed. Added complexity not justified. |
| Do not implement QUIC | TCP + Zstd achieves adequate latency. QUIC adds complexity without clear benefit. |
| Do not implement NFSv4/SMB3 | FUSE3 + WebDAV covers primary use cases. Kernel-level dev not justified. |
| Do not migrate to FlatBuffers | bincode + Zstd is performant and well-tested. |
| Do not optimize for nanosecond latency | VCS operations are I/O bound. Focus on large-file and large-repo scale. |

### What to Double Down On

| Decision | Rationale |
|----------|-----------|
| Semantic merge quality | Sole differentiator from Git. Every driver improvement increases value. |
| suture-merge library adoption | Growth vector. Low friction (cargo add), high impact. |
| Formal verification | Unique in VCS space. Builds trust for regulated industries. |
| Performance at scale | Enterprise adoption requires 100K+ files, 100K+ commits. |
| Editor integrations | Where users spend their time. Reduce context switching. |
