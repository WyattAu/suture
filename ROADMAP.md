# Suture Production Roadmap

**Version:** 5.4.0
**Date:** 2026-05-15
**Author:** Comprehensive monorepo audit (tests, CI/CD, docs, code quality)
**Status:** All prior phases complete. Post-audit remediation in progress.

---

## 0. Current State (Post-Audit 2026-05-15)

### 0.1 Quantitative Baseline

| Metric | Value |
|--------|-------|
| Workspace crates | 44 |
| Rust LoC | ~108,000 |
| Test functions | 1,759 |
| Test failures | 0 |
| Clippy warnings | 0 |
| Rustdoc warnings | 0 |
| Semantic drivers | 18 (JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, OTIO, SQL, PDF, Image, SVG, HTML, Feed, iCal, Properties) |
| CLI subcommands | 58+ |
| Lean 4 proofs | 16 theorems |
| Unsafe blocks (production) | 33 (all with SAFETY comments) |
| CI workflows | 8 (CI, Docker, Pages, Release, Security, Performance, Semantic Merge, Example Merge) |
| crates.io crates | 37 |
| Editor plugins | 3 (Neovim, JetBrains, VS Code) |
| Language bindings | 2 (Node.js, Python) |
| Pre-commit hooks | fmt + clippy + check + test |
| Pre-push hooks | fmt + clippy + test |

### 0.2 Quality Gate Results (2026-05-15)

| Gate | Result | Details |
|------|--------|---------|
| cargo fmt --check | PASS | Zero formatting issues |
| cargo clippy --workspace -D warnings | PASS | Zero warnings |
| cargo doc --workspace --no-deps | PASS | Zero warnings (5 fixed in audit) |
| cargo test --workspace | PASS | 1,759 passed, 0 failed, ~22 ignored |
| E2E tests | PASS | 200 tests across all workflows |
| Pre-commit hook | PASS | fmt + clippy + check + test |
| Pre-push hook | PASS | fmt + clippy + test |
| CI (main) | PASS | All matrix jobs green (Linux/macOS/Windows) |
| CI (coverage) | PASS | >50% threshold enforced |
| CI (security) | PASS | cargo audit clean |
| CI (Docker) | FIXING | Image builds; smoke test retry logic added |
| CI (Pages) | PASS | docs/ deployed to GitHub Pages |
| CI (feature-matrix) | PASS | raft-cluster, s3-backend, raft/persist all green |

### 0.3 Maturity Assessment

| Layer | Status | Evidence |
|-------|--------|----------|
| Core VCS engine | Production-ready | 355 tests, 21 proptest, formal proofs |
| Semantic merge | Production-ready | 18 drivers, 71 binary E2E tests |
| CLI | Production-ready | 58+ commands, shell completions, 62 man pages |
| Hub (HTTP + gRPC) | Production-ready | 92 tests, auth, webhooks, S3, Raft |
| Wire protocol | Production-ready | 55 tests, V2 handshake, Zstd, delta encoding |
| VFS (FUSE3 + WebDAV) | Production-ready | 33 tests, read/write FUSE |
| TUI | Production-ready | 37 tests, hunk-level conflict resolver |
| Raft consensus | Production-ready | 52 tests, multi-node TCP cluster |
| S3 storage | Production-ready | 27 tests, SigV4, MinIO compatible |
| Desktop App | Scaffold | Tauri v2, excluded from workspace/CI |
| SaaS Platform | Functional | Stripe billing, OAuth, orgs, merge API |
| Connectors | Scaffold | Airtable, Google Sheets, Notion |
| WASM plugins | Experimental | ABI defined, not in CI |
| Python bindings | Excluded | PyO3, not in workspace/CI |
| npm package | Published | suture-merge-driver on npm |

### 0.4 Technical Debt Register (Updated)

| ID | Severity | Description | Status | Effort |
|----|----------|-------------|--------|--------|
| TD-1 | Critical | CLI CWD mutex forces --test-threads=1 in all CI | Open | 3d |
| TD-2 | Critical | FUSE unsafe impl Send/Sync soundness audit | Open | 2d |
| TD-3 | Medium | SHM unsafe impl Send/Sync -- no repr(C) proof | Open | 0.5d |
| TD-4 | Low | from_utf8_unchecked in binary drivers -- debug-assert added | Mitigated | -- |
| TD-5 | High | suture-py excluded from workspace/CI | Open | 2d |
| TD-6 | High | desktop-app excluded from workspace/CI | Open | 3d |
| TD-7 | Done | rust-toolchain.toml pins 1.94.1 | Resolved | -- |
| TD-8 | Done | 33 unsafe blocks all have SAFETY comments | Resolved | -- |
| TD-9 | Done | Coverage threshold 50% enforced in CI | Resolved | -- |
| TD-10 | Low | No performance regression gating in CI | Open | 2d |
| TD-11 | Done | Stale version references updated to 5.4.0 | Resolved | -- |
| TD-12 | Low | 2 VFS integration tests ignored (need root) | Open | 2d |
| TD-13 | Low | Desktop app not in workspace | Open | 1d |
| TD-14 | Done | Doc warnings fixed (5 rustdoc issues) | Resolved | -- |
| TD-15 | Done | CI checkout versions unified to @v5 | Resolved | -- |
| TD-16 | Done | CI coverage typo (suspense-lsp) fixed | Resolved | -- |
| TD-17 | Done | License refs corrected (Apache 2.0 -> AGPL-3.0) | Resolved | -- |
| TD-18 | Done | Emoji removed from docs/index.html | Resolved | -- |
| TD-19 | Done | Fake format claims removed (Protobuf/AVRO/CBOR etc.) | Resolved | -- |
| TD-20 | Low | BLOG_ANNOUNCEMENT.md describes v1.0.0-rc.1 | Open | 0.5d |
| TD-21 | Low | docs/release-notes.md describes v2.9.0 | Open | 0.5d |
| TD-22 | Low | CHANGELOG missing entries for v5.2-v5.4 | Open | 1d |

---

## Phase 1: Hardening (v5.5) -- 2 weeks

**Goal:** Eliminate critical technical debt. Make CI fully green.

| Task | Details | Priority |
|------|---------|----------|
| TD-1: CWD mutex removal | Refactor CLI integration tests to use tempdir per test. Target --test-threads=4 in CI. | Critical |
| TD-2: FUSE soundness audit | Document libfuse3 single-threaded guarantee. Remove or justify unsafe impl Send/Sync. | Critical |
| TD-5: suture-py CI | Fix PyO3 build, add to test matrix, gate on feature flag. | High |
| TD-6: Desktop app CI | Resolve Tauri dependency conflicts, add macOS/Windows build matrix. | High |
| CHANGELOG update | Add entries for v5.2.0 through v5.4.0. | Medium |
| Stale docs archive | Move BLOG_ANNOUNCEMENT.md and docs/release-notes.md to docs/archive/. | Low |

**Exit criteria:** Zero critical TD items. CI fully green on all platforms.

---

## Phase 2: Distribution (v6.0) -- 3 weeks

**Goal:** Make suture trivially installable and discoverable.

| Task | Details | Priority |
|------|---------|----------|
| Homebrew formula update | v6.0 with test block, auto-update. | High |
| AUR PKGBUILD update | Arch Linux package. | High |
| Nix flake update | Pin to v6.0. | Medium |
| Docker multi-arch | linux/amd64 + linux/arm64. | High |
| Install script verification | Test on Ubuntu, macOS, Fedora, Arch. | Medium |
| suture-merge v1.0 | API stabilization, semver commitment. | High |
| crates.io publish verification | Automated dry-run in CI. | Medium |
| README refresh | Accurate feature list, no aspirational claims. | Medium |

**Exit criteria:** `cargo install suture-cli`, `brew install`, `pip install` all work.

---

## Phase 3: Enterprise Readiness (v7.0) -- 6 weeks

**Goal:** Hub deployment is production-grade for team use.

| Task | Details | Priority |
|------|---------|----------|
| Backup/restore | `suture hub backup` / `suture hub restore`. | High |
| Prometheus metrics | `/metrics` endpoint with request latency, connections, repos. | High |
| Per-user rate limiting | Scope from global to per-authenticated-user. | Medium |
| API versioning | `/api/v1/` prefix with deprecation headers. | Medium |
| Deep health check | DB, S3, Raft liveness in `/healthz`. | High |
| Structured JSON logging | Replace eprintln with tracing-subscriber. | Medium |
| S3 multipart upload | For blobs > 100MB. | Medium |
| Raft log compaction at scale | Test at 1M entries. | Medium |
| Replication lag visibility | Commit index vs applied index. | Low |

**Exit criteria:** Hub runs 30 days without manual intervention in staging.

---

## Phase 4: Advanced Merge (v7.1) -- 4 weeks

**Goal:** Expand semantic merge to harder real-world cases.

| Task | Details | Priority |
|------|---------|----------|
| DOCX track-changes merge | Detect and preserve Word track changes. | High |
| XLSX formula-aware merge | Detect formula conflicts, not just value. | High |
| PPTX animation/timing merge | Preserve slide animations across merges. | High |
| OOXML comment merge | Preserve reviewer comments. | Medium |
| Lockfile merge strategy | Cargo.lock, package-lock.json semantic merge. | High |
| Merge conflict callback | Programmatic resolution for API users. | Medium |
| Custom merge strategies | User-defined per file type. | Medium |

**Exit criteria:** 3 real-world document collaboration scenarios validated.

---

## Phase 5: Scale and Reliability (v8.0) -- 6 weeks

**Goal:** Prove suture works at enterprise scale.

| Task | Details | Priority |
|------|---------|----------|
| Partial clone | Sparse checkout for large repos. | High |
| Shallow clone | Depth-limited history fetch. | Medium |
| Pack files | Combine small blobs into packed files. | Medium |
| Background GC | Incremental GC running concurrently. | Medium |
| Per-repo permissions | Owner, collaborator, reader roles. | High |
| Branch protection | Protected branches, required reviews. | High |
| Concurrent push handling | Hub handles multiple simultaneous pushes. | High |
| Performance regression gating | Criterion in CI with 10% threshold. | High |

**Exit criteria:** 100K files, 10K commits, all operations under 30s.

---

## Phase 6: Formal Verification Expansion (v8.1) -- 4 weeks

**Goal:** Expand Lean 4 proof coverage for critical algorithms.

| Property | Status | Effort |
|----------|--------|--------|
| Touch-set conflict equivalence | Proven | -- |
| Disjoint commutativity | Proven | -- |
| Merge symmetry | Proven | -- |
| Identity element | Proven | -- |
| Merge determinism | Proven | -- |
| Diff determinism | Proven | -- |
| Patch composition associativity | Proven | -- |
| Reflog append-only | Proven | -- |
| Patch-DAG acyclicity | Proven | -- |
| LCA correctness | Proven | -- |
| Three-way merge completeness | Proven | -- |
| CAS injectivity | Proven | -- |
| GC reachability | Proven | -- |
| Touch set monotonicity | Proven | -- |
| Raft election safety | Target | 5d |
| Ed25519 non-forgeability | Target | 5d |
| Conflict marker well-formedness | Target | 2d |

**Exit criteria:** 18+ proven theorems. Raft safety proven.

---

## Phase 7: Platform Deepening (v9.0) -- 6 weeks

**Goal:** Native integrations with professional tools.

| Task | Details | Priority |
|------|---------|----------|
| VS Code real-time merge preview | Show semantic diff inline. | High |
| JetBrains merge conflict UI | Integrate with IDEA merge tool. | High |
| Neovim stable release | Tag and publish to MELPA/lazy.nvim. | Medium |
| Airtable/Sheets/Notion connectors | Wire CLI commands. | Medium |
| Terraform state merge | Semantic merge for .tfstate JSON. | Medium |
| K8s manifest merge | YAML-aware merge for K8s resources. | Medium |

**Exit criteria:** At least 2 editor integrations ship in stable release.

---

## Phase 8: Desktop App (v9.1) -- 4 weeks

**Goal:** Native desktop app for non-developer users.

| Task | Details | Priority |
|------|---------|----------|
| Re-include in workspace | Fix Tauri build, add CI matrix. | High |
| Real-time sync status | Push/pull progress indicator. | High |
| Visual merge conflict resolution | Side-by-side editor. | High |
| Repository browser | File tree with history sidebar. | Medium |
| System tray notifications | Background sync alerts. | Medium |

**Exit criteria:** Desktop app has CI, smoke tests, and installable artifacts.

---

## Phase 9: v1.0 Release (v10.0) -- 4 weeks

**Goal:** Ship a stable, documented, well-supported v1.0.

| Task | Details | Priority |
|------|---------|----------|
| API stability audit | cargo-semver-checks in CI. | Critical |
| Compatibility matrix | Rust 1.85+, Ubuntu 22.04+, macOS 13+, Windows 10+. | High |
| Migration guide | v0.x to v1.0 upgrade path. | Medium |
| Troubleshooting guide | Common issues and solutions. | Medium |
| Video tutorials | 3-5 minute walkthroughs for core workflows. | Medium |
| Automated release | Build, sign, upload, publish on tag push. | High |
| GPG signing | Detached signatures for release binaries. | Medium |

**Exit criteria:** v1.0.0 tag pushed. Binaries on GitHub Releases. crates.io published.

---

## Version Timeline

| Version | Focus | Est. Duration |
|---------|-------|---------------|
| v5.5 | Hardening | 2 weeks |
| v6.0 | Distribution | 3 weeks |
| v7.0 | Enterprise readiness | 6 weeks |
| v7.1 | Advanced merge | 4 weeks |
| v8.0 | Scale and reliability | 6 weeks |
| v8.1 | Formal verification | 4 weeks |
| v9.0 | Platform deepening | 6 weeks |
| v9.1 | Desktop app | 4 weeks |
| v10.0 | v1.0 release | 4 weeks |
| **Total** | | **~39 weeks** |

---

## Metrics Targets

| Metric | v5.4 (now) | v7.0 | v9.0 | v10.0 |
|--------|------------|------|------|-------|
| Tests | 1,759 | 2,000 | 2,300 | 2,500 |
| Branch coverage (critical) | ~60% | >80% | >90% | >95% |
| Lean 4 proofs | 16 | 16 | 18 | 20 |
| Semantic drivers | 18 | 20 | 22 | 24 |
| crates.io crates | 37 | 37 | 40 | 42 |
| CLI commands | 58 | 60 | 65 | 70 |
| Unsafe blocks | 33 | 25 | 20 | 15 |
| Clippy warnings | 0 | 0 | 0 | 0 |
| CI pipeline time | ~15m | <12m | <12m | <10m |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Rust edition upgrade breaks compilation | Medium | High | Pin rust-toolchain.toml, test before upgrade |
| SQLite WAL corruption on crash | Low | Critical | WAL checkpoint on shutdown, fsck on startup |
| Raft split-brain | Low | Critical | Persisted election state, BTreeMap ordering |
| WASM sandbox escape | Low | Critical | Limit host imports, review wasmtime advisories |
| Supply chain attack via dependency | Medium | High | cargo audit in CI, lockfile pinning, entropy analysis |
| Driver regression | Medium | High | Property-based tests, E2E lifecycle tests |
| Performance regression | Medium | Medium | Criterion in CI with regression gating |
| Tauri breaking changes | Medium | Medium | Pin Tauri version, test before upgrade |

---

## Strategic Decisions

### What NOT to Do

| Decision | Rationale |
|----------|-----------|
| Migrate to PostgreSQL | SQLite + Raft covers single-node and distributed. Added complexity not justified. |
| Implement QUIC | TCP + Zstd achieves adequate latency. QUIC adds complexity without clear benefit. |
| Implement NFSv4/SMB3 | FUSE3 + WebDAV covers primary use cases. Kernel-level dev not justified. |
| Migrate to FlatBuffers | bincode + Zstd is performant and well-tested. |
| Optimize for nanosecond latency | VCS operations are I/O bound. Focus on large-file and large-repo scale. |

### What to Double Down On

| Decision | Rationale |
|----------|-----------|
| Semantic merge quality | Sole differentiator from Git. Every driver improvement increases value. |
| suture-merge library adoption | Growth vector. Low friction (cargo add), high impact. |
| Formal verification | Unique in VCS space. Builds trust for regulated industries. |
| Performance at scale | Enterprise adoption requires 100K+ files, 100K+ commits. |
| Editor integrations | Where users spend their time. Reduce context switching. |

---

## Post-Audit Change Log

### 2026-05-15: Comprehensive Audit

**Tests:** 1,759 pass, 0 fail across 44 crates.
**CI/CD:** Fixed 3 issues (coverage bash syntax, Docker case sensitivity, checkout version consistency).
**Documentation:** Fixed 7 issues (5 rustdoc warnings, license refs, emoji removal, fake format claims, stale versions).
**Code Quality:** Zero clippy warnings, zero fmt issues, zero doc warnings.
**Pre-commit:** Verified functional (fmt + clippy + check + test).
**Pre-push:** Verified functional (fmt + clippy + test).
**Website:** Landing page and docs deployed via GitHub Pages. No new sites needed.
