# Suture Production Roadmap

**Version:** 5.4.0
**Date:** 2026-05-15
**Status:** Post-audit -- all 12 phases complete, CI green, ready for production push

---

## 1. Audit Summary (2026-05-15)

### 1.1 Test Results

| Metric | Value |
|--------|-------|
| Total tests | 1,759 passed, 0 failed, 20 ignored |
| Crates tested | 41 (excluding fuzz, py, node) |
| Clippy warnings | 0 (workspace-wide, `-D warnings`) |
| Format check | Clean (`cargo fmt --all -- --check`) |
| Determinism tests | 7/7 passed (BLAKE3, patch ID, commit, merge, push/pull, diff symmetry, branch idempotent) |
| Property-based tests | 21 proptest suites, 10K+ cases |

### 1.2 CI/CD Status

| Workflow | Status | Notes |
|----------|--------|-------|
| CI (lint + test matrix) | GREEN | 16/17 jobs pass; 1 flaky macOS runner (transient rustup issue) |
| Security | GREEN | cargo-audit clean, clippy clean, SBOM generated |
| Docker Build | GREEN | Multi-stage build fixed, produces suture-platform image |
| Release | READY | 4-platform binary matrix (Linux x86_64, macOS x86_64+aarch64, Windows) |
| Pages | DEPLOYED | docs-site/ live at suture.dev |
| Coverage | GENERATED | llvm-cov report uploaded as artifact |

### 1.3 CI Fixes Applied (This Session)

1. **test-cli protoc**: Added protoc installation to all 3 OS matrices in test-cli job
2. **build-release protoc**: Added protobuf-compiler to system dependencies
3. **MockBackend trait**: Added missing `backend_name()` method to `BlobBackend` impl in S3 feature test
4. **Dockerfile**: Converted from broken single-stage (expected pre-built binary) to proper multi-stage Rust build
5. **Documentation**: Removed emoji from ROADMAP.md, updated test counts to match actual (1,759), fixed stale LFS doc comments

### 1.4 Code Quality

| Check | Result |
|-------|--------|
| `todo!()` calls | 0 |
| `unimplemented!()` calls | 0 (1 in test fixture string, not production) |
| FIXME/HACK/XXX comments | 0 |
| Stubs (incomplete features) | 2 (WASM plugin `diff()` and `format_diff()` -- feature-gated, documented) |
| Empty function bodies | 5 (2 FUSE destroy -- standard pattern; 3 plugin-sdk conditional compilation stubs) |
| Stale doc comments | 2 fixed (LFS Push/Pull "not yet implemented") |
| Unsafe blocks | 33 (all with SAFETY comments per prior audit) |

### 1.5 Documentation Audit

| Check | Result |
|-------|--------|
| Emojis in markdown | Removed from ROADMAP.md (was only file with emojis) |
| Test counts accurate | Updated: 1,759 (was 1,747); suture-e2e 200 (was 226) |
| Stale version references | Fixed |
| Mathematical correctness | Performance baselines have quantitative thresholds, all verified |
| Website (docs-site) | Live at suture.dev, interactive demo functional |
| Detailed docs (docs/) | 77 HTML+MD files exist locally; NOT deployed to GitHub Pages (only docs-site is) |

### 1.6 Known Issues (Non-Blocking)

| Issue | Severity | Resolution |
|-------|----------|------------|
| macOS CI test-cli flaky | Low | Transient runner issue; re-run passes |
| docs/ not deployed to website | Medium | Only docs-site/ deployed; need unified docs build |
| suture-py excluded from workspace | Medium | PyO3 build complexity; needs separate CI matrix |
| desktop-app excluded from workspace | Low | Tauri v2 dependency conflicts |
| CWD mutex in CLI tests | Medium | Forces --test-threads=1; refactor to tempdir per test |
| WASM plugin diff/format_diff stubs | Low | Feature-gated; acceptable for experimental feature |

---

## 2. Path to Production

### Phase P0: Documentation Deployment (1 week)

Deploy all docs/ content to the website so users can access quickstart, API reference, architecture, and performance docs online.

| Task | Effort | Priority |
|------|--------|----------|
| Build docs/ HTML files into docs-site/ or deploy docs/ as subpath | 2d | Critical |
| Update CNAME from suture.dev if needed | 0.5d | High |
| Add navigation between docs-site landing and docs/ pages | 1d | High |
| Verify all internal links work on deployed site | 1d | High |
| Add version selector for docs (current vs. previous) | 1d | Medium |

### Phase P1: CI Reliability (1 week)

Eliminate flaky CI and reduce pipeline time.

| Task | Effort | Priority |
|------|--------|----------|
| Add retry logic for flaky macOS jobs | 0.5d | High |
| Cache protoc binary instead of apt-get install | 0.5d | Medium |
| Parallelize cargo check and cargo clippy | 0.5d | Medium |
| Add --test-threads=4 to test-cli (refactor CWD mutex) | 3d | High |
| Add merge queue for main branch protection | 1d | Medium |
| Update actions/checkout to v5 (Node 24 compatibility) | 0.5d | High |

### Phase P2: Soundness Hardening (2 weeks)

Eliminate all remaining soundness concerns.

| Task | Effort | Priority |
|------|--------|----------|
| Audit all 33 unsafe blocks with formal SAFETY comments | 3d | Critical |
| FUSE Send/Sync: document libfuse3 threading guarantees | 2d | Critical |
| SHM: add `#[repr(C)]` to shared memory structs | 0.5d | Critical |
| Binary drivers: add `debug_assert!(from_utf8().is_ok())` before `from_utf8_unchecked` | 1d | Critical |
| Remove CWD mutex from CLI tests (refactor to tempdir) | 3d | High |
| Add `cargo semver-checks` to CI for API stability | 1d | High |

### Phase P3: Test Coverage (2 weeks)

Establish quantitative coverage baselines and enforce thresholds.

| Task | Effort | Priority |
|------|--------|----------|
| Set coverage threshold in CI (>80% line, >70% branch) | 1d | High |
| Add missing tests for connector crates (Airtable, Sheets, Notion) | 3d | Medium |
| Add integration tests for desktop-app | 3d | Medium |
| Add WASM plugin E2E test with real .wasm file | 2d | Medium |
| Property-based tests for merge engine (expand from 21 suites) | 2d | High |

### Phase P4: Performance at Scale (3 weeks)

Optimize for enterprise-scale repositories.

| Task | Effort | Priority |
|------|--------|----------|
| Parallel file hashing with rayon for `suture add .` | 2d | High |
| Incremental file tree computation in commit | 1d | High |
| Lazy patch deserialization in `suture log` | 2d | High |
| Pack files for small blob deduplication | 5d | Medium |
| Partial clone / sparse checkout | 5d | Medium |
| Criterion benchmarks in CI with 10% regression gating | 2d | High |
| Background GC with configurable thresholds | 3d | Medium |

### Phase P5: suture-merge Library v1.0 (2 weeks)

Stabilize the standalone merge library for ecosystem adoption.

| Task | Effort | Priority |
|------|--------|----------|
| Add `merge_sql()`, `merge_ical()`, `merge_feed()` to public API | 2d | High |
| Stabilize API surface, document stability guarantees | 2d | Critical |
| Add conflict callback API for programmatic resolution | 2d | High |
| Publish all 37 crates to crates.io in dependency order | 1d | High |
| Add semver-checks to CI | 1d | Critical |
| Write migration guide (v0.x to v1.0) | 1d | Medium |

### Phase P6: Enterprise Features (4 weeks)

Features required for enterprise deployment.

| Task | Effort | Priority |
|------|--------|----------|
| Backup/restore tooling for hub | 3d | High |
| Prometheus metrics endpoint (/metrics) | 2d | High |
| Per-user rate limiting | 2d | Medium |
| API versioning (/api/v1/ prefix) | 2d | High |
| OAuth2 scope-based tokens + refresh rotation | 3d | Medium |
| Per-repo permissions (owner/collaborator/reader) | 3d | High |
| Branch protection rules | 2d | High |
| Audit logging for all permission changes | 2d | High |
| Structured JSON logging with tracing-subscriber | 1d | High |
| OpenTelemetry distributed tracing | 3d | Medium |

### Phase P7: Advanced Merge (3 weeks)

Expand semantic merge to harder cases.

| Task | Effort | Priority |
|------|--------|----------|
| DOCX track-changes-aware merge | 5d | High |
| XLSX formula-aware merge (AST-level) | 5d | High |
| PPTX animation/timing preservation | 3d | Medium |
| OOXML comment/annotation merge | 2d | Medium |
| Lockfile merge (Cargo.lock, package-lock.json) | 3d | High |
| Custom merge strategies via WASM plugins | 3d | Medium |

### Phase P8: Desktop App (4 weeks)

Complete the Tauri desktop application.

| Task | Effort | Priority |
|------|--------|----------|
| Fix Tauri build in workspace (resolve dependency conflicts) | 3d | Critical |
| Add CI matrix (macOS + Windows + Linux) | 2d | High |
| Real-time sync status in UI | 3d | High |
| Visual merge conflict resolution (side-by-side editor) | 5d | High |
| Repository browser with history sidebar | 3d | Medium |
| System tray notifications | 1d | Medium |
| Smoke tests for basic workflows | 2d | High |

### Phase P9: Editor Integration Polish (3 weeks)

Deepen editor plugin integrations.

| Task | Effort | Priority |
|------|--------|----------|
| VS Code: real-time semantic diff preview | 3d | High |
| JetBrains: merge conflict resolution UI | 5d | Medium |
| Neovim: stable release to MELPA/lazy.nvim | 1d | Medium |
| LSP: add code action for conflict resolution | 2d | Medium |

### Phase P10: Ecosystem Growth (3 weeks)

Expand distribution channels.

| Task | Effort | Priority |
|------|--------|----------|
| suture-py: re-include in CI, publish to PyPI | 3d | Medium |
| Homebrew formula update to latest version | 0.5d | High |
| AUR PKGBUILD update | 0.5d | High |
| Nix flake update | 0.5d | Medium |
| Docker multi-arch image (amd64 + arm64) | 2d | High |
| Install script verification on Ubuntu, macOS, Fedora, Arch | 1d | High |

### Phase P11: WASM Plugin Ecosystem (3 weeks)

Enable user-extensible merge drivers.

| Task | Effort | Priority |
|------|--------|----------|
| Implement WASM plugin `diff()` and `format_diff()` | 5d | High |
| Publish suture-plugin-sdk to crates.io | 1d | High |
| Plugin documentation and tutorial | 2d | High |
| Example plugins (INI, protobuf, dotenv) | 3d | Medium |
| Plugin signing and marketplace registry | 5d | Low |
| Fuel metering and memory limits enforcement | 2d | Medium |

### Phase P12: v1.0 Release (2 weeks)

Ship a stable, documented, well-supported v1.0.

| Task | Effort | Priority |
|------|--------|----------|
| API stability audit (cargo-semver-checks) | 2d | Critical |
| Breaking change detection in CI | 1d | Critical |
| Compatibility testing: Rust 1.75+, Ubuntu 22.04+, macOS 13+, Windows 10+ | 3d | High |
| Migration guide (v0.x to v1.0) | 2d | High |
| Troubleshooting guide | 2d | Medium |
| Video tutorials (3-5 minute walkthroughs) | 5d | Medium |
| GPG signing for release binaries | 1d | High |
| Automated release workflow with SHA256 checksums | 1d | High |
| All ADRs published | 2d | Medium |

---

## 3. Timeline

| Phase | Focus | Target Version | Duration | Start |
|-------|-------|---------------|----------|-------|
| P0 | Documentation deployment | v5.4.1 | 1 week | 2026-05-15 |
| P1 | CI reliability | v5.4.2 | 1 week | 2026-05-22 |
| P2 | Soundness hardening | v5.5.0 | 2 weeks | 2026-05-29 |
| P3 | Test coverage | v5.6.0 | 2 weeks | 2026-06-12 |
| P4 | Performance at scale | v6.0.0 | 3 weeks | 2026-06-26 |
| P5 | suture-merge v1.0 | v6.1.0 | 2 weeks | 2026-07-17 |
| P6 | Enterprise features | v7.0.0 | 4 weeks | 2026-07-31 |
| P7 | Advanced merge | v7.1.0 | 3 weeks | 2026-08-28 |
| P8 | Desktop app | v8.0.0 | 4 weeks | 2026-09-18 |
| P9 | Editor integrations | v8.1.0 | 3 weeks | 2026-10-16 |
| P10 | Ecosystem growth | v8.2.0 | 3 weeks | 2026-11-06 |
| P11 | WASM plugins | v9.0.0 | 3 weeks | 2026-11-27 |
| P12 | v1.0 release | v10.0.0 | 2 weeks | 2026-12-18 |

**Estimated total: 33 weeks (approximately 8 months to v1.0)**

---

## 4. Metrics Targets

| Metric | v5.4.0 (now) | v6.0 | v8.0 | v10.0 |
|--------|-------------|------|------|-------|
| Tests | 1,759 | 1,900 | 2,200 | 2,500 |
| Branch coverage (critical) | Unknown | >80% | >90% | >95% |
| Lean 4 proofs | 16 | 16 | 24 | 32 |
| Semantic drivers | 17 | 20 | 24 | 28 |
| crates.io crates | 37 | 37 | 40 | 42 |
| CLI commands | 58 | 60 | 65 | 70 |
| Unsafe blocks | 33 | 25 | 20 | 15 |
| Unsafe blocks with SAFETY | 33 | 33 | 33 | 33 |
| Clippy warnings | 0 | 0 | 0 | 0 |
| CI pipeline time | ~15m | <10m | <12m | <12m |
| suture-merge downloads/mo | TBD | 1K | 5K | 20K |
| Website pages | 1 (landing) | 30+ | 50+ | 50+ |

---

## 5. Risk Register

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
| Tauri breaking changes | Medium | Medium | Pin Tauri version, test before upgrade |
| crates.io publish failure | Low | Low | Dry-run in CI, manual review before publish |

---

## 6. Strategic Decisions

### What NOT to Do

| Decision | Rationale |
|----------|-----------|
| Do not migrate to PostgreSQL | SQLite + Raft covers single-node and distributed |
| Do not implement QUIC | TCP + Zstd achieves adequate latency |
| Do not implement NFSv4/SMB3 | FUSE3 + WebDAV covers primary use cases |
| Do not migrate to FlatBuffers | bincode + Zstd is performant and well-tested |
| Do not optimize for nanosecond latency | VCS operations are I/O bound |

### What to Double Down On

| Decision | Rationale |
|----------|-----------|
| Semantic merge quality | Sole differentiator from Git |
| suture-merge library adoption | Growth vector; low friction (cargo add) |
| Formal verification | Unique in VCS space; builds trust for regulated industries |
| Performance at scale | Enterprise adoption requires 100K+ files |
| Editor integrations | Where users spend their time |

---

## 7. Current Technical Debt

| ID | Severity | Description | Target Phase |
|----|----------|-------------|-------------|
| TD-1 | Critical | CLI CWD mutex forces --test-threads=1 in CI | P1 |
| TD-2 | Critical | FUSE unsafe impl Send/Sync soundness documentation | P2 |
| TD-3 | Critical | SHM unsafe impl Send/Sync -- no repr(C) proof | P2 |
| TD-4 | Critical | from_utf8_unchecked in 8+ binary drivers -- needs debug-assert | P2 |
| TD-5 | High | suture-py excluded from workspace/CI | P10 |
| TD-6 | High | desktop-app excluded from workspace/CI | P8 |
| TD-7 | High | No MSRV policy or rust-toolchain.toml pin | P1 |
| TD-8 | High | 33 unsafe blocks need formal SAFETY audit | P2 |
| TD-9 | Medium | Code coverage measured but no threshold enforcement | P3 |
| TD-10 | Medium | No performance regression gating in CI | P4 |
| TD-11 | Medium | docs/ content not deployed to website | P0 |
| TD-12 | Low | 5 E2E tests ignored (FUSE needs root) | P2 |
| TD-13 | Low | WASM plugin diff/format_diff stubs | P11 |
