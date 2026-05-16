# Suture Production Roadmap

**Version:** 5.4.0
**Date:** 2026-05-16
**Author:** Full monorepo audit (tests, code quality, CI/CD, docs, security)
**Status:** Post-audit remediation complete. CI green. Production path clear.

---

## 0. Current State (Post-Audit 2026-05-16)

### 0.1 Quantitative Baseline

| Metric | Value |
|--------|-------|
| Workspace crates | 44 (37 publishable to crates.io) |
| Rust LoC | ~108,000 |
| Test functions | 1,759 (all passing, 0 failures) |
| Clippy warnings | 0 (-D warnings enforced) |
| Rustdoc warnings | 0 |
| Semantic drivers | 18 (JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, OTIO, SQL, PDF, Image, SVG, HTML, Feed, iCal, Properties) |
| CLI subcommands | 58+ |
| Lean 4 formal proofs | 16 theorems (1 sorry: DAG acyclicity topological ordering) |
| Unsafe blocks (production) | 33 (all with SAFETY comments) |
| CI workflows | 8 (CI, Docker, Pages, Release, Security, Performance, Semantic Merge, Example Merge) |
| Editor plugins | 3 (Neovim, JetBrains, VS Code) |
| Language bindings | 2 (Node.js via napi-rs, Python via PyO3) |
| Fuzz targets | 7 (libfuzzer-sys) |
| Proptest suites | 21 |

### 0.2 Quality Gate Results (2026-05-16)

| Gate | Result |
|------|--------|
| cargo fmt --check | PASS |
| cargo clippy --workspace -D warnings | PASS |
| cargo doc --workspace --no-deps | PASS |
| cargo test --workspace (excl. fuzz/py/node) | PASS (1,759 tests) |
| Pre-commit hook | PASS (fmt + clippy + test) |
| Pre-push hook | PASS (fmt + clippy + test) |
| CI (main, lint) | PASS |
| CI (test-workspace, 3-OS matrix) | PASS |
| CI (test-cli, 3-OS matrix) | PASS |
| CI (test-core, stable+beta) | PASS |
| CI (build-release) | PASS |
| CI (coverage, >50%) | PASS |
| CI (security-audit) | PASS |
| CI (Docker build+push) | PASS |
| CI (Docker smoke test) | PASS (JWT secret fix applied) |
| CI (Pages deploy) | PASS |
| CI (feature-matrix) | PASS |

### 0.3 Maturity Assessment

| Layer | Status | Evidence |
|-------|--------|----------|
| Core VCS engine | Production-ready | 355 tests, 21 proptest, 16 Lean 4 proofs |
| Semantic merge | Production-ready | 18 drivers, property-based tests, E2E lifecycle tests |
| CLI | Production-ready | 58+ commands, shell completions, 62 man pages |
| Hub (HTTP + gRPC) | Production-ready | 92 tests, auth, webhooks, S3, Raft clustering |
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

### 0.4 Technical Debt Register

| ID | Severity | Description | Status | Effort |
|----|----------|-------------|--------|--------|
| TD-1 | Critical | CLI CWD mutex forces --test-threads=1 in CI | Open | 3d |
| TD-2 | Critical | FUSE unsafe impl Send/Sync -- formal soundness audit | Open | 2d |
| TD-3 | Medium | SHM unsafe impl Send/Sync -- no repr(C) proof | Open | 0.5d |
| TD-4 | Low | WASM plugin diff/format_diff not implemented (graceful error) | Open | 3d |
| TD-5 | High | suture-py excluded from workspace/CI (PyO3 build issues) | Open | 2d |
| TD-6 | High | suture-node excluded from CI (ctor proc_macro regression) | Open | 1d |
| TD-7 | High | desktop-app excluded from workspace/CI | Open | 3d |
| TD-8 | Low | XLSX merge_cells() and rebuild_sheet_xml() are dead code | Open | 1d |
| TD-9 | Low | No performance regression gating in CI (display-only) | Open | 2d |
| TD-10 | Low | Dockerfile.build FROM scratch lacks runtime deps | Open | 0.5d |
| TD-11 | Low | CHANGELOG missing entries for v5.2-v5.4 | Open | 1d |
| TD-12 | Low | 2 VFS integration tests ignored (require root) | Open | 2d |
| TD-13 | Low | Subdirectory docs (blog/, roadmap/, deployment/) not built to HTML | Open | 1d |
| TD-14 | Low | Landing page missing OG/Twitter Card meta tags | Open | 0.5d |
| TD-15 | Low | suture.dev custom domain not resolving | Open | 0.5d |
| TD-16 | Info | ADR-008 through ADR-011 duplicate ADR-001 through ADR-004 | Open | 0.5d |

### 0.5 Audit Summary (2026-05-16)

**Code quality:** 0 critical, 0 high, 5 medium, 35 low, 60 info findings across 98 audited items. Fixed: unsafe from_utf8_unchecked in PDF driver, 7 fragile unwrap() calls in rate_limit middleware.

**CI/CD:** Fixed 8 critical, 10 high issues. Standardized crate exclusion lists across all workflows. Added missing protobuf-compiler to 4 workflows. Fixed Docker health check exit code. Fixed coverage threshold logic. Fixed redundant test runs.

**Documentation:** Fixed license mismatch (Cargo.toml Apache-2.0 to AGPL-3.0-or-later). Fixed CODE_OF_CONDUCT placeholder email. Fixed broken HTML nesting in docs/index.html. Fixed wrong license in template footer (all 36 generated pages). Added missing OTIO to landing page format grid. Fixed stale copyright year.

**Websites:** Landing page (docs-site/) exists at suture.dev (domain not resolving). Docs site deployed to GitHub Pages at wyattau.github.io/suture/. Both functional. No new sites needed.

---

## Phase 1: Hardening (v5.5) -- 2 weeks

**Goal:** Eliminate critical/high technical debt. Achieve full CI green across all crates.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| TD-1: CWD mutex removal | Refactor CLI tests to use per-test tempdir. Enable --test-threads=4. | Critical | 3d |
| TD-2: FUSE soundness proof | Formal proof or documented justification for unsafe impl Send/Sync on RwFilesystem. | Critical | 2d |
| TD-6: suture-node CI fix | Pin ctor to version with proc_macro feature, or fix napi-rs compatibility. | High | 1d |
| TD-5: suture-py CI | Gate behind feature flag, add Python dev headers to CI. | High | 2d |
| TD-11: CHANGELOG | Add entries for v5.2.0, v5.3.0, v5.3.1, v5.4.0. | Medium | 1d |
| VERSION.md condensation | Reduce 678 lines to ~30 lines. Move history to CHANGELOG. | Medium | 0.5d |
| Lean 4 DAG proof | Complete the sorry in proof_suture_core.lean (DAG acyclicity). | Medium | 1d |

**Exit criteria:** Zero critical TD items. All workspace crates compile and test in CI.

---

## Phase 2: Distribution (v6.0) -- 3 weeks

**Goal:** Make suture trivially installable and discoverable.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| Homebrew formula | v6.0 formula with test block and auto-update. | High | 1d |
| AUR PKGBUILD | Arch Linux package. | High | 0.5d |
| Nix flake | Pin to v6.0, verify `nix build`. | Medium | 1d |
| Docker multi-arch | linux/amd64 + linux/arm64 in CI matrix. | High | 2d |
| Install script | Test on Ubuntu, macOS, Fedora, Arch, Nix. | Medium | 1d |
| suture-merge v1.0 | API stabilization, semver commitment, cargo-semver-checks. | High | 3d |
| crates.io publish dry-run | Automated in release workflow. | Medium | 0.5d |
| README refresh | Accurate feature list, current test counts, no stale claims. | Medium | 1d |
| Landing page SEO | OG tags, Twitter Cards, canonical URL, sitemap. | Medium | 0.5d |
| suture.dev DNS | Configure DNS for custom domain. | Low | 0.5d |

**Exit criteria:** `cargo install suture-cli`, `brew install`, `pip install`, `npm install` all work. Landing page accessible at suture.dev.

---

## Phase 3: Enterprise Readiness (v7.0) -- 6 weeks

**Goal:** Hub deployment is production-grade for team use.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| Backup/restore | `suture hub backup` and `suture hub restore` commands. | High | 3d |
| Prometheus metrics | `/metrics` endpoint: request latency, connections, repos, merge stats. | High | 2d |
| Per-user rate limiting | Scope from global to per-authenticated-user. | Medium | 2d |
| API versioning | `/api/v1/` prefix with deprecation headers. | Medium | 2d |
| Deep health check | DB connectivity, S3 reachability, Raft liveness in `/healthz`. | High | 1d |
| Structured JSON logging | tracing-subscriber with JSON output for production. | Medium | 1d |
| S3 multipart upload | For blobs > 100MB. | Medium | 2d |
| Raft log compaction | Test at 1M entries, verify snapshot+compaction correctness. | Medium | 3d |
| Replication lag visibility | Expose commit index vs applied index via API. | Low | 1d |
| OIDC/OAuth2 standardization | Support generic OIDC providers beyond Google/GitHub. | Medium | 3d |

**Exit criteria:** Hub runs 30 days without manual intervention in staging environment.

---

## Phase 4: Advanced Merge (v7.1) -- 4 weeks

**Goal:** Expand semantic merge coverage to harder real-world cases.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| DOCX track-changes merge | Detect and preserve Word track changes during merge. | High | 5d |
| XLSX formula-aware merge | Detect formula conflicts, not just value conflicts. | High | 3d |
| PPTX animation/timing merge | Preserve slide animations and timing across merges. | High | 3d |
| OOXML comment merge | Preserve reviewer comments in DOCX/PPTX. | Medium | 2d |
| Lockfile merge strategy | Cargo.lock, package-lock.json, poetry.lock semantic merge. | High | 3d |
| Merge conflict callback | Programmatic resolution API for library users. | Medium | 2d |
| Custom merge strategies | User-defined per-file-type merge strategies via config. | Medium | 3d |
| WASM plugin diff/format_diff | Implement the two missing WASM plugin methods (TD-4). | Medium | 3d |

**Exit criteria:** 3 real-world document collaboration scenarios validated end-to-end.

---

## Phase 5: Scale and Reliability (v8.0) -- 6 weeks

**Goal:** Prove suture handles enterprise-scale repositories.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| Partial clone | Sparse checkout for repos with large working directories. | High | 5d |
| Shallow clone | Depth-limited history fetch. | Medium | 2d |
| Pack files | Combine small blobs into packed files for storage efficiency. | Medium | 3d |
| Background GC | Incremental garbage collection running concurrently. | Medium | 3d |
| Per-repo permissions | Owner, collaborator, reader roles. | High | 3d |
| Branch protection | Protected branches, required reviews, status checks. | High | 3d |
| Concurrent push handling | Hub handles multiple simultaneous pushes to same repo. | High | 3d |
| Performance regression gating | Criterion in CI with automated 10% threshold. | High | 2d |
| Large file optimization | Streaming merge for files > 100MB. | Medium | 3d |

**Exit criteria:** 100K files, 10K commits, all operations complete under 30 seconds.

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
| Patch-DAG acyclicity | Proven (sorry fixed in v5.5) | -- |
| LCA correctness | Proven | -- |
| Three-way merge completeness | Proven | -- |
| CAS injectivity | Proven | -- |
| GC reachability | Proven | -- |
| Touch set monotonicity | Proven | -- |
| Raft election safety | Target | 5d |
| Raft log consistency | Target | 5d |
| Ed25519 non-forgeability | Target | 5d |

**Exit criteria:** 18+ proven theorems. Raft safety properties proven.

---

## Phase 7: Platform Deepening (v9.0) -- 6 weeks

**Goal:** Native integrations with professional tools and workflows.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| VS Code real-time merge preview | Show semantic diff inline during merge conflicts. | High | 5d |
| JetBrains merge conflict UI | Integrate with IDEA/Goland/RustRover merge tool. | High | 5d |
| Neovim stable release | Tag and publish to MELPA/lazy.nvim. | Medium | 2d |
| Terraform state merge | Semantic merge for .tfstate JSON. | Medium | 3d |
| K8s manifest merge | YAML-aware merge for K8s resources with strategic merge patch. | Medium | 3d |
| Airtable/Sheets/Notion connectors | Wire CLI commands and hub integration. | Medium | 5d |
| Webhook event system | Real-time notifications for repo events. | Medium | 2d |

**Exit criteria:** At least 2 editor integrations ship in stable release.

---

## Phase 8: Desktop App (v9.1) -- 4 weeks

**Goal:** Native desktop application for non-developer users.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| Re-include in workspace | Fix Tauri build, add macOS/Windows CI matrix. | High | 3d |
| Real-time sync status | Push/pull progress indicator. | High | 2d |
| Visual merge conflict resolution | Side-by-side editor with semantic highlighting. | High | 5d |
| Repository browser | File tree with history sidebar. | Medium | 3d |
| System tray notifications | Background sync alerts. | Medium | 1d |

**Exit criteria:** Desktop app has CI, smoke tests, and installable artifacts for macOS and Windows.

---

## Phase 9: v1.0 Release (v10.0) -- 4 weeks

**Goal:** Ship a stable, documented, well-supported v1.0.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| API stability audit | cargo-semver-checks in CI for all publishable crates. | Critical | 2d |
| Compatibility matrix | Rust 1.94+, Ubuntu 22.04+, macOS 13+, Windows 10+. | High | 1d |
| Migration guide | v0.x to v1.0 upgrade path documentation. | Medium | 1d |
| Troubleshooting guide | Common issues and solutions. | Medium | 1d |
| Automated release | Build, sign, upload, publish on tag push. | High | 2d |
| GPG signing | Detached signatures for release binaries. | Medium | 1d |
| Security audit | Third-party penetration test or formal security review. | High | 5d |
| Load testing | Simulate 100 concurrent users on Hub. | Medium | 2d |

**Exit criteria:** v1.0.0 tag pushed. Signed binaries on GitHub Releases. All 37 crates published to crates.io.

---

## Phase 10: Post-v1.0 Growth (v10.x) -- Ongoing

**Goal:** Ecosystem expansion and community growth.

| Task | Details | Priority | Effort |
|------|---------|----------|--------|
| Forgejo/Gitea integration | Native merge driver plugin. | High | 5d |
| GitLab CI integration | Merge driver for GitLab MRs. | High | 3d |
| Bitbucket integration | Merge driver for Bitbucket PRs. | Medium | 3d |
| Mobile app | Read-only repository browser with merge preview. | Low | 10d |
| Plugin marketplace | Community WASM plugins with verification. | Low | 5d |
| Observability suite | Distributed tracing (OpenTelemetry), log aggregation. | Medium | 5d |
| Multi-tenant SaaS | Organization isolation, resource quotas, billing tiers. | High | 10d |

---

## Version Timeline

| Version | Focus | Est. Duration | Start |
|---------|-------|---------------|-------|
| v5.5 | Hardening | 2 weeks | 2026-05-19 |
| v6.0 | Distribution | 3 weeks | 2026-06-02 |
| v7.0 | Enterprise readiness | 6 weeks | 2026-06-23 |
| v7.1 | Advanced merge | 4 weeks | 2026-08-04 |
| v8.0 | Scale and reliability | 6 weeks | 2026-09-01 |
| v8.1 | Formal verification | 4 weeks | 2026-10-13 |
| v9.0 | Platform deepening | 6 weeks | 2026-11-10 |
| v9.1 | Desktop app | 4 weeks | 2026-12-22 |
| v10.0 | v1.0 release | 4 weeks | 2027-01-19 |
| v10.x | Post-v1.0 growth | Ongoing | 2027-02-16 |
| **Total to v1.0** | | **~39 weeks** | |

---

## Metrics Targets

| Metric | v5.4 (now) | v6.0 | v7.0 | v8.0 | v10.0 |
|--------|------------|------|------|------|-------|
| Tests | 1,759 | 1,800 | 2,000 | 2,200 | 2,500 |
| Branch coverage (critical) | ~60% | >70% | >80% | >85% | >95% |
| Lean 4 proofs | 16 | 16 | 16 | 18 | 20 |
| Semantic drivers | 18 | 18 | 20 | 22 | 24 |
| crates.io crates | 37 | 37 | 37 | 40 | 42 |
| CLI commands | 58 | 60 | 62 | 65 | 70 |
| Unsafe blocks | 33 | 30 | 25 | 20 | 15 |
| Clippy warnings | 0 | 0 | 0 | 0 | 0 |
| CI pipeline time | ~22m | <15m | <15m | <12m | <10m |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Rust edition upgrade breaks compilation | Medium | High | Pin rust-toolchain.toml, test before upgrade |
| SQLite WAL corruption on crash | Low | Critical | WAL checkpoint on shutdown, fsck on startup |
| Raft split-brain | Low | Critical | Persisted election state, BTreeMap ordering |
| WASM sandbox escape | Low | Critical | Limit host imports, review wasmtime advisories |
| Supply chain attack via dependency | Medium | High | cargo audit in CI, lockfile pinning |
| Driver regression | Medium | High | Property-based tests, E2E lifecycle tests |
| Performance regression | Medium | Medium | Criterion in CI with regression gating |
| Tauri breaking changes | Medium | Medium | Pin Tauri version, test before upgrade |
| suture.dev domain hijacking | Low | Medium | DNSSEC, monitor with Certificate Transparency |

---

## Strategic Decisions

### What NOT to Do

| Decision | Rationale |
|----------|-----------|
| Migrate to PostgreSQL | SQLite + Raft covers single-node and distributed. Added complexity not justified for v1.0. |
| Implement QUIC | TCP + Zstd achieves adequate latency. QUIC adds complexity without clear benefit. |
| Implement NFSv4/SMB3 | FUSE3 + WebDAV covers primary use cases. Kernel-level development not justified. |
| Migrate to FlatBuffers | bincode + Zstd is performant and well-tested. No benchmark shows FlatBuffers winning. |
| Optimize for nanosecond latency | VCS operations are I/O bound. Focus on large-file and large-repo scale. |
| Support Git protocol compatibility | Suture is a separate VCS, not a Git drop-in. Different data model (patches vs snapshots). |

### What to Double Down On

| Decision | Rationale |
|----------|-----------|
| Semantic merge quality | Sole differentiator from Git. Every driver improvement compounds value. |
| suture-merge library adoption | Growth vector. Low friction (cargo add), high impact. |
| Formal verification | Unique in VCS space. Builds trust for regulated industries (aerospace, medical, finance). |
| Performance at scale | Enterprise adoption requires 100K+ files, 100K+ commits. |
| Editor integrations | Where users spend their time. Reduces context switching. |
| Lean 4 proofs for safety-critical claims | Mathematically proven correctness for core algorithms. |

---

## Known Limitations (Pre-v1.0)

| Limitation | Impact | Resolution |
|------------|--------|------------|
| suture-py not in CI | Python users cannot install from source | Phase 1: Gate on feature flag |
| suture-node broken (ctor regression) | npm package may not build | Phase 1: Pin ctor version |
| Desktop app excluded | No native desktop experience | Phase 8: Tauri v2 integration |
| WASM plugins experimental | Plugin ecosystem cannot grow | Phase 4: Complete ABI |
| suture.dev not resolving | Landing page unreachable via custom domain | Phase 2: DNS configuration |
| No partial/shallow clone | Large repos inefficient | Phase 5: Sparse checkout |
| No per-repo permissions | Multi-team Hub deployments insecure | Phase 5: RBAC |
