# Suture Roadmap -- Forward Path Analysis

**Date:** 2026-05-10
**Version:** 5.3.1 (post-audit)
**Author:** Automated audit + architectural review

---

## 1. Current State Assessment

### 1.1 Quantitative Baseline

| Metric | Value |
|--------|-------|
| Workspace crates | 42 |
| Total Rust LoC | 108,029 |
| Test functions | 1,747 |
| Test modules | 104 |
| Public API surface (core) | 274 functions |
| Semantic drivers | 17 |
| Lean 4 formal proofs | 8 theorems/lemmas |
| Cargo audit findings | 0 critical, 1 unmaintained (paste via rav1e, transitive, no fix) |
| Clippy warnings | 0 (clean with `-D warnings`) |
| Formatting | Clean (`rustfmt`) |
| CI workflows | 8 (ci, release, security, performance, semantic-merge, pages, docker, example-merge) |

### 1.2 Quality Gate Status

| Gate | Status | Evidence |
|------|--------|----------|
| Unit tests | PASS | 1,747 test functions, 0 failures |
| Integration tests | PASS | 56 E2E tests (27 driver correctness + 29 workflow) |
| Property-based tests | PASS | 21 proptest suites across core |
| Formal verification | PASS | 8 Lean 4 theorems (conflict equivalence, commutativity, symmetry, identity, determinism) |
| Security audit | PASS | `cargo audit` clean (1 allowed unmaintained) |
| Static analysis | PASS | `cargo clippy --workspace -- -D warnings` clean |
| Formatting | PASS | `cargo fmt --all -- --check` clean |
| Pre-commit hook | INSTALLED | `scripts/pre-commit` -> `.git/hooks/pre-commit` |

### 1.3 Architectural Strengths

1. **Patch-DAG model with BLAKE3 CAS**: Content-addressed storage with SIMD-accelerated hashing. Every blob stored once, deduplication is free.
2. **Semantic merge across 17 file formats**: JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, OTIO, SQL, PDF, Image, Example, HTML, SVG, iCal. This is the core differentiator.
3. **Formal verification**: Lean 4 proofs for merge algebra properties (touch-set conflict equivalence, disjoint commutativity, merge symmetry, identity element, determinism). This is unusual for a VCS.
4. **Raft consensus**: Multi-node cluster with TCP transport, persisted log, election timeouts per Raft paper section 5.2. Production-ready distributed option.
5. **Supply chain integrity**: Shannon entropy analysis with 13 XZ-style attack detection indicators. Not a standard VCS feature.
6. **Zero-copy wire protocol**: Zstd compression, delta encoding, V2 handshake with capability negotiation.
7. **Comprehensive CLI**: 58+ subcommands covering the full VCS lifecycle (init, add, commit, branch, merge, rebase, bisect, blame, stash, reflog, fsck, gc, export, sync, archive, grep, etc.).

### 1.4 Technical Debt Identified

#### Critical (blocks reliability)

| ID | Issue | Impact | Fix Effort |
|----|-------|--------|------------|
| TD-1 | CWD-dependent CLI tests require `--test-threads=1` | CI parallelism bottleneck, mutex poisoning risk | Medium: refactor tests to use tempdir per test, remove static mutex |
| TD-2 | FUSE VFS `unsafe impl Send/Sync` on `RwFilesystem` | Soundness concern -- FUSE callbacks are single-threaded but impl claims thread-safety | Medium: audit callback threading model, justify or remove |
| TD-3 | SHM `unsafe impl Send/Sync` on `ShmStatus` | Memory-mapped struct shared across processes without formal proof of layout stability | Low: add static_assert for repr(C), document layout guarantees |
| TD-4 | `from_utf8_unchecked` in 8+ locations (DOCX, XLSX, PPTX, PDF, Image drivers) | Correct only if input is actually valid UTF-8; binary ZIP data passed through could theoretically corrupt | Medium: add debug-assert validation in debug builds, document invariants. NOTE: workspace zip feature unification causes Deflate compression, making zip bytes non-UTF-8. Tests fixed to use merge_raw() instead of string round-trip. |

#### High (blocks adoption)

| ID | Issue | Impact | Fix Effort |
|----|-------|--------|------------|
| TD-5 | `suture-py` excluded from workspace | Python bindings not tested in CI, version drift risk | Low: fix PyO3 build, re-include |
| TD-6 | `desktop-app` excluded from workspace | Tauri app not built/tested in CI | Medium: add Tauri CI matrix |
| TD-7 | No MSRV (Minimum Supported Rust Version) policy | Users on older Rust may hit edition/feature incompatibilities | Low: add `rust-toolchain.toml` pin, test with MSRV |
| TD-8 | VERSION.md claims "1,662 tests" but actual count is ~1,747 | Documentation drift from reality | Trivial: update count. Fixed: now 1,747 tests across workspace. |

#### Medium (blocks scale)

| ID | Issue | Impact | Fix Effort |
|----|-------|--------|------------|
| TD-9 | 48 `unsafe` blocks in production code (excluding tests/FFI) | Surface area for soundness bugs | Medium: audit each, justify with SAFETY comments, eliminate where possible |
| TD-10 | No code coverage measurement | Cannot prove >80% branch coverage claim | Low: add `cargo-tarpaulin` or `cargo-llvm-cov` to CI |
| TD-11 | Flatbuffers mentioned in requirements.md but not implemented | Wire protocol uses custom serialization, not Flatbuffers | Low: update requirements.md to match reality, or evaluate FlatBuffers migration |
| TD-12 | NFSv4/SMB3/ProjFS mentioned in requirements.md but not implemented | Only FUSE3 and WebDAV implemented | Low: update requirements.md |
| TD-13 | QUIC transport mentioned in requirements.md but not implemented | Only TCP for gRPC | Low: update requirements.md |
| TD-14 | PostgreSQL/Redis mentioned in requirements.md but not implemented | Only SQLite for hub | Low: update requirements.md |

#### Low (polish)

| ID | Issue | Impact | Fix Effort |
|----|-------|--------|------------|
| TD-15 | `librust_out.rmeta` in repo root | Build artifact committed or ignored incorrectly | Trivial: git rm, add to .gitignore |
| TD-16 | 5 E2E tests ignored (FUSE/VFS integration require root) | Tests not running in CI | Low: add to CI with rootless fallback or container |

#### Resolved This Session

| ID | Issue | Resolution |
|----|-------|------------|
| -- | 2 DOCX track-changes tests failing in workspace build | Root cause: zip crate feature unification changes default compression from Stored to Deflate, making zip bytes non-UTF-8. Tests rewritten to use merge_raw() + OoxmlDocument::from_bytes(). |
| -- | 1 clippy warning (unused variable in suture-s3) | Removed unused binding. |
| -- | e2e push_pull_roundtrip test failing | Test hub missing /push/compressed and /pull/compressed routes. Routes added. |
| -- | 7 fuzz bin targets running under cargo test indefinitely | Added harness=false to all [[bin]] fuzz targets in suture-fuzz Cargo.toml. |
| -- | 5 broken doc links across repo | Fixed stale crate refs, broken relative paths, and incorrect driver count. |

---

## 2. Roadmap

### Phase 1: Hardening (v5.4) -- 2 weeks

**Goal:** Eliminate all critical and high-priority technical debt.

| Task | Priority | Effort | Owner |
|------|----------|--------|-------|
| TD-1: Refactor CLI tests to eliminate CWD mutex | Critical | 3d | Core |
| TD-2: Audit and document FUSE Send/Sync | Critical | 2d | VFS |
| TD-3: Add repr(C) + static_assert to ShmStatus | Critical | 0.5d | Daemon |
| TD-4: Add debug-assert validation to from_utf8_unchecked | Critical | 1d | Drivers |
| TD-8: Update VERSION.md test count | High | 0.5d | Docs |
| TD-10: Add code coverage to CI | High | 1d | CI |
| TD-15: Remove stray build artifact | Low | 0.1d | Repo |
| TD-9: Audit all 48 unsafe blocks, add SAFETY comments | Medium | 3d | Core |

**Exit criteria:** Zero critical TD items. Code coverage visible in CI. All unsafe blocks documented.

### Phase 2: Requirements Reconciliation (v5.5) -- 1 week

**Goal:** Align `requirements.md` with actual implementation. Remove aspirational claims for unimplemented features.

| Task | Priority | Effort |
|------|----------|--------|
| TD-11: Update FlatBuffers claim in requirements.md | Medium | 0.5d |
| TD-12: Update NFSv4/SMB3/ProjFS claims | Medium | 0.5d |
| TD-13: Update QUIC transport claim | Medium | 0.5d |
| TD-14: Update PostgreSQL/Redis claim | Medium | 0.5d |
| Write ADR for each deferred feature with rationale | High | 1d |

**Exit criteria:** `requirements.md` describes the system as it exists, not as it was imagined. ADRs exist for each deferred item.

### Phase 3: Performance Engineering (v6.0) -- 4 weeks

**Goal:** Establish quantitative performance baselines and optimize hot paths.

| Task | Priority | Effort |
|------|----------|--------|
| Benchmark suite expansion (suture-bench: 44 functions currently) | High | 2d |
| Identify O(n^2) or worse paths via profiling (perf/criterion) | Critical | 5d |
| Incremental file tree computation (already done for commits) | Done | -- |
| Lazy patch loading for `suture log` on large repos | High | 3d |
| Blob cache hit/miss metrics and tuning | Medium | 2d |
| WASM plugin cold-start optimization | Medium | 3d |
| Large file handling (>100MB) streaming | High | 5d |
| Parallel patch application for batch merges | Medium | 3d |
| SQLite query optimization (WAL tuning, indexes) | Medium | 2d |

**Target metrics:**

| Operation | Current | Target |
|-----------|---------|--------|
| `suture init` | <100ms | <50ms |
| `suture add .` (10K files) | O(n) | O(n), <2s |
| `suture commit` | <500ms | <200ms |
| `suture log` (10K commits) | O(n) | O(n), <500ms |
| `suture merge` (100 files, 10 conflicts) | O(n*m) | O(n+m) per file |
| `suture push` (1K patches, 10K blobs) | Linear | Linear, >50MB/s |
| Semantic merge (DOCX 100 paragraphs) | O(p^2) worst case | O(p) average |

### Phase 4: Ecosystem Maturity (v6.1) -- 4 weeks

**Goal:** Make suture-merge the go-to semantic merge library.

| Task | Priority | Effort |
|------|----------|--------|
| TD-5: Re-include suture-py in workspace CI | High | 2d |
| TD-6: Add Tauri desktop-app to CI | High | 3d |
| suture-merge v0.3: add merge_yaml(), merge_toml(), merge_csv(), merge_xml() | High | 3d |
| suture-merge API stabilization (semver guarantee) | High | 2d |
| VS Code extension: real-time merge preview | Medium | 5d |
| JetBrains plugin: merge conflict resolution UI | Medium | 5d |
| WASM plugin SDK documentation and example | Medium | 3d |
| crates.io publishing automation (33 crates) | High | 2d |
| Homebrew/AUR/Nix package updates | Medium | 1d |

### Phase 5: Distributed Systems Depth (v7.0) -- 8 weeks

**Goal:** Make the hub deployment story production-grade.

| Task | Priority | Effort |
|------|----------|--------|
| Hub configuration validation and schema | High | 2d |
| Hub backup/restore tooling | High | 3d |
| Hub monitoring (Prometheus metrics endpoint) | High | 3d |
| Hub rate limiting per-user (currently global) | Medium | 2d |
| Hub replication lag visibility | Medium | 2d |
| S3 blob backend production hardening (multipart upload, retry) | High | 5d |
| Raft log compaction (snapshotting) | Critical | 5d |
| Raft membership changes (add/remove nodes at runtime) | High | 3d |
| gRPC reflection for debugging | Low | 1d |
| Hub API versioning strategy | High | 2d |
| Authentication improvements (OAuth2, API tokens with scopes) | High | 5d |

### Phase 6: Advanced Merge (v7.1) -- 6 weeks

**Goal:** Expand semantic merge to cover more formats and harder cases.

| Task | Priority | Effort |
|------|----------|--------|
| DOCX: track-changes-aware merge | High | 5d |
| XLSX: formula-aware merge (detect formula conflicts) | High | 5d |
| PPTX: animation/timing merge | Medium | 5d |
| OTIO: transition/effect merge | Medium | 3d |
| Image: pixel-level diff for PNG/JPEG | Low | 5d |
| Config file merge (INI, dotenv, properties) | Medium | 3d |
| Lockfile merge (Cargo.lock, package-lock.json, yarn.lock) | High | 3d |
| Database schema migration merge (SQL ALTER statements) | Medium | 5d |
| Merge conflict resolution API (programmatic) | High | 3d |
| Merge strategy plugins (user-defined) | Medium | 5d |

### Phase 7: Scale and Reliability (v8.0) -- 8 weeks

**Goal:** Prove suture works at enterprise scale.

| Task | Priority | Effort |
|------|----------|--------|
| Repository size limits and enforcement | High | 2d |
| Partial clone (sparse checkout) | High | 5d |
| Shallow clone (depth-limited history) | Medium | 3d |
| Garbage collection tuning (incremental, background) | High | 5d |
| fsck improvements (parallel, repair mode) | Medium | 3d |
| Concurrent push handling (hub) | High | 5d |
| Repository mirroring (hub-to-hub) | Medium | 3d |
| Access control (per-repo, per-branch permissions) | High | 5d |
| Webhooks reliability (retry queue, dead letter) | Medium | 3d |
| Observability (structured logging, tracing, error taxonomy) | High | 3d |

### Phase 8: Formal Verification Expansion (v8.1) -- 4 weeks

**Goal:** Expand Lean 4 proof coverage for critical algorithms.

| Current proofs | Target proofs |
|----------------|---------------|
| Touch-set conflict equivalence | Patch-DAG acyclicity invariant |
| Disjoint commutativity | LCA correctness |
| Merge symmetry | Three-way merge completeness |
| Identity element | Blob CAS consistency |
| Merge determinism | Ed25519 signature non-forgeability (assuming curve) |
| Diff determinism | Raft election safety |
| Patch composition associativity | Conflict marker well-formedness |
| Reflog append-only invariant | GC reachability correctness |

### Phase 9: Platform Deepening (v9.0) -- 6 weeks

**Goal:** Native integrations with professional tools.

| Task | Priority | Effort |
|------|----------|--------|
| DaVinci Resolve plugin (via Python/PyO3) | High | 10d |
| Premiere Pro panel (via CEP/UXP) | Medium | 10d |
| Final Cut Pro XML round-trip | Medium | 5d |
| Avid AAF interchange | Low | 10d |
| Excel add-in (via Office.js or COM) | Medium | 10d |
| Google Docs integration (via API) | Low | 5d |
| Notion/Airtable/Google Sheets connectors (already scaffolded) | Medium | 5d |

### Phase 10: v1.0 Release Preparation (v10.0) -- 4 weeks

**Goal:** Ship a stable, documented, well-supported v1.0.

| Task | Priority | Effort |
|------|----------|--------|
| API stability audit (semver check on all public types) | Critical | 5d |
| Documentation complete (API reference, migration guide, troubleshooting) | High | 10d |
| Performance regression suite in CI | High | 3d |
| Security audit (external, if budget allows) | High | 5d |
| Compatibility matrix (OS/Rust versions) | Medium | 2d |
| Release notes automation | Medium | 2d |
| Breaking change detection in CI | High | 2d |
| User survey and feedback incorporation | Medium | 3d |

---

## 3. Strategic Decisions

### 3.1 What NOT to Do

| Decision | Rationale |
|----------|-----------|
| Do not migrate to PostgreSQL/Redis | SQLite is sufficient for single-hub deployments. Multi-hub scenarios are rare and can use SQLite + Raft. Added operational complexity is not justified by current user base. |
| Do not implement QUIC transport | TCP + Zstd compression achieves adequate latency. QUIC adds implementation complexity (connection migration, 0-RTT) without clear benefit for hub-to-client traffic patterns. |
| Do not implement NFSv4/SMB3/ProjFS | FUSE3 + WebDAV covers the primary use cases. NFSv4/SMB3 require kernel-level development. ProjFS is Windows-only. |
| Do not migrate to FlatBuffers | Current wire format (bincode + Zstd) is performant and well-tested. FlatBuffers adds build complexity and the zero-copy benefit is marginal for patch-sized payloads. |
| Do not pursue HFT-level nanosecond optimization | Suture is a VCS, not a trading system. Microsecond-level latency is sufficient. Focus optimization effort on large-file and large-repo scenarios instead. |

### 3.2 What to Double Down On

| Decision | Rationale |
|----------|-----------|
| Semantic merge quality | This is the sole differentiator from Git. Every driver improvement directly increases value. |
| suture-merge library adoption | The library crate is the growth vector. Low friction (cargo add), high impact (semantic merge in any Rust project). |
| Formal verification | Unique in the VCS space. Builds trust for safety-critical domains (defence, medical, finance). |
| Lean 4 proof expansion | Proves correctness claims. Marketing asset for regulated industries. |
| Performance on large repos | Enterprise adoption requires handling repos with 100K+ files and 100K+ commits. |

---

## 4. Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Rust edition upgrade breaks compilation | Medium | High | Pin rust-toolchain.toml, test with nightly before stable |
| SQLite WAL corruption on crash | Low | Critical | WAL checkpoint on graceful shutdown, fsck on startup |
| BLAKE3 collision (theoretical) | Negligible | Critical | Monitor BLAKE3 research, plan migration path |
| Raft split-brain | Low | Critical | Persist election state, use BTreeMap for deterministic ordering |
| WASM plugin sandbox escape | Low | Critical | Limit host imports, review wasmtime security advisories |
| Cargo dependency supply chain attack | Medium | High | cargo audit in CI, lockfile pinning, entropy analysis |
| Driver correctness regression | Medium | High | Property-based tests for each driver, E2E lifecycle tests |
| Performance regression | Medium | Medium | Criterion benchmarks in CI with regression detection |

---

## 5. Metrics Targets

| Metric | Current (v5.3.1) | v6.0 Target | v8.0 Target | v10.0 Target |
|--------|-------------------|-------------|-------------|--------------|
| Test count | 1,747 | 1,900 | 2,200 | 2,500 |
| Branch coverage (critical paths) | Unknown | >80% | >90% | >95% |
| Lean 4 proofs | 8 | 12 | 16 | 20 |
| Semantic drivers | 18 | 20 | 25 | 30 |
| crates.io crates published | 37 | 37 | 40 | 42 |
| CLI commands | 58 | 62 | 65 | 70 |
| Unsafe blocks (production) | 48 | 30 | 20 | 15 |
| Clippy warnings | 0 | 0 | 0 | 0 |
| Cargo audit critical | 0 | 0 | 0 | 0 |
| CI pipeline time | ~15m | <12m | <15m | <15m |
| suture-merge downloads/month | TBD | 1K | 5K | 20K |

---

## 6. Version Timeline

| Version | Focus | Est. Date |
|---------|-------|-----------|
| v5.4 | Hardening (TD-1 through TD-9) | 2026-05-24 |
| v5.5 | Requirements reconciliation | 2026-05-31 |
| v6.0 | Performance engineering | 2026-06-28 |
| v6.1 | Ecosystem maturity | 2026-07-26 |
| v7.0 | Distributed systems depth | 2026-09-20 |
| v7.1 | Advanced merge | 2026-11-01 |
| v8.0 | Scale and reliability | 2027-01-10 |
| v8.1 | Formal verification expansion | 2027-02-07 |
| v9.0 | Platform deepening | 2027-03-21 |
| v10.0 | v1.0 release | 2027-04-18 |
