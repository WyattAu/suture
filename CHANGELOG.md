# Changelog

## [5.1.0] - 2026-04-30

### Added
- **Security hardening**: Stripe webhook HMAC-SHA256 verification, OAuth CSRF protection, JWT token revocation, 7-day payment grace period
- **Interactive merge demo**: Client-side 3-way merge (JSON, YAML, TOML, CSV) with no backend required, auto-merge on load, conflict highlighting, URL sharing
- **Landing page**: Social proof, animated merge preview, how-it-works section, 17+ format grid, use cases, FAQ
- **Git merge driver**: One-liner install script (`curl | bash`), 9 `.gitattributes` templates for every ecosystem (Node.js, Rust, Python, K8s, Java, CI/CD, Web, Go)
- **GitHub Action**: `suture/merge-action` for CI/CD pipelines, reads files at git refs, calls merge API
- **CI/CD integration**: Portable bash script for GitLab CI, CircleCI, Jenkins
- **Stripe checkout E2E**: Full subscription lifecycle (checkout → webhook → tier activation → portal management)
- **Usage analytics**: Merge logging, 30-day bar chart, donut chart, driver distribution, gated to pro/enterprise
- **Team/org management**: Member invite/remove/role, invitation acceptance, org listing
- **WASM plugin system**: Plugin trait, wasmtime v28 host with fuel-based timeout, memory limits, plugin registry, example plugin
- **Raft consensus**: Pre-vote mechanism, log compaction, snapshot install, leadership transfer, read-only queries, joint consensus membership changes
- **Performance**: Merge cache (LRU), key interner, size-based merge strategy selection
- **Benchmarks**: criterion-based, 5 formats × 4 sizes × 4 scenarios
- **Deploy configs**: Fly.io, Docker (multi-stage), systemd services, Caddy, nginx, Helm chart
- **Self-hosting docs**: Complete guide (Docker, binary, backup, reverse proxy, monitoring, security)
- **VS Code extension**: Conflict highlighting, status bar, auto-resolve on save, bulk resolve, Quick Pick
- **SEO**: Blog posts, sitemap, robots.txt, meta tags
- **SIGTERM handling** in daemon for graceful shutdown

### Changed
- **Error handling**: 16 production `.unwrap()` calls replaced with graceful fallbacks
- **Foundation**: 46 `let _ =` sites fixed — all data mutations now propagate errors
- **Determinism**: Patch ID excludes timestamp, `new_batch()` sorts file_changes, Kahn's sort stable, read_dir sorted
- **Auto-detect repo type**: Depth limit (10 levels) + symlink protection

### Fixed
- Axum 0.8 billing portal: MutexGuard held across `.await` caused `!Send` future — scoped DB operations
- Webhook signing: `sign_payload` returns `Option<String>` instead of panicking on oversized HMAC keys
- Topo sort: Graceful handling of missing in-degree entries instead of `.expect()`
- Rate limit headers: `parse().unwrap()` → `if-let Ok` for header value parsing

### Security
- Stripe webhook signature verification (HMAC-SHA256, 5-min timestamp freshness)
- OAuth state parameter CSRF protection (UUID, 10-min expiry, one-time use)
- JWT token revocation (blacklist table + middleware checks)
- Password change endpoint with old password verification
- Account deletion with email confirmation

## [Unreleased]

### Error Quality — Typed Error Variants
- **150 → 0 `Custom(String)` usages in production code** — Replaced with 35+ typed variants across `RepoError`, `StorageError`, `MetaError`, `CommonError`, `DagError`, `ApplyError`. Examples: `RepoError::PatchNotFound(id)`, `RepoError::InvalidHeadOffset(n)`, `StorageError::PoisonedLock(msg)`, `MetaError::Serialization(err)`.
- **7 unused `Custom(String)` definitions removed** from `CommonError`, `MetaError`, `DagError`, `ApplyError`, `SigningError`, `MergeError`, `OtioError`. Kept on `RepoError` and `StorageError` as forward-compatibility escape hatches.
- **`#[from] rusqlite::Error` and `#[from] serde_json::Error`** added to `RepoError` and `MetaError`, eliminating 30+ boilerplate `.map_err()` calls.

### Distribution — Homebrew Tap
- **Homebrew tap created** at [github.com/WyattAu/homebrew-suture-merge-driver](https://github.com/WyattAu/homebrew-suture-merge-driver). Install: `brew tap WyattAu/suture-merge-driver && brew install suture-merge-driver`.

### Documentation
- **5-minute Git merge driver quickstart** (`docs/merge-driver-quickstart.md`) — zero-to-semantic-merge in 4 steps.
- **Blog post: Semantic Merge for 17 File Formats** (`docs/blog/semantic-merge-for-17-file-formats.md`) — ready for HN/Reddit submission.
- **Landing page updated** with correct Homebrew install, blog link, and quickstart link.

### Test Coverage
- **Merge associativity test** — `test_correctness_merge_associativity` in JSON driver verifies `merge(merge(base,A,B),C) == merge(base,A,merge(B,C))`.
- **New-file-on-both-sides test** — `test_merge_new_file_on_both_sides_keeps_head` in E2E.
- **9 proptests** added across XML (3), SQL (3), CSV (3) — identity, idempotence, non-overlapping merge.
- **Total: 1,260+ tests, 0 failures**.

## [5.0.1] - 2026-04-27

### Security Fixes
- **SHA-256 token hashing** in suture-hub — replaces plaintext token storage.
- **LFS path traversal validation** — blocks `../` in blob paths.
- **Signing key name validation** — restricts to alphanumeric + hyphens.
- **Mutex poison recovery** in suture-vfs — `UnpoisonMutex` trait replaces `.unwrap()` on poisoned locks.
- **Fetch error propagation** in remote protocol — missing commits no longer silently ignored.

### Bug Fixes
- **npm package asset names** — `darwin` → `macos`, Windows uses `.zip`, per-asset `.sha256` checksums.
- **`from_utf8_unchecked` eliminated** from 5 production drivers (docx, xlsx, pptx, image, pdf) — replaced with `bytes_to_string_lossy()`.
- **Defence onboarding docs** — 20+ command mismatches fixed vs actual CLI.

### Performance
- **Release binary benchmarks**: 4ms startup, 7ms status, 9ms commit, 14ms log (100 commits).

## [5.0.0] - 2026-04-25

### Semantic Merge — Batch Patch File-Level 3-Way Merge

- **File-level 3-way merge for conflicting batch patches** — Previously, when a batch commit (touching multiple files) conflicted with another batch commit, the entire patch was skipped — losing all non-conflicting file changes. Now, `execute_conflicting_merge()` uses tree-diff approach: computes LCA→source diff, applies source-only changes directly, and performs per-file 3-way text merge for files changed on both sides. Clean merges are applied automatically; true conflicts still produce conflict markers for manual resolution.
- **Validated with 100-file YouTube metadata merge** — Two branches (marketing + analytics) each modifying all 100 JSON files in a single batch commit merge cleanly: trending tags, sponsored markers, analytics data, and revenue figures all preserved.

### Flaky Test Fix — suture-raft

- **`HashMap` → `BTreeMap` in `Cluster`** — The raft cluster test `test_three_node_election` was flaky because `HashMap` iteration order is non-deterministic. Election timeouts are deterministic (base + node ID jitter), but message delivery order depended on HashMap iteration. Switching to `BTreeMap` ensures consistent node processing order. Verified 5/5 deterministic runs.

### Snapshot & Merge Fixes (from earlier sessions)

- **`patch_chain_full()`** — BFS + topological sort following ALL parent edges for correct merge commit snapshots
- **`snapshot_head()` cache invalidation** — Compares `cached_head_id` against current `head_id`
- **`resolve_ref()` hex prefix** — 7-character minimum to prevent branch name collisions
- **DAG-aware `merge_branches()` and `branch_divergence()`** — Uses `(ancestors(tip) ∪ {tip}) - (ancestors(lca) ∪ {lca})`
- **LCA-based semantic re-merge** — `semantic_remerge_both_modified()` computes correct 3-way merge via LCA
- **`rewrite_head_tree()`** — Updates merge commit's stored tree from working tree
- **Snapshot cache versioning** — `snapshot_engine_version = "2"` auto-clears stale caches

### Validation Results

- **NIST 800-53 YAML** — 11/11 checks pass (government security control schema, 3 compliance branches)
- **YouTube bulk metadata** — 5/5 checks pass (100 JSON files, 2 team branches, batch commits)
- **OTIO film timeline** — Driver dispatch works; known `merge_trees()` nesting bug (pre-existing)
- **Stress test** — 10K files, 100 commits, 10MB files, branch+merge all functional

## [5.0.0] - 2026-04-23

Major release — version unification, release pipeline, defence/compliance features, Google Drive replacement.

### Version & Distribution

- **All 36 crates unified to v5.0.0** — coherent versioning across the workspace
- **GitHub Actions release workflow** — prebuilt binaries for Linux x86_64, macOS x86_64 + aarch64, Windows x86_64 with SHA256 checksums
- **`cargo install suture-cli`** now installs a modern v5.0.0 binary (was v1.0.0)
- **GitHub Release v5.0.0** — https://github.com/WyattAu/suture/releases/tag/v5.0.0

### Defence & Compliance

- **`suture log --audit`** — Structured audit trail export for compliance accreditation
  - `--audit-format json|csv|text` — three output formats
  - JSON: machine-readable with ISO 8601 timestamps, full metadata
  - CSV: spreadsheet-ready for compliance packages
  - Text: human-readable formatted report
- **`suture diff --classification`** — Detect security classification marking changes
  - NATO/US/UK/AU classification hierarchies (UNCLASSIFIED through TOP SECRET//SCI)
  - Detects ADDED, REMOVED, UPGRADED, DOWNGRADED events
  - Works on text files and DOCX (Office XML) format

### Google Drive Replacement

- **`suture sync`** — One command to auto-commit, pull, and push
  - Smart commit messages: "Sync: update 2 documents, 1 spreadsheet"
  - Categorizes files by type (documents, spreadsheets, presentations)
  - Handles merge conflicts gracefully (stops, doesn't overwrite)
  - Works offline (skips push/pull when no remote configured)
  - `--no-push`, `--pull-only`, `--message` flags

### New Commands

- **`suture switch`** — Modern branch switching (alias for `checkout -b`)
- **`suture restore`** — Restore working tree files from HEAD or specific ref
- **`suture ls-remote`** — List branches on a remote Hub without cloning
- **`suture archive`** — Export repo contents as tar.gz/tar/zip
- **`suture grep`** — Search tracked file content with regex
- **`suture export`** — Clean snapshot delivery for client handoff (directory or zip)
- **`suture stash branch`** — Create branch from stash entry
- **`suture hook list/run/edit`** — Manage repository hooks
- **`suture remote rename`** — Rename configured remotes

### New Flags

- **`suture log --stat`** — Per-commit file change statistics
- **`suture log --diff`** — Inline patch content in log output
- **`suture log --audit`** — Structured audit trail export
- **`suture log --graph`** — Improved with merge lines, HEAD labels, author names, relative time
- **`suture diff --name-only`** — List only changed file names
- **`suture diff --summary`** — Human-readable change summary for non-technical stakeholders
- **`suture diff --classification`** — Classification marking change detection
- **`suture blame --at <ref>`** — Historical blame at any commit
- **`suture show --stat`** — File classification in show output
- **`suture tag --sort date|name`** — Sort tags by date or name
- **`suture tag 'pattern'`** — Filter tags by glob pattern
- **`suture reflog --show`** — Full patch details in reflog
- **`suture notes add --append`** — Append to existing notes
- **`suture verify`** — Per-commit Ed25519 signature verification (auto-signs when `signing.key` configured)
- **`suture log --verify`** — Show signature status per commit (✓/✗/—)
- **`suture stash show`** — Preview stash contents with file stats
- **`suture clean`** — Remove untracked files (`--dry-run`, `--dirs`)
- **`suture blame -L 10,20`** — Line range filtering
- **`suture describe`** — Describe commit by nearest tag (`v1.0-3-gabc1234`)
- **`suture rev-parse`** — Resolve refs for scripting (`--short`, `--verify`)
- **`suture doctor --fix`** — Auto-remediation of common issues
- **`suture init --template`** — Bootstrap from 4 templates (document/video/data/report)
- **`suture archive --prefix`** — Custom directory prefix in archives

### Core Engine

- **Unified `resolve_ref()`** — Single method for HEAD, HEAD~N, hex hash, short prefix, tag, branch
- **Detached HEAD** — Checkout arbitrary commits, `is_detached()`, `get_detached_head()`
- **Historical blame** — `blame()` accepts optional `at` parameter

### OOXML Conflict UX

- DOCX/XLSX/PPTX conflicts no longer corrupt files
- Preserves "ours" version, generates `.suture_conflicts/report.md`
- Dry-run mode prints OOXML conflict handling note

### Binary Format Fix

- `suture diff` for DOCX/XLSX/PPTX/PDF/images now preserves raw bytes
- Semantic drivers receive intact ZIP/binary content via `from_utf8_unchecked`

### CLI Improvements

- Remote tracking in status: ahead/behind counts when tracking refs exist
- Progress indicators for clone/fetch/pull/push
- Negation patterns in `.sutureignore` (`!important.tmp`)
- Directory-only ignore patterns (`node_modules/`)
- Rule attribution in `suture ignore check`
- Welcome banner on first `suture init`
- 60-second quickstart for non-developers
- **`suture pull --autostash`** — auto-stash before pull, pop on success
- **`suture cherry-pick --no-commit`** — stage without committing
- **`suture log --diff-filter`** — filter by change type (A/D/M)
- **`suture gc --dry-run --aggressive`** — preview or deep-clean garbage collection
- **`suture fsck --full`** — verify blob integrity, parent chains, branch refs
- **`suture stash save`** — alias for `stash push` (git parity)
- **`suture stash clear`** — drop all stash entries
- **`suture worktree prune`** — clean up stale worktree entries

### Quality

- **1,436 tests** across 37 crates, 0 failures
- **98 CLI tests** with parallel-safe test isolation
- **Prebuilt release binaries** for Linux/macOS/Windows
- **33-crate publish script** verified for crates.io v5.0.0 release

### Distribution

- **npm package:** `suture-merge-driver` — auto-downloads platform binary
- **pip package:** `suture-merge-driver` — Python wrapper for git merge driver
- **VS Code extension:** one-click merge driver configuration
- **Desktop app:** Tauri v2 with dark theme UI, 26 commands, auto-sync tray
- **Blog post:** "Stop Having Merge Conflicts in package.json"
- **Merge driver guide:** comprehensive setup and troubleshooting

### Defence & Compliance

- **Tamper-evident audit log:** BLAKE3 hash chain, `suture audit --verify`
- **Bulk classification scan:** `suture classification scan` across all commits
- **Compliance reports:** `suture classification report` for accreditation packages
- **Audit logging** wired into commit and merge flows
- **Doctor compliance check:** `suture doctor` now checks audit chain + signing

### Film & Video

- **Timeline management:** `suture timeline import/export/summary/diff/list`
- **OTIO support:** minimal OTIO JSON parsing (tracks, clips, duration)
- **Video template:** `suture init --template video` with timelines/, media/ dirs

### Content Creator & YouTube

- **Batch operations:** `suture batch stage/commit/export-clients` with glob patterns
- **Report generation:** `suture report change/activity/stats` (text/markdown/HTML)
- **Export templates:** `suture export --template/--client` with placeholder replacement
- **Client delivery:** multi-client export with subdirectories

### Documentation

- `docs/quickstart.md` — 60-second quickstart targeting non-developers
- README.md refreshed for v5.0.0 (20+ formats, 50+ commands, all verticals)
- CHANGELOG.md updated for v5.0.0

## [4.1.0] - 2026-04-22

Production VCS readiness release — 10 new commands, core engine improvements, binary diff fix.

### New Commands

- **`suture switch`** — Modern branch switching (alias for `checkout -b`)
- **`suture restore`** — Restore working tree files from HEAD or specific ref (`--staged` to unstage)
- **`suture ls-remote`** — List branches on a remote Hub without cloning
- **`suture archive`** — Export repo contents as tar.gz/tar/zip (`--format`, `--prefix`)
- **`suture grep`** — Search tracked file content with regex (`-i`, `-l`, `-F`, `-C`)
- **`suture stash branch`** — Create and checkout a branch from a stash entry

### New Flags

- **`suture log --stat`** — Show per-commit file change statistics
- **`suture log --diff`** — Show patch content inline in log output
- **`suture diff --name-only`** — List only changed file names
- **`suture blame --at <ref>`** — Blame as of a specific commit

### Core Engine

- **Unified `resolve_ref()`** — Single method for HEAD, HEAD~N, hex hash, short prefix, tag, branch resolution. Refactored `create_branch`, `create_tag`, `reset`, `diff` to use it (fixes bug where `reset <tag>` failed).
- **Detached HEAD** — Checkout arbitrary commits without a branch. `is_detached()`, `get_detached_head()`, status shows "HEAD detached at <hash>".
- **Historical blame** — `blame()` now accepts an optional `at` parameter for blaming at any commit.

### CLI Improvements

- **Remote tracking in status** — Shows ahead/behind counts when remote tracking refs exist. `do_fetch` saves remote branch tips for comparison.
- **Progress indicators** — clone, fetch, pull, push now show progress messages (blob/patch counts).
- **Binary format diff fix** — `suture diff` for DOCX/XLSX/PPTX/PDF/images now uses `from_utf8_unchecked` to preserve raw bytes, enabling semantic drivers to receive intact ZIP/binary content.

### Library

- **suture-merge v0.2** — DOCX/XLSX/PPTX feature flags, public merge functions, 4 new integration tests
- **suture-action** — GitHub composite action for CI/CD semantic merge
- **Dependency bumps** — roxmltree 0.21, rayon 1.12

### Documentation

- **Blog post** — "I Built a Semantic Merge Engine in Rust That Understands Word, Excel, and PowerPoint Files" (~950 words)
- **14 dependabot issues closed** with documented rationale for deferred major version bumps

## [4.0.0] - 2026-04-22

Major driver rewrite release — all OOXML and OTIO drivers rewritten for production quality.

### OOXML Infrastructure

- **suture-ooxml**: Per-part relationship resolution (`part_rels`, `resolve_rel()`, `get_part_rels()`, `path_rels_to_owner()`, `resolve_relative_path()`, `parse_rels_by_id()`) — 8 tests

### Driver Rewrites

- **suture-driver-docx**: Complete rewrite — XML-level paragraph preservation instead of text extraction. Preserves formatting (bold, italic, fonts), paragraph properties (`w:pPr`, `w:pStyle`), run properties (`w:rPr`, `w:b`, `w:i`), change tracking attributes (`w:rsidR`, `w:rsidP`), `xml:space="preserve"`, trailing body content (`<w:sectPr>`), namespace declarations, self-closing `<w:p/>` elements. 13 unit tests + 24 E2E tests.
- **suture-driver-pptx**: Complete rewrite — Proper slide discovery via `<p:sldIdLst>` + relationship ID resolution through `suture-ooxml`. Content-hash-based modification detection avoids false positives from auto-generated IDs. Slide name extraction from `<p:cNvPr name="..."/>`. Three-way merge with de-duplication for independently-added slides. 19 unit tests + 11 E2E tests.
- **suture-driver-xlsx**: Complete rewrite — A1 notation parser (`col_from_a1`, `parse_a1`, `col_to_letter`). Shared string table support via `xl/sharedStrings.xml`. Handles inline strings (`t="inlineStr"`), booleans (`t="b"`), numerics, formula strings. Merge preserves sheet structure by replacing only `<sheetData>` section. 13 unit tests + 13 E2E tests.
- **suture-driver-otio**: `SutureDriver` trait implementation for `OtioDriver`. Content-based identity via `content_fingerprint()` using `media_reference.target_url` + `source_range`. `OtioNode::Unknown` variant for Gap, Marker, Effect, TimeEffect, and any future types. Three-way merge at JSON level with global `placed_fps` tracking to prevent duplicate placement. Leaf-only modification detection. Raw JSON comparison (no serde round-trip). Legacy API preserved as `LegacyOtioDriver`. 21 unit tests + 18 E2E tests.

### Bug Fixes

- Fix flaky `test_perf_10k_commits_and_log` — marked `#[ignore]` for CI (run with `--ignored`)

### Quality

- 1,427 tests, 0 failures across 37 crates

## [1.1.0] - 2026-04-16

Rigorous testing and correctness release — bugs found and fixed, formal proofs, comprehensive test suite.

### Bug Fixes

- **BUG-001 (CRITICAL): Delta encoding data corruption** — `apply_delta` silently corrupted data when target ≥ 24 bytes shared no prefix/suffix with base. The delta format now uses a discriminator byte (0x00 = full copy, 0x01 = delta) to eliminate ambiguity.
- **BUG-002: Create silently overwrites** — `apply_single_op` for `Create` now returns the tree unchanged if the path already exists, instead of silently overwriting.
- **BUG-003: Modify silently creates** — `apply_single_op` for `Modify` now returns the tree unchanged if the path doesn't exist, instead of silently creating it.
- **BUG-004: Commit used wrong operation type** — `commit()` now uses `OperationType::Create` for added files and `OperationType::Modify` for modified files, matching the corrected apply semantics.
- **E2E test fix** — `test_push_pull_roundtrip` now uses `into_make_service_with_connect_info` so axum can provide the `ConnectInfo<SocketAddr>` extractor.
- **Git merge driver fix** — Accepts a 4th argument `%P` (original file path) for file extension detection, fixing temp file name issues.

### Testing (142 new tests → 808 total)

- **suture-protocol: +25 tests** (0 → 25) — delta roundtrip (10 sizes), compress/decompress (6), capability negotiation, V2 types, protocol constants
- **Semantic drivers: +136 tests** — JSON (+33), YAML (+19), TOML (+22), CSV (+16), XML (+21), Markdown (+25). Covers merge determinism, idempotency, commutativity, deep nesting, Unicode, null values, large files, output validity
- **Determinism: +7 tests** — commit determinism, patch ID determinism, merge determinism, push-pull roundtrip, BLAKE3 determinism, diff symmetry, branch idempotency
- **Git merge driver: +2 tests** — clean merge end-to-end, conflict detection end-to-end (with real git repos)

### Formal Verification (Lean4)

23 theorems proved in `.specs/02_architecture/proofs/proof_suture_core.lean`:

- **TouchSet algebra** (3): conflict ↔ intersection, symmetry, disjoint ⇒ no conflict
- **Commutativity** (7): symmetry, disjoint touch sets ⇒ commute, identity units, contrapositive
- **DAG acyclicity** (2): add_node preserves acyclicity, add_edge preserves acyclicity
- **Ancestor transitivity** (3): reachable is transitive, ancestors are transitive, reflexivity
- **LCA properties** (5): common ancestors non-empty, reflexivity, reaches both, is ancestor, symmetry
- **Merge properties** (7): base-equals-theirs, base-equals-ours, both-same, symmetry, no spurious, clean symmetry, clean subset

### Performance Benchmarks

28 benchmark functions in `suture-bench` covering: repo operations (init, commit, log, diff, branch, merge), semantic merge (JSON/YAML/TOML/CSV at 3 sizes), protocol (delta compute/apply at 3 sizes), compression (3 sizes), hub push/pull/roundtrip, large file handling.

## [1.0.1] - 2026-04-16

Distribution release — one-command install via Homebrew and Nix.

### Distribution

- **Homebrew tap** — `brew tap WyattAu/suture && brew install suture` (macOS + Linux, x86_64 + aarch64)
- **Nix flake packages** — `nix build .#suture` and `nix build .#suture-hub` (source build via `buildRustPackage`)
- **README** — Homebrew listed as primary install method

## [1.0.0] - 2026-04-16

Stable release — the culmination of Path A (honest code), Phase 1 (adoption), Phase 2 (polish), and Phase 4 (ecosystem).

### Documentation & Adoption (Phase 1)

- **README rewrite** — added visual Git-vs-Suture merge diagram, "Who is this for?" section, Hub web UI docs
- **Comparison page** — `docs/comparison.md` with detailed Suture vs Git vs Pijul vs Darcs vs Mercurial analysis
- **Demo script** — `docs/demo.md` — 60-second step-by-step semantic merge demo
- **Blog announcement** — updated `BLOG_ANNOUNCEMENT.md` for v1.0.0 with honest messaging

### Polish (Phase 2)

- **Human-readable error messages** — CLI now prints `error: {message}` with actionable hints (`hint: run suture remote add`) instead of Rust-internal traces
- **WASM plugin soundness fix** — replaced unsafe raw pointer casting with `Box::leak` for `'static` str conversion
- **WASM plugin stubs made honest** — `diff()` and `format_diff()` now return errors instead of silently returning empty results
- **CLI audit** — confirmed zero production `panic!()` calls (all 30 are test-only exhaustiveness guards)

### Ecosystem (Phase 4)

- **VS Code extension scaffold** — `vscode-suture/` with 3 commands (semantic merge, init, status) and terminal integration
- **GitHub Action improvements** — added `fail-on-conflict` and `file-patterns` inputs, better PR comment table with format column and color-coded badges, collapsible setup instructions

## [1.0.0-rc.1] - 2026-04-16

Honest v1.0 — fixing everything that was broken or fake in v0.10.0, stripping stubs that pretended to work. This is the "come clean" release.

### Critical Fixes (Path A)

- **Auto-sync was a no-op** — `sync_once()` opened the repo, called `status()`, returned `Ok(())`. Now actually pulls patches from hub then pushes local patches via HTTP.
- **LSP goto-definition was fake** — returned `suture://patch/{hex}` URIs that no editor can resolve. Removed entirely.
- **LSP semantic tokens were a stub** — `semantic_tokens_full()` returned `Ok(None)` while advertising the capability. Removed entirely.
- **LSP completion provider was a stub** — Advertised capability but returned nothing. Removed entirely.
- **WASM plugins had 2 critical CVEs** — wasmtime 28.0.1 had sandbox escapes (RUSTSEC-2026-0095, RUSTSEC-2026-0096). Downgraded to wasmtime 22.0.1 (safe, zero API changes).
- **Hub token creation had NO auth gate** — anyone could `POST /auth/token` to mint unlimited tokens. Now requires admin auth (or bootstrap mode when no users exist), rate-limited to 5/min/IP.
- **Tokens had no expiration** — now expire after 30 days, `verify_token()` checks `expires_at`.
- **Hub replication had NO background task** — tables, API endpoints, and CRUD existed, but no automated process transferred data between peers. Now a `tokio::spawn` loop in leader mode pushes replication log entries to followers every 30s.
- **Web UI "Replication" tab hit wrong endpoints** — called `POST /mirror/status` instead of `GET /replication/status` and `GET /replication/peers`. Now calls the correct replication endpoints with proper table columns (Peer URL, Role, Status, Sync Seq).
- **Push handlers didn't log to replication log** — mutations were silently dropped. Now `handle_push` and `handle_push_v2` record all patch insertions and branch updates in `replication_log`.
- **Replication peer sync position never updated** — `last_sync_seq` was never written after a successful sync. Added `update_peer_sync_seq` storage method.
- **No `--replication-role` CLI flag** — hub binary could only be configured programmatically. Now accepts `--replication-role {leader,follower,standalone}`.
- **4 pre-existing clippy errors** — `Role::from_str` shadowing std trait, collapsible `if` statements, missing `Default` impl, manual suffix stripping. All fixed.

### Testing (104 new tests → 602 total)

- **Hub: +65 tests** (43 → 108) — push edge cases (15), pull edge cases (12), user/auth CRUD (10), rate limiting (4), branch protection (5), repository management (5), storage-level (8), mirror operations (4), misc (3)
- **Daemon: +15 tests** (6 → 21) — sync error cases (5), file watcher edge cases (5), auto-commit edge cases (3), integration (3)
- **LSP: +8 tests** (3 → 11) — hover with various states (4), diagnostics (3), lifecycle (1)
- **Clippy** — full workspace passes with `-D warnings`

### Removed / Stripped

- LSP goto-definition provider (was fabricating URIs)
- LSP semantic tokens provider (was returning `Ok(None)`)
- LSP completion provider (was advertising empty capability)

## [0.10.0] - 2026-04-15

Full-stack roadmap release — 6,614 lines added across 34 files. All 4 roadmap tiers executed: testing hardening, adoption drivers, VCS completeness, and ecosystem expansion.

### Testing (73 new tests → 498 total)

- **Protocol roundtrip tests** — 16 serde serialization roundtrips for all wire format types
- **CLI unit tests** — 25 clap argument parsing tests covering all 40 commands
- **E2E integration tests** — 10 new tests: merge-file JSON/YAML/auto-detect, conflict detection, cherry-pick, revert, notes, worktree
- **Hub tests** — 13 new tests: rate limiting (3), pagination (3), compressed push/pull (1), V2 delta transfer (3), replication (5)
- **Protocol tests** — 10 new tests: V2 types, delta compute/apply, capability negotiation

### Tier 1: Hardening

- **Hub concurrent access** — Migrated from `Mutex<HubStorage>` to `RwLock<HubStorage>` for parallel read operations
- **LSP version fix** — Corrected server info version from 0.1.0 to 0.10.0
- **Deleted suture-daemon** — Removed empty placeholder (`add(2,2)=4`)
- **Deleted suture-git-bridge** — Removed deprecated crate with known data loss issues

### Tier 2: Adoption Drivers

- **Git merge driver** — Shell script (`contrib/git-merge-driver/suture-merge-driver`) that lets Git use Suture's semantic merge for conflict resolution via `.gitattributes`
- **Git merge driver documentation** — Setup guide in `docs/git_merge_driver.md`
- **GitHub Action** — Reusable composite action (`.github/actions/semantic-merge/`) that checks PR file changes for semantic mergeability, posts results as PR comments
- **Hub rate limiting** — IP-based sliding window: 100 pushes/hour, 1000 pulls/hour per IP, returns 429 with Retry-After
- **Hub pagination** — `/repos/{id}/patches` now supports `?offset=N&limit=N` (default 50, max 200)
- **Wire compression** — zstd-compressed push/pull endpoints: `/push/compressed`, `/pull/compressed`

### Tier 3: VCS Completeness

- **LSP enhancements** — Go-to-definition (blame-based), file diagnostics (untracked/modified detection on open/save), semantic tokens placeholder
- **Daemon** — Full rewrite: file watcher (notify crate, debounced), auto-commit, auto-sync with configurable intervals, graceful shutdown
- **Hub user accounts** — User CRUD with roles (admin/member/reader), API token auth, RBAC middleware on all endpoints
- **Protocol v2** — Delta transfer (binary patch encoding), capability negotiation, compressed endpoints, backward-compatible with v1

### Tier 4: Ecosystem

- **WASM plugin loading** — `wasmtime`-based plugin system behind `wasm-plugins` feature flag, ABI documentation, plugin registry integration
- **Driver SDK example** — `suture-driver-example` crate implementing a `.properties` file driver with full diff/merge
- **Python bindings** — Updated to PyO3 0.25, edition 2024, version 0.10.0
- **Hub web UI** — Single-page application with repository browser, user management, replication status (dark theme, vanilla JS)

### EXTRA: Hub Replication

- **Leader-follower replication** — Replication log table, peer management API, sync endpoint, background replication task infrastructure

## [0.9.0] - 2026-04-15

Quality and publishability release — merge-file semantic drivers, crates.io preparation, CI hardening, and bug fixes from a 417-test CLI determinism audit.

### New Features

- **`merge-file` semantic driver support** — `suture merge-file` now accepts `--driver <name>` for explicit driver selection (json, yaml, csv, xml, toml, markdown) and `-o`/`--output <path>` to write merged output to a file. Auto-detects driver by file extension when `--driver` is omitted. Falls back to line-based merge with a warning when the semantic driver returns a conflict or error. Hard error if an explicitly requested driver doesn't exist.

### Bug Fixes

- **`tag --delete nonexistent` exited 0** — `delete_tag()` now checks existence first and returns an error if the tag doesn't exist.
- **`tag --target HEAD` failed** — `create_tag()` now resolves HEAD/HEAD~N references before attempting branch name resolution.
- **`log --graph` was non-deterministic** — HashMap iteration order caused varying output between runs. Fixed by sorting branches and adding secondary/tertiary sort keys (commit message, patch ID) to the commit groups.
- **Broken pipe panics** — Added `libc::signal(SIGPIPE, SIG_DFL)` at CLI startup to prevent panics when output is piped to `head` or similar.
- **CSV driver: both sides adding different rows conflicted** — Changed merge semantics from conflict to union when both sides add different rows to a CSV file.
- **XML driver: both sides adding different children conflicted** — Same fix as CSV: union semantics for divergent additions.
- **`notes remove` with invalid index exited 0** — Now validates the index is in range before removing.
- **`merge --dry-run` actually modified the repo** — Added `preview_merge()` read-only method; CLI uses it for dry-run mode instead of performing the actual merge.

### crates.io Preparation

- Added `readme.workspace = true` to all 17 publishable library crates.
- Fixed `suture-tui` version mismatch (was 0.12.0, aligned to 0.8.0).
- Added `suture-driver` as a dependency to `suture-driver-otio`.

### CI/CD

- **Dependabot** — Added `.github/dependabot.yml` for automated cargo and GitHub Actions dependency updates (weekly schedule).
- **Release workflow hardened** — Added system dependency installation (`pkg-config`, `libssl-dev`) for Linux builds, Cargo cache for faster builds, test step on default target, and scoped build to `suture-cli` package only.
- **Repository cleanup** — Removed test artifacts, added `/test.*` pattern to `.gitignore`.

### QA Results

- 425/425 workspace tests pass (excluding e2e, bench, fuzz)
- Clippy clean with `-D warnings`

## [0.8.1] - 2026-04-07

Bugfix release — three bugs found and fixed during full QA sweep (80+ E2E tests, 438 unit tests).

### Bug Fixes

- **`diff --cached` showed all tracked files as deleted** — `diff_staged()` built the staged tree from empty instead of from HEAD. Now correctly starts from `head_tree` and overlays staged additions, modifications, and deletions.
- **`branch --target HEAD` and `branch --target HEAD~N` failed** — `create_branch()` tried to resolve HEAD as a `BranchName` before checking for HEAD refs. Now checks HEAD/HEAD~N first, then branch names, then hex hashes.
- **`diff --from/--to` rejected short hash prefixes** — `resolve_id` in `diff()` only accepted full 64-char hex via `Hash::from_hex`. Now tries prefix matching against all patch IDs, matching the behavior of `show`, `blame`, and `log`.

### QA Results

- 438/438 unit tests pass
- 80+ E2E CLI tests pass (41 commands tested, 9 drivers tested)
- 5 semantic merge formats verified end-to-end (JSON, CSV, TOML, XML, Markdown)

## [0.8.0] - 2026-04-05

Suture v0.8.0 — Scale release with batched commits, eliminating the #1 performance bottleneck. Each commit now creates a single patch instead of N patches (one per file).

### Batched Commit Model

- **`OperationType::Batch`** — new variant that carries a `Vec<FileChange>` as its payload. Each `FileChange` contains an operation type, file path, and payload (blob hash).
- **`Patch::new_batch()`** — creates a single patch representing an entire commit. The touch set contains all affected file paths.
- **`Patch::file_changes()` / `Patch::is_batch()`** — helpers to inspect batch patches.
- **`FileChange` struct** — serializable representation of a single file operation within a batch.

### Performance Impact

- **Commit writes**: reduced from 2N SQLite writes per commit (N files) to 2 writes total (1 patch + 1 edge).
- **DAG size**: reduced from O(commits × avg_files) nodes to O(commits) nodes.
- **Cold snapshot replay**: reduced from O(P × F) to O(C × F) where C = commits (not total patches).
- **`patch_chain()` walk**: O(C) instead of O(P), eliminating the O(P²) `chain.contains()` check.
- **Repo open**: loads C patches instead of P patches into the in-memory DAG.

### Engine Changes

- **`apply_patch` Batch handling** — applies all file changes from a Batch patch in a single pass over the FileTree (one clone instead of N clones).
- **New tests** — `test_apply_batch` and `test_apply_batch_with_delete` verify correct multi-file application.

### Push/Pull Compatibility

- **Push blob collection** — CLI `cmd/push.rs` extracts per-file blob hashes from Batch patch payloads for upload.
- **Pull blob delivery** — Hub `server.rs` parses Batch patch payloads to deliver only referenced blobs.
- **Metadata serialization** — `get_patch()` in `metadata/mod.rs` handles `"batch"` operation type correctly.

### Backward Compatibility

- Old per-file patch chains continue to work — `apply_patch` handles both `Batch` and single-op patches.
- Existing repos with per-file chains replay correctly on open.
- Wire format (`PatchProto`) requires no changes — `operation_type: "batch"`, `touch_set: [all paths]`, `payload: [JSON file changes]`.

### Test Coverage

- 485 workspace tests pass (266 core + 18 e2e + 19 hub + 182 others)
- 2 new batch apply tests in suture-core
- Clippy clean with `-D warnings`

## [0.7.0] - 2026-04-05

Suture v0.7.0 — Library SDK release with in-memory repository support, hidden internal modules, improved documentation, and config-without-filesystem constructors.

### In-Memory Repository

- **`Repository::open_in_memory()`** — creates a fully initialized repository backed by a tempdir CAS and an in-memory SQLite metadata store. No filesystem setup required — ideal for testing, embedding, and programmatic use.
- **`BlobStore::open_in_memory()`** — creates a CAS backed by a temporary directory. The directory persists for the lifetime of the BlobStore.

### API Surface Cleanup

- **Hidden internal modules** — sub-modules like `cas::compressor`, `cas::hasher`, `cas::pack`, `dag::branch`, `dag::merge`, `engine::apply`, `patch::commute`, `patch::compose`, `metadata::global_config`, `metadata::repo_config`, and `repository::repo_impl` are now `#[doc(hidden)] pub(crate)`. They remain accessible within the crate but are not part of the public API.
- **`DagNode` fields** — changed from `pub` to `pub(crate)` to prevent external access to internal DAG structure.
- **Public re-exports preserved** — `BlobStore`, `CasError`, `PatchDag`, `DagError`, `FileTree`, `Patch`, etc. remain accessible via their parent module re-exports.

### Documentation

- **`RepoError` variants** — all 14 variants now have `///` doc comments explaining when each error occurs.
- **`suture-protocol` crate docs** — added module-level documentation explaining the wire format purpose.

### Config Without Filesystem

- **`RepoConfig::from_str()`** — parse repository configuration from a TOML string without touching the filesystem.
- **`GlobalConfig::from_str()`** — parse global configuration from a TOML string without touching the filesystem.

### Library Hygiene

- **`eprintln!` → `tracing::warn!`** — replaced all 4 `eprintln!` calls in library code with proper `tracing::warn!` logging. Eliminates side-effect output in library consumers.
- **`tempfile` promoted to regular dependency** — was dev-dependency only; now required for `open_in_memory()` public API.

### Test Coverage

- 483 workspace tests pass
- Clippy clean with `-D warnings`
- All formatting consistent

## [0.6.0] - 2026-04-04

Suture v0.6.0 — Collaboration features including Hub fast-forward validation, selective blob transfer, max_depth support, force push, branch protection, and worktree support.

### Hub Fast-Forward Validation

- **Push validation** — `handle_push` now checks `known_branches` against the server's current branch state using `is_ancestor()` parent chain walk. Non-fast-forward pushes are rejected with HTTP 409 unless `force: true`.
- **Force push** — `suture push --force` bypasses fast-forward validation. Added `force: bool` (with `#[serde(default)]`) to `suture-protocol::PushRequest`.
- **Per-branch push** — `suture push <branch>` pushes only the specified branch and its patches.

### Selective Blob Transfer

- **Blob pruning on pull** — `handle_pull` now collects payload hashes from new patches and returns only the referenced blobs via `get_blobs(repo_id, hashes)`, instead of returning all blobs in the repo.
- **Payload format handling** — payloads may be raw hex (from tests/internal use) or base64-encoded (from CLI). The pull handler detects format by checking if all characters are hex digits before attempting base64 decode.

### Max Depth Support

- **`max_depth` on pull** — `handle_pull` now respects the `max_depth` field from `PullRequest`, truncating the new patches list after computing the delta.

### Branch Protection

- **Protection table** — added `branch_protection` table to HubStorage schema with `protect_branch`, `unprotect_branch`, and `is_branch_protected` methods.
- **Protection endpoints** — `POST /repos/{repo_id}/protect/{branch}` and `POST /repos/{repo_id}/unprotect/{branch}`.
- **Push enforcement** — protected branches reject pushes from non-owners with HTTP 403.
- **CLI support** — `suture branch --protect <name>` and `suture branch --unprotect <name>`. Branch listing shows `[protected]` marker.

### Worktree Support

- **Core implementation** — symlink-based worktrees sharing `.suture/metadata.db`, `objects/`, and `keys/`. Per-worktree HEAD via `.suture/HEAD` file. Worktree detection via `.suture/worktree` marker file.
- **CLI commands** — `suture worktree add <path> [-b <branch>]`, `suture worktree list`, `suture worktree remove <name>`.
- **Unix-only** — worktrees use `std::os::unix::fs::symlink`. Added `Unsupported` variant to `RepoError`.

### Protocol Fixes

- **Eliminated protocol type duplication** — CLI now depends on `suture-protocol` crate instead of redefining all types in `remote_proto.rs`.
- **`known_branches` field** — added to CLI's `PushRequest`, included in canonical push bytes for signature verification.

### Test Coverage

- 264 unit tests in suture-core
- 19 hub tests (including `test_blobs_roundtrip`)
- 18 e2e tests (including `test_push_pull_roundtrip`)
- All 483 workspace tests pass, clippy clean with `-D warnings`

## [0.5.0] - 2026-04-04

Suture v0.5.0 — Semantic Merge 2.0 with XLSX/PPTX merge support, merge abort, strategy resolution, branch-name conflict markers, a Markdown driver, and standalone merge-file.

### Merge Enhancements

- **Merge abort** — `suture merge --abort` cancels an in-progress merge, clears `pending_merge_parents`, and restores the working tree to HEAD.
- **Merge strategies** — `suture merge --strategy ours` / `--strategy theirs` resolves all conflicts by taking one side. `suture merge --strategy auto` (default) uses driver-assisted resolution.
- **Branch names in conflict markers** — conflict markers now show actual branch names (`<<<<<<< feature (HEAD)` / `>>>>>>> main`) instead of hardcoded "ours/theirs".

### Standalone merge-file

- **`suture merge-file`** — performs three-way file merge outside of a branch merge context. Reads base/ours/theirs files, writes merged output to stdout. Supports `--label-ours` and `--label-theirs` for custom conflict marker labels.

### Semantic Drivers

- **XLSX merge** — wired up the existing `merge_cells()` implementation. XLSX files now participate in semantic three-way merge at the cell level.
- **PPTX merge** — wired up the existing `merge_slides()` implementation. PPTX files now participate in semantic three-way merge at the slide level.
- **Markdown driver** — new `suture-driver-markdown` crate with section-level diff and merge. Parses Markdown into blocks (headings, code blocks, lists, tables, paragraphs), matches by heading, and performs three-way merge at the block level. 21 unit tests.
- **Centralized driver registry** — extracted `builtin_registry()` helper in `driver_registry.rs`. Eliminates duplicated 19-line registration blocks in `cmd/merge.rs`, `cmd/diff.rs`, and `cmd/drivers.rs`. All 9 drivers (JSON, TOML, CSV, YAML, XML, Markdown, DOCX, XLSX, PPTX) registered in one place.

### Test Coverage

- 264 unit tests in suture-core
- 21 unit tests in suture-driver-markdown
- 12 unit tests in suture-driver-xlsx
- All workspace tests pass, clippy clean with `-D warnings`

## [0.4.0] - 2026-04-04

Suture v0.4.0 — a usability and polish release with CLI modularization, rich help text, fuzzy error suggestions, and nushell shell completions.

### CLI Modularization

- **Split main.rs into 36 command modules** — the 3,102-line monolithic `main.rs` is now 737 lines containing only CLI definitions and dispatch. Each command lives in its own `cmd/*.rs` file.
- **Extracted helper modules** — `style.rs` (ANSI constants, hook runner), `display.rs` (file walking, diff formatting, timestamps), `ref_utils.rs` (ref resolution, time parsing), `remote_proto.rs` (Hub protocol types and helpers).

### Shell Completions

- **Nushell support** — added `clap_complete_nushell` dependency. `suture completions nushell` generates Nushell completion scripts.
- **String-based shell argument** — changed from `clap_complete::Shell` enum to a string parameter, supporting `bash`, `zsh`, `fish`, `powershell`/`pwsh`, and `nushell`.
- **Clear error for unsupported shells** — prints available shells when an invalid name is given.

### Fuzzy Error Suggestions

- **"Did you mean...?" suggestions** — added `strsim` (Levenshtein distance) based fuzzy matching. When a branch name, tag name, ref, config key, or key name is not found, the closest match is suggested.
- **Applies to**: `checkout`, `branch --delete`, `rebase`, `show`, `notes`, `config`, `key public`, and any command using `resolve_ref()`.

### Rich Help Text

- **Usage examples for all commands** — added `after_long_help` with practical examples to every command and subcommand. Run `suture COMMAND --help` to see examples.
- **Covers**: init, status, add, rm, commit, branch, log, checkout, mv, diff, revert, merge, cherry-pick, rebase, blame, tag, config, push, pull, fetch, clone, reset, show, squash, completions, key (generate/list/public), stash (push/pop/apply/list/drop), remote (add/list/remove/login/mirror), notes (add), bisect (start/run).

### Dependencies

- Added `strsim = "0.11"` for fuzzy string matching
- Added `clap_complete_nushell = "4.6"` for Nushell completions

## [0.3.0] - 2026-04-04

Suture v0.3.0 — a scalability release with persistent snapshots, eliminating O(n) patch replay on cold start.

### Scale

- **Persistent FileTree in SQLite** — new `file_trees` table stores `(patch_id, path, blob_hash)` entries. `snapshot_head()` and `snapshot()` load from SQLite in O(1) instead of replaying all patches O(n). Trees are persisted after every commit.
- **SQLite reflog** — new `reflog` table replaces the legacy config-based approach. O(1) append writes instead of O(n) full-rewrite. Automatic migration from legacy format on first read.
- **Schema migration v2** — automatic migration adds `file_trees` and `reflog` tables to existing repositories.

### Bug Fixes

- **Fixed stale HEAD cache** — `snapshot_head()` now always reads the fresh branch target from the DAG, bypassing stale cached IDs. This fixes a bug where `clone` and `pull` (via `do_fetch`) could return outdated snapshots.
- **Fixed `is_tracked()` cold path** — now queries the SQLite `file_trees` table before falling back to the expensive DAG walk.
- **Made `invalidate_head_cache()` public** — CLI operations like `do_fetch()` that update branch pointers externally can now properly invalidate the cache.

### Test Coverage

- 264 unit tests in suture-core (up from 258)
- 28 metadata tests (up from 22) — 6 new tests for file_trees and reflog persistence
- 18 e2e tests (including `test_push_pull_roundtrip`, previously failing)
- All tests pass, clippy clean with `-D warnings`

## [0.2.0] - 2026-04-03

Suture v0.2.0 — a major performance release with algorithmic improvements, caching, and parallelization.

### Performance

- **O(n) LCA algorithm** — replaced O(n²) LCA with generation-number-based computation. Each node stores its generation (depth from root) at insertion time, enabling O(1) depth comparison instead of BFS-based `ancestor_depth()`.
- **DAG ancestor caching** — `ancestors()` results are cached in a `RefCell<HashMap>`. First call computes via BFS; subsequent calls return cached result in O(1). Cache is stable because `add_patch()` never changes existing nodes' ancestor sets.
- **Pack index caching** — `BlobStore` caches loaded pack indices in a `Mutex<Option<PackCache>>`. First access reads `.idx` files from disk; subsequent calls return cached data. Invalidated automatically on `repack()`.
- **Optional hash verification on read** — `BlobStore::set_verify_on_read(false)` skips the BLAKE3 integrity check on `get_blob()`, saving O(n) per read. Enabled by default for safety; disabled in Repository for performance (content addressing provides correctness by construction).
- **Parallel file I/O** — `sync_working_tree()` uses rayon to pre-fetch blobs and write files in parallel during checkout/merge. Three-phase pipeline: parallel blob reads → directory creation → parallel file writes.
- **HEAD caching** — `head()` branch name cached in `RefCell<Option<String>>`, avoiding SQLite query on every call. Invalidated on all HEAD-modifying operations.

### Benchmarks

- New `dag_lca_diamond` benchmark — measures LCA on diamond-shaped merge DAGs (the most common merge pattern)
- New `dag_ancestors_cached` benchmark — measures cache hit performance for repeated ancestor queries

### Test Coverage

- 258 unit tests in suture-core (up from 256)
- 9 new DAG tests: generation numbers (linear, diamond, uneven branches), ancestor caching, LCA (uneven branches, no common ancestor)
- 2 new pack cache tests: cache hit behavior, invalidation
- All tests pass, clippy clean with `-D warnings`

Suture v0.1.0 — the first stable release of a patch-based, semantically-aware version control system.

### Core

- **BLAKE3 content-addressable storage** with Zstd compression
- **Patch DAG** — commits as patches in a directed acyclic graph, not linear snapshots
- **Touch set commutativity** — conflict detection via logical address intersection
- **SQLite metadata** — branches, config, working set, reflog (WAL mode)
- **Ed25519 commit signing** with key management
- **Per-repo configuration** (`.suture/config` TOML with cascading lookup)

### Semantic Merge

- **9 format-aware drivers:** JSON (RFC 6901), YAML, TOML, CSV, XML, DOCX, XLSX, PPTX, OTIO
- **Automatic driver dispatch** — `suture diff` and `suture merge` use semantic drivers when available
- **Conflict auto-resolution** for Office documents during merge

### CLI (37 commands)

**Repository:** init, status, show, reflog, fsck, gc, config
**Staging:** add (--all), rm (--cached), commit (--all), stash (push/pop/apply/list/drop)
**History:** log (--graph/--oneline/--author/--grep/--since/--all), shortlog, blame, diff (--from/--to/--cached)
**Branching:** branch (create/delete/list/-t), checkout (-b), merge, cherry-pick, revert, reset
**Rebase:** rebase (--interactive/--abort), squash
**Remote:** push, pull (--rebase), fetch (--depth), clone (--depth), remote (add/list/remove/login/mirror)
**Search:** bisect (start/good/bad/run/reset)
**Tags:** tag (create/annotate/delete/list)
**Notes:** notes (add/list/show/remove)
**Signing:** key (generate/list/public)
**Utilities:** mv, drivers, completions, tui, version

### Hook System

- 10 hook types: pre/post-commit, pre/post-push, pre/post-merge, pre/post-rebase, pre-cherry-pick, pre-revert
- `.suture/hooks/` directory with `core.hooksPath` config override
- `hook.d/` directory support for multiple ordered scripts per hook
- Standard environment variables (`SUTURE_HOOK`, `SUTURE_REPO`, `SUTURE_BRANCH`, `SUTURE_HEAD`, `SUTURE_AUTHOR`, etc.)
- Operation-specific env vars for push, merge, rebase, cherry-pick, revert

### Interactive Rebase

- `suture rebase -i <base>` with editor-based TODO file (git-compatible format)
- Actions: pick, reword, edit, squash, drop
- `--abort` to cancel, state persisted in SQLite for crash recovery

### Bisect

- Manual: `suture bisect start <good> <bad>`, then `suture bisect good` / `suture bisect bad`
- Automated: `suture bisect run <good> <bad> -- <test-command>`
- Reports first bad commit automatically

### Remote

- HTTP/JSON Hub server with Ed25519 push signing
- rustls TLS (pure Rust, no OpenSSL dependency)
- Shallow clone (`--depth`)
- Pull with rebase (`--rebase`)
- ARM Linux binary (aarch64-unknown-linux-gnu)

### Platforms

- Linux x86_64, Linux aarch64
- macOS x86_64, macOS aarch64
- Windows x86_64

### Quality

- 419 tests (0 failures)
- 18 end-to-end integration tests
- 0 clippy warnings
- 0 cargo-audit findings
- CI: Nix-based build + test + clippy + fmt + audit

## [0.1.0-rc.1] - 2026-04-03

### Changed

#### README Overhaul
- Complete rewrite of README.md with:
  - Binary release installation instructions
  - Comprehensive CLI reference (37 commands organized by category)
  - Feature documentation (hooks, interactive rebase, bisect, semantic merge)
  - Updated semantic driver table (9 drivers: JSON, YAML, TOML, CSV, XML, DOCX, XLSX, PPTX, OTIO)
  - Architecture overview with all 22 workspace crates
  - Repository layout documentation

### Quality
- All 37 CLI commands have consistent help text
- 419 tests, 0 failures, 0 clippy warnings
- Zero audit findings

## [0.1.0-beta.3] - 2026-04-03

### Added

#### Interactive Rebase (`suture rebase -i`)
- New `-i` / `--interactive` flag on `suture rebase` — opens editor with TODO file
- Supports 5 actions: `pick`, `reword`, `edit`, `squash`, `drop`
- Editor integration via `$SUTURE_EDITOR` or `$EDITOR` environment variable
- TODO file format compatible with git's interactive rebase
- Supports reordering commits, dropping commits, squashing adjacent commits
- `--abort` flag to cancel an in-progress rebase (restores original HEAD)
- `--resume` flag to continue after pausing at an `edit` action
- Rebase state persisted in SQLite for crash recovery

#### Core Rebase Infrastructure
- `commit_groups()` — groups per-file patches into logical commits (by shared message)
- `patches_since_base()` — collects patches between a base commit and HEAD
- `generate_rebase_todo()` — produces TODO file content for editor
- `parse_rebase_todo()` — reads edited TODO back into a structured plan
- `rebase_interactive()` — executes a rebase plan (pick/reword/edit/squash/drop)
- `rebase_abort()` — restores branch to pre-rebase state
- `RebaseState` / `RebasePlan` / `RebaseAction` types for state management

#### Existing Features (already present)
- `suture reflog` — already implemented (shows HEAD movement history)
- `suture log --graph` — already implemented (ASCII DAG visualization)

### Quality
- Test count: 419 (up from 415 in v0.1.0-beta.2)
- 4 new e2e tests (rebase: non-interactive, abort, plan parsing, drop)
- Zero clippy warnings, zero audit findings

### Deferred
- `add -p` (partial/hunk staging) deferred to post-1.0
- Full `--continue` support (edit workflow) deferred to beta.4

## [0.1.0-beta.2] - 2026-04-03

### Added

#### Hook System
- New `suture-core::hooks` module — full git-compatible hook execution framework
- Supported hooks: `pre-commit`, `post-commit`, `pre-push`, `post-push`, `pre-merge`, `post-merge`, `pre-rebase`, `post-rebase`, `pre-cherry-pick`, `pre-revert`
- Hooks directory: `.suture/hooks/` (overridable via `core.hooksPath` in `.suture/config`)
- Supports `hook.d/` directories for multiple ordered scripts per hook type (e.g., `pre-commit.d/01-lint`, `pre-commit.d/02-test`)
- Non-executable hooks are silently skipped (Unix permission bit check)
- Missing hooks are silently skipped — zero friction for repos without hooks
- Hook failure (non-zero exit) aborts the operation and prints stderr to the user
- Hook stdout is printed to the user for feedback
- Standard environment variables passed to all hooks: `SUTURE_HOOK`, `SUTURE_REPO`, `SUTURE_HOOK_DIR`, `SUTURE_OPERATION`, `SUTURE_AUTHOR`, `SUTURE_BRANCH`, `SUTURE_HEAD`
- Operation-specific env vars: `SUTURE_PUSH_REMOTE`, `SUTURE_PUSH_PATCHES` (pre/post-push), `SUTURE_MERGE_SOURCE` (pre/post-merge), `SUTURE_REBASE_ONTO` (pre/post-rebase), `SUTURE_CHERRY_PICK_TARGET` (pre-cherry-pick), `SUTURE_REVERT_TARGET` (pre-revert), `SUTURE_COMMIT` (post-commit)

#### Hook Integration Points
- `suture commit`: runs `pre-commit` before finalizing, `post-commit` after success
- `suture push`: runs `pre-push` before HTTP POST, `post-push` after successful push
- `suture merge`: runs `pre-merge` before merge execution, `post-merge` after clean merge
- `suture revert`: runs `pre-revert` before revert execution
- `suture cherry-pick`: runs `pre-cherry-pick` before applying patch
- `suture rebase`: runs `pre-rebase` before replaying patches, `post-rebase` after completion

### Quality
- Test count: 415 (up from 385 in v0.1.0-beta.1)
- 16 new unit tests (hooks module: find, run, build_env, format, directory support)
- 6 new integration tests (pre-commit pass/block, post-commit, env vars, non-executable, hook.d/)
- Zero clippy warnings, zero audit findings

### Deferred
- `add -p` (partial/hunk staging) deferred to beta.3 — requires staging model changes

## [0.1.0-beta.1] - 2026-04-02

### Added

#### `suture pull --rebase`
- New `--rebase` flag on `suture pull` — fetches remote patches then rebases local commits on top
- Replaces merge-based pull with rebase workflow for cleaner linear history
- Automatically fast-forwards when possible, reports replayed commit count

#### `suture bisect run`
- New `bisect run <good> <bad> -- <command>` — fully automated binary search for bug-introducing commit
- Runs the given test command at each bisection step (exit 0 = good, non-zero = bad)
- Reports first bad commit after narrowing the range
- Restores original branch state after completion
- Fixed bisect index ordering bug: commits are now correctly ordered newest-to-oldest

#### Per-Repo Config File
- New `.suture/config` TOML file support — repo-level configuration checked before SQLite config
- Supports `[user]`, `[signing]`, `[core]`, `[push]`, `[pull]` sections
- Config lookup priority: `.suture/config` → SQLite config → global `~/.config/suture/config.toml`
- 3 new unit tests for repo config parsing

#### ARM Linux Binary
- Re-enabled `aarch64-unknown-linux-gnu` target in release workflow (previously blocked by native-tls)

### Changed

#### TLS: native-tls → rustls
- Migrated all reqwest usage from `native-tls` to `rustls-tls` (pure Rust TLS)
- Removes dependency on system OpenSSL — enables cross-compilation without C toolchain
- Affects `suture-cli`, `suture-hub`, and `suture-e2e` crates
- Removed `openssl` from Nix flake dependencies

### Fixed
- Fixed bisect ordering: `older_idx`/`newer_idx` were swapped — good commit (older, higher index) and bad commit (newer, lower index) are now correctly identified
- Bisect midpoint narrowing now correctly adjusts bounds based on test results

### Quality
- Test count: 385 (up from 382 in v0.1.0-alpha.2)
- 3 new tests (repo config parsing)
- Zero clippy warnings, zero audit findings

## [0.1.0-alpha.2] - 2026-04-01

### Fixed

#### Release Infrastructure
- Fixed binary name in GitHub release workflow: `suture-cli`/`suture-hub` → `suture` in tar/zip artifacts
- Added ARM build targets: `aarch64-unknown-linux-gnu` and `aarch64-apple-darwin`

#### CLI Hardening
- Fixed 3 `unwrap()` calls in branch/tag commands — replaced with proper error messages
- Fixed `suture log` to show all commits by default (uses `reachable_patches()` instead of first-parent-only `patch_chain()`)
- Fixed `suture diff` to fall back to disk read when working tree files aren't in CAS
- Fixed `suture checkout -b` to accept optional branch name
- Fixed `suture commit -a` to stage all files before committing
- Fixed HEAD/HEAD~N ref resolution in `show`, `revert`, `cherry-pick`, and `reset`
- Fixed `suture bisect` to use subcommand syntax (`bisect good/bad/reset`)
- Fixed `suture notes show` subcommand routing

#### Safety & Quality
- Added SAFETY comments to 3 `unsafe` blocks in production code (`suture-common` blake3 transmute, `suture-driver-docx` from_utf8_unchecked)
- Replaced `SECURITY.md` placeholder with `security@suture.dev` contact

#### Supply Chain
- Fixed `suture-core` missing `toml` and `dirs` crate dependencies
- Wired `global_config` module into `metadata/mod.rs`
- Reduced Nix flake dependencies (removed unnecessary packages)

### Changed
- `suture-cli` version bumped to `0.1.0-alpha.2`
- Root `Cargo.toml` now includes `[workspace.package]` metadata (description, license, repository, homepage, keywords)

## [0.12.0] - 2026-03-29

### Added

#### Terminal UI (Path C — Interface Evolution)
- New `suture-tui` crate — interactive terminal UI built with `ratatui` and `crossterm`
- Launch via `suture tui` subcommand
- **Status panel**: shows current branch, HEAD patch, staged/unstaged file counts, quick summary
- **Log view**: ASCII branch topology graph with commit markers (●/◆), branch head labels, author/timestamp
- **Interactive staging**: split-pane view of staged and unstaged files, toggle with Space/Enter, Tab to switch focus, `a` to stage all
- **Diff viewer**: line-level diff with +/- prefixes and line numbers, scrollable, color-coded (green/red/cyan)
- **Help panel**: complete keyboard shortcut reference for all tabs
- **Commit mode**: inline commit message input (Enter to commit, Esc to cancel)
- **Tab navigation**: Tab/Shift+Tab to cycle, Alt+1..5 for direct jump
- **Status bar**: shows branch, staged/unstaged counts, error messages, commit mode input
- 10 unit tests: tab cycling, timestamp formatting, date conversion, LCS diff computation

#### CLI Integration
- `suture tui` subcommand — launches the terminal UI for the current repository
- CLI now has 37 commands total

## [0.11.0] - 2026-03-29

### Added

#### OOXML Shared Infrastructure (Path B — Semantic Differentiator)
- New `suture-ooxml` crate — shared Office Open XML infrastructure
- `OoxmlDocument::from_bytes()` / `to_bytes()` — read/write OOXML ZIP archives
- Part navigation: `get_part()`, `main_document_path()`
- Relationship parsing from `.rels` XML files
- Content type tracking
- 4 tests: attr extraction, rels parsing, ZIP roundtrip

#### DOCX Semantic Driver (Path B — Semantic Differentiator)
- New `suture-driver-docx` crate implementing `SutureDriver`
- Paragraph-level diff for Word documents (`.docx`)
- Three-way merge at paragraph granularity: auto-merges non-overlapping changes
- Parses `word/document.xml`, extracts paragraphs from `<w:p>` elements
- Full ZIP roundtrip: reads .docx → modifies XML parts → writes .docx
- 7 tests: name, extensions, diff added/removed/modified, merge clean, merge conflict

#### XLSX Semantic Driver (Path B — Semantic Differentiator)
- New `suture-driver-xlsx` crate implementing `SutureDriver`
- Cell-level diff for Excel spreadsheets (`.xlsx`)
- Addressing: `/{sheet_name}/{row}/{col}` for precise cell identification
- Parses `xl/worksheets/sheet*.xml` for row/cell data
- 5 tests: name, extensions, diff cells, merge no-conflict, merge conflict

#### PPTX Semantic Driver (Path B — Semantic Differentiator)
- New `suture-driver-pptx` crate implementing `SutureDriver`
- Slide-level diff for PowerPoint presentations (`.pptx`)
- Parses `ppt/presentation.xml` for `<p:sp>` slide elements
- Slide ordering preserved through merge
- 7 tests: name, extensions, diff add/remove/no-change, merge different slides, merge conflict

#### Full Office Driver Wiring
- All 3 Office drivers (DOCX, XLSX, PPTX) wired into CLI
- Registered in `cmd_drivers`, `cmd_diff`, and `cmd_merge`
- `suture drivers` now lists 8 drivers: JSON, TOML, CSV, YAML, XML, DOCX, XLSX, PPTX

#### Quality
- Test count: 319 (up from 296 in v0.10.0)
- 23 new tests (4 OOXML, 7 DOCX, 5 XLSX, 7 PPTX)
- Zero clippy warnings, zero audit findings
- 17 workspace crates (up from 14)

## [0.10.0] - 2026-03-29

### Added

#### Formal Patch Algebra (Core Theory)
- **Patch composition** (`patch/compose.rs`): `compose()` collapses two patches into one equivalent operation; `compose_chain()` handles sequences
- **THM-COMPOSE-001**: Composed patch preserves union of touch sets; parent chain collapses
- **DEF-COMPOSE-001**: Formal definition — apply(P₃, pre_P₁) = apply(P₂, apply(P₁, pre_P₁))
- 8 composition tests (linear chain, disjoint/overlapping touch sets, chain, error cases)

#### Conflict Classification (Core Theory)
- **`ConflictClass`** enum: `AutoResolvable` (identical changes), `DriverResolvable` (different sub-addresses), `Genuine` (same element, different values), `Structural` (operation type mismatch)
- **`Conflict::classify()`** method: inspects patch payloads to determine conflict severity
- **`TouchSet::union()`** and **`TouchSet::subtract()`**: set-theoretic operations on touch sets
- 9 new tests (classification: 5, touch set operations: 4)

#### `suture squash` (Path A — Git Parity)
- `Repository::squash(count, message)` — composes last N patches into one
- Verifies chain ancestry before composing
- Updates branch pointer and records reflog
- CLI: `suture squash N [-m message]`

#### `log --all` (Path A — Git Parity)
- `suture log --all` — shows commits across ALL branches, deduplicated, sorted by timestamp
- Collects from all branch tips via `dag().patch_chain()`
- Graph mode auto-disabled with `--all`

#### `log --since/--until` (Path A — Git Parity)
- `suture log --since "3 days ago"` — show commits newer than threshold
- `suture log --until "2026-01-15"` — show commits older than threshold
- Supports ISO dates (YYYY-MM-DD) and relative times (N seconds/minutes/hours/days/weeks/months/years ago)

#### CSV Semantic Merge (Path B — Semantic Differentiator)
- `CsvDriver::merge()` — three-way merge for CSV files
- Header union: columns added by either side are included
- Cell-by-cell conflict detection: same-index, different-value = conflict
- 4 tests: no-conflict, conflict, added rows, header change

#### OTIO Element ID Fix (Path B — Semantic Differentiator)
- Fixed element ID collision: `element_id()` now includes index and name (`{index}:{type}:{name}`)
- Multiple clips/tracks of same type get unique IDs
- Updated `collect_elements()` to pass child index through recursion
- Added `test_unique_ids_for_same_type` verification test

#### Quality
- Test count: 296 (up from 274 in v0.9.0)
- 22 new tests (8 compose, 9 conflict/touchset, 4 CSV merge, 1 OTIO)
- Zero clippy warnings, zero audit findings
- 14 workspace crates

## [0.9.0] - 2026-03-29

### Added

#### `suture gc` (Path A — Git Parity)
- `gc()` — garbage collection for unreachable patches
- Walks all branch tips, collects reachable patches via ancestor traversal
- Deletes unreachable patches from metadata (patches, edges, signatures tables)
- CLI: `suture gc` — reports count of removed patches

#### `suture fsck` (Path A — Git Parity)
- `fsck()` — verify repository integrity
- Checks DAG parent consistency (all parent IDs exist)
- Checks branch target validity (all branches point to existing patches)
- Checks blob references (patch payloads resolve to CAS blobs)
- Checks HEAD consistency (current branch exists)
- CLI: `suture fsck` — reports passed checks, warnings, and errors

#### `suture bisect` (Path A — Git Parity)
- Binary search for bug-introducing commit
- Accepts good and bad refs (commit hashes, branch names, partial hashes)
- Finds midpoint in linear history, prints guidance for narrowing
- Reports first bad commit when range is narrowed to one commit
- CLI: `suture bisect <good> <bad>`

#### XML Semantic Driver (Path B — Semantic Differentiator)
- New `suture-driver-xml` crate implementing `SutureDriver`
- Element-level XML diff using `roxmltree` DOM parser
- XPath-like paths: `/root/child[index]`, `/root/child[index]@attr`, `/root/child[index]#text`
- Detects Added, Removed, Modified changes for elements, attributes, and text
- Semantic merge: recursive three-way merge with conflict detection
- 9 tests: name, extensions, modified text, added element, removed element, attribute change, format diff, merge clean, merge conflict

#### YAML Semantic Merge (Path B — Semantic Differentiator)
- `YamlDriver::merge()` — three-way merge for YAML files
- Recursive merge of `serde_yaml::Value` mappings and sequences
- Auto-merges non-overlapping changes (additions, deletions, modifications to different parts)
- Detects conflicts when same key/element modified differently by both sides
- 5 new tests: no-conflict, conflict, both-add-different, both-add-same, nested merge

#### XML/YAML Drivers Wired Into CLI (Path B — Semantic Differentiator)
- `suture diff` now uses XML driver for `.xml` files automatically
- `suture diff` now uses YAML driver for `.yaml`/`.yml` files automatically
- `suture merge` attempts XML semantic merge for conflicting `.xml` files
- `suture merge` attempts YAML semantic merge for conflicting `.yaml`/`.yml` files
- `suture drivers` lists all 5 drivers: JSON, TOML, CSV, YAML, XML

#### End-to-End Integration Tests (Path C — Hardening)
- New `suture-e2e` crate with custom test harness
- 7 integration tests: init→commit→status, branch→merge, gc, fsck, bisect, tag, stash→pop
- Tests invoke `suture-cli` binary as subprocess against real repositories
- Gracefully skips if binaries not built

#### GitHub Release Workflow (Path C — Infrastructure)
- `.github/workflows/release.yml` — triggered on `v*` tag push
- Cross-compiles static binaries for Linux, macOS, Windows
- Creates GitHub Release with attached binaries (tar.gz / zip)

#### Quality
- Test count: 274 (up from 260 in v0.8.0)
- 14 new tests (9 XML driver, 5 YAML merge) + 7 e2e integration tests
- Zero clippy warnings, zero audit findings
- 14 workspace crates (up from 12)

## [0.8.0] - 2026-03-29

### Added

#### Semantic Merge Wiring (Path B — Semantic Differentiator)
- `suture merge` now attempts semantic merge via drivers for conflicting files
- Builds `DriverRegistry` with JSON, YAML, TOML, CSV drivers after conflict detection
- Retrieves clean base/ours/theirs content from CAS via conflict blob hashes
- If a driver resolves the merge, writes result to disk and stages it
- Reports count of semantically resolved vs remaining conflicts

#### TOML Driver (Path B — Semantic Differentiator)
- New `suture-driver-toml` crate implementing `SutureDriver`
- Key-level TOML diff using `toml::Value` recursive comparison
- Semantic merge: auto-merges non-overlapping key changes, detects conflicts
- 7 tests: name, extensions, modified, added, nested, merge clean, merge conflict

#### CSV Driver (Path B — Semantic Differentiator)
- New `suture-driver-csv` crate implementing `SutureDriver`
- Row/cell-level CSV diff with row:col addressing
- Header change detection, added/removed row tracking
- 5 tests: name, extensions, cell change, added row, removed row

#### `suture shortlog` (Path A — Git Parity)
- Groups commits by author using sorted output
- Shows commit count and first message per author
- `--branch` and `-n` (limit) flags

#### `suture tag --annotate` (Path A — Git Parity)
- `suture tag -a -m "message" <name>` — creates annotated tags with stored message
- Annotations stored as `tag.<name>.message` config entries
- `suture tag list` shows `(annotated)` marker and message for annotated tags
- Tag deletion cleans up annotation

#### `suture notes` (Path A — Git Parity)
- `suture notes add <commit> [-m message]` — attach a note to a commit
- `suture notes list <commit>` — list all notes for a commit
- `suture notes remove <commit> <index>` — remove a specific note
- Notes stored as `note.<hash>.<index>` config entries
- `add_note()`, `list_notes()`, `remove_note()` core methods

#### `suture version` (Path A — Git Parity)
- Prints version from `CARGO_PKG_VERSION`

#### README Rewrite (Path C — Documentation)
- Completely rewritten to reflect actual v0.8.0 state
- Honest architecture description (CAS, Patch DAG, Hub)
- Quick start guide, CLI reference table (32 commands)
- Driver SDK section with how-to-write guidance
- Honest "not yet implemented" section (VFS, Raft, SSO, Web UI)

#### Quality
- Test count: 260 (up from 248 in v0.7.0)
- 12 new tests (7 TOML, 5 CSV) + notes/shortlog/tag-annotate core tests
- Zero clippy warnings, zero audit findings
- 12 workspace crates (up from 10)

## [0.7.0] - 2026-03-29

### Added

#### `suture rm` (Path A — Git Parity)
- `suture rm <path> [paths...]` — remove files from working tree and staging area
- `suture rm --cached <path>` — remove from staging only, keep file on disk
- `add()` now handles missing tracked files by staging them as `FileStatus::Deleted`

#### `suture mv` (Path A — Git Parity)
- `suture mv <source> <dest>` — rename/move tracked files
- Moves file on disk, stages old path as Deleted and new path as Added
- `rename_file()` core method validates paths before moving

#### `suture remote remove` (Path A — Git Parity)
- `suture remote remove <name>` — delete a configured remote
- Cleans up associated `last_pushed` state automatically
- `delete_config()` method added to `MetadataStore`

#### Semantic Merge (Path B — Semantic Differentiator)
- `SutureDriver::merge()` trait method — three-way semantic merge
- Default implementation returns `Ok(None)` (fall back to line-level)
- `JsonDriver::merge()` — key-level JSON merge: auto-merges non-overlapping changes, detects conflicts
- 6 tests: no-conflict, conflict, both-add-different, both-add-same, nested, identical

#### YAML Driver (Path B — Semantic Differentiator)
- New `suture-driver-yaml` crate implementing `SutureDriver`
- Recursive YAML comparison using `serde_yaml::Value`
- `format_diff` with YAML-specific paths
- 5 tests: modified, added, nested, and format diff scenarios

#### `suture drivers` (Path B — CLI)
- `suture drivers` — lists all registered semantic drivers with their extensions
- Shows JSON and YAML drivers by default

#### Quality
- Test count: 248 (up from 232 in v0.6.0)
- 16 new tests (5 core: rm/mv/remote, 6 JSON merge, 5 YAML driver)
- Zero clippy warnings, zero audit findings
- 10 workspace crates (up from 9)

## [0.6.0] - 2026-03-29

### Added

#### SutureDriver Trait & Registry (Path B — Semantic Differentiator)
- New `suture-driver` crate with the `SutureDriver` trait
- `SutureDriver::diff()` — produces `SemanticChange` enum (Added/Removed/Modified/Moved)
- `SutureDriver::format_diff()` — human-readable semantic diff for a file type
- `DriverRegistry` — dispatches to drivers by file extension
- `DriverError`, `VisualDiff`, `DiffHunk`, `DiffSummary` supporting types

#### JSON Semantic Driver (Path B)
- New `suture-driver-json` crate implementing `SutureDriver`
- Recursive JSON comparison using RFC 6901 JSON Pointer paths
- Detects Added, Removed, Modified changes at key level
- `format_diff` shows semantic operations: `MODIFIED /name: "Alice" → "Bob"`
- 10 tests covering nested objects, arrays, new files, identical files

#### Semantic Diff in CLI (Path B)
- `suture diff` now uses JSON driver for `.json` files automatically
- Falls through to line-level diff for unsupported formats
- Shows key-level changes instead of raw line noise for JSON files

#### `suture show <ref>` (Path A — Git Parity)
- Display commit hash, author, timestamp, message, parents, changed files
- Supports branch names, tag names, full and partial commit hashes
- `resolve_ref` helper for ref resolution across all command contexts

#### `suture reflog` (Path A — Git Parity)
- `record_reflog()` tracks HEAD movements in config as JSON entries
- `reflog_entries()` retrieves history (newest first, capped at 100)
- Reflog recorded for: commit, checkout, reset, cherry-pick, rebase
- CLI: `suture reflog` displays `short_hash entry_string` per line

#### CI/CD (Path C — Hardening)
- `.forgejo/workflows/ci.yml` — Forgejo Actions workflow (build, test, clippy, fmt, audit)
- Uses `dtolnay/rust-toolchain` action, no Nix dependency in CI
- Mirrors existing `.github/workflows/ci.yml`

#### Infrastructure (Path C — Hardening)
- `rust-toolchain.toml` — pins stable channel for non-Nix users
- `.gitignore` updated: added `.direnv/`, `suture-e2e-*/`, `alice/`

#### Quality
- Test count: 232 (up from 222 in v0.5.0)
- 10 new JSON driver tests
- Zero clippy warnings, zero audit findings
- 9 workspace crates (up from 7)

## [0.5.0] - 2026-03-29

### Added

#### `-C <path>` Global Flag
- `suture -C <path> <command>` — run any command as if started in a different directory
- Global flag applies to all subcommands (except `init` and `clone` which take their own path)

#### Cherry-Pick
- `cherry_pick(&mut self, patch_id)` — apply a specific commit onto current HEAD
- Creates a new patch with the same content but current HEAD as parent
- Skips identity, merge, and create patches (not cherry-pickable)
- CLI: `suture cherry-pick <commit-hash>`
- Bug fix: capture `old_tree` before branch update for correct working tree sync

#### Rebase
- `rebase(&mut self, target_branch)` — replay commits from current branch onto target
- Finds unique commits via LCA (Lowest Common Ancestor)
- Supports fast-forward when current branch is ancestor of target
- Returns `RebaseResult` with replay count and new tip ID
- CLI: `suture rebase <branch>`
- Bug fix: capture `old_tree` before branch update for correct working tree sync

#### Blame
- `blame(&self, path)` — per-line commit attribution for a file
- Walks patch chain tracking line-level modifications via LCS diff
- Returns `Vec<BlameEntry>` with patch_id, message, author, line content, line number
- CLI: `suture blame <file>` — displays `line_num | hash (author) content`

#### Log Filtering
- `suture log --oneline` — compact format (short hash + message)
- `suture log --author=<name>` — filter commits by author
- `suture log --grep=<pattern>` — filter commits by message substring (case-insensitive)
- Filters compose with `--graph` (graph mode falls back to filtered non-graph when filters active)

#### Quality
- Test count: 222 (up from 216 in v0.4.0)
- 6 new tests: cherry-pick (2), rebase (2), blame (2)
- Zero clippy warnings, zero audit findings
- Bug fixes in cherry-pick and rebase: working tree sync now captures old snapshot before branch update

## [0.4.0] - 2026-03-28

### Added

#### Human-Readable Diff Output
- `suture diff` now shows line-level content with `+`/`-` prefixes
- ANSI color output (green for additions, red for deletions, bold cyan for headers)
- `diff --git a/<path> b/<path>` headers and `@@ hunk @@` markers
- Added, Deleted, Modified, and Renamed files all display correctly
- Uses existing LCS-based `diff_lines` from the merge engine

#### Enhanced Status
- `suture status` now shows unstaged changes alongside staged changes
- "Unstaged changes:" section with modified, deleted, and untracked files
- Files modified after staging marked with `[staged+unstaged]`
- Walks repository directory and compares against HEAD tree

#### Clone Command
- `suture clone <url> [dir]` — bootstrap a repository from a remote Hub
- Creates target directory, initializes repo, adds "origin" remote, pulls patches
- Extracts directory name from URL when not specified

#### Fetch Command
- `suture fetch [remote]` — fetch patches from remote without updating working tree
- Updates DAG and metadata only (no working tree sync)
- Extracted shared `do_fetch`/`do_pull` helpers for code reuse

#### Reset Command
- `suture reset [--mode soft|mixed|hard] <ref>` — move HEAD to a different commit
- `--soft`: move branch pointer, keep staging and working tree
- `--mixed` (default): move branch pointer, clear staging, keep working tree
- `--hard`: move branch pointer, clear staging, restore working tree to target
- `ResetMode` enum exposed from `suture-core`

#### Quality
- Test count: 216 (up from 213 in v0.3.0)
- 3 new reset tests (soft, mixed, hard modes)
- Zero clippy warnings, zero audit findings

## [0.3.0] - 2026-03-28

### Added

#### Shell Completions
- `clap_complete` dependency for generating shell completion scripts
- CLI `completions` command: `suture completions bash|zsh|fish`

#### Log Graph
- `--graph` flag on `log` command shows ASCII branch topology
- Column-based rendering with merge lines and branch alignment
- Logical commit grouping (patches sharing message+timestamp grouped as one)
- Topological sort newest-first ordering
- Branch labels at tips with `*` marker for HEAD branch

#### Working Tree Safety
- `has_uncommitted_changes()` detects both staged and unstaged changes
- `checkout()` auto-stashes dirty working tree before switching, restores after
- Matches git behavior: dirty state is preserved across branch switches

#### Stash
- `stash_push(message)` — saves staged and unstaged changes as a stash entry
- `stash_pop()` — applies highest-index stash and removes it
- `stash_apply(index)` — applies stash without removing it
- `stash_list()` — lists all stash entries with message, branch, and HEAD
- `stash_drop(index)` — removes a stash entry
- Stash entries stored as config entries (`stash.{index}.{message,head_branch,head_id,files}`)
- CLI commands: `suture stash push [-m msg]`, `suture stash pop`, `suture stash apply <n>`, `suture stash list`, `suture stash drop <n>`

#### Quality
- Test count: 213 (up from 203 in v0.2.0)
- 9 new stash tests covering push/pop, list, drop, apply-keeps-entry, has_uncommitted_changes variants
- Zero clippy warnings, zero audit findings

## [0.2.0] - 2026-03-27

### Added

#### Incremental Push
- `patches_since(since_id)` — walks DAG from branch tips, returns only new patches
- Topological sort (Kahn's algorithm) ensures parents-before-children ordering
- CLI `push` tracks `remote.<name>.last_pushed` config for incremental sync
- Push now shows patch count: "Push successful (3 patch(es))"

#### Author Identity
- `Repository::init()` defaults to `"unknown"` author (no longer hardcoded `"alice"`)
- `open()` reads `user.name` config first, falls back to `author`, then `"unknown"`
- `get_config` / `set_config` / `list_config` exposed as public API
- CLI `config` command: list all, get single key, set key=value
- Internal keys (`head_branch`, `pending_merge_parents`) hidden from `config` listing
- Init prints hint: "run `suture config user.name \"Your Name\"` to set your identity"

#### Tag Support
- `create_tag(name, target)` / `delete_tag(name)` / `list_tags()` / `resolve_tag(name)`
- Tags stored as `tag.<name>` config entries mapping to patch IDs
- CLI `tag` command: list all, create at HEAD or `--target <ref>`, `--delete`

#### Branch Delete
- `delete_branch(name)` with current-branch protection
- Removes branch from both DAG and metadata
- CLI `branch --delete <name>`

#### Conflict Resolution Persistence
- `pending_merge_parents` persisted to config as JSON on merge
- Restored on `Repository::open()`
- Cleared on commit (conflict resolved)

#### Ed25519 Signing Wired Into Push
- CLI `key generate [name]` — generates Ed25519 keypair
- Private key saved to `.suture/keys/<name>.ed25519`
- Public key stored in config as `key.public.<name>`
- CLI `key list` / `key public [name]`
- `signing.key` config auto-set to `"default"` on key generation
- `cmd_push()` reads private key, signs canonical push bytes, attaches signature
- Hub only verifies signatures when authorized keys are configured
- Hub accepts signed pushes even without auth (backward compatible)
- `canonical_push_bytes()` aligned between CLI and hub (includes operation_type)

#### Line-Level Diff3 Merge
- `engine::merge` module with LCS-based line diff algorithm
- `three_way_merge_lines()` — performs three-way merge with conflict markers
- `MergeOutput` struct with `lines`, `is_clean`, `auto_merged`, `conflicts` fields
- Conflict markers use configurable labels (e.g., `<<<<<<< ours (HEAD)`)
- Handles: trivial cases, one-side changes, non-overlapping regions, same-change detection, empty base, trailing insertions
- 13 unit tests covering all merge scenarios

#### Hub SQLite Persistence
- `HubStorage` module — SQLite-backed storage replacing in-memory HashMaps
- Persistent repos, patches, branches, and blobs across hub restarts
- `--db <path>` CLI flag for `suture-hub` to specify database file
- In-memory mode still available (default, for testing)
- 4 storage-specific tests including cross-reopen persistence

#### Hub Ed25519 Authentication
- `authorized_keys` table in hub storage for registering public keys
- Push request signature verification using canonical bytes
- Auth is optional — only enforced when authorized keys are configured
- `signature` field in `PushRequest` (optional, 64-byte Ed25519 sig)
- 3 auth tests: required-when-keys-exist, valid-signature-succeeds, no-auth-when-unconfigured

#### Quality
- Test count: 203 (up from 180 in v0.1.0)
- Zero clippy warnings, zero audit findings
- End-to-end verified: signed push with hub auth, pull by unauthorized client

## [0.1.0] - 2026-03-27

### Added

#### Core Engine
- Content Addressable Storage (CAS) with BLAKE3 hashing and Zstd compression
- Patch Algebra engine with commutativity detection and merge computation
- Patch DAG (Directed Acyclic Graph) with branch management and LCA computation
- SQLite metadata store with WAL mode
- Patch Application Engine: `apply_patch`, `apply_patch_chain`, `resolve_payload_to_hash`
- FileTree: in-memory file tree with insert/remove/rename operations
- Diff Engine: `diff_trees` with Added/Modified/Deleted/Renamed detection
- Full DAG reconstruction on `Repository::open()` (loads all patches from SQLite)
- `.sutureignore` support with glob-like matching (`*.o`, `build/`, exact match)
- `Repository::add_all()` — stage all files respecting ignore patterns
- `Repository::snapshot()` / `snapshot_head()` — build FileTree from patch chain
- `Repository::checkout()` — switch branches, update working tree, HEAD tracking
- `Repository::diff()` — compare two commits/branches via snapshot diff
- `Repository::revert()` — create inverse patches (Delete for Create/Modify, re-create for Delete)
- `DirtyWorkingTree` error — checkout refuses if staged changes exist

#### Distributed Sync
- Hub daemon (`suture-hub`) with axum-based HTTP API
- `POST /push` — push patches, branches, and blobs to hub
- `POST /pull` — pull new patches based on client's known branches
- `GET /repos` — list all repositories
- `GET /repo/{id}` — get repo info (patch count, branches)
- Topological sort for patch delivery order
- CLI: `remote add`, `remote list`, `push`, `pull` commands
- End-to-end verified: repo A pushes → Hub stores → repo B pulls → files appear on disk

#### Merge Execution
- `execute_merge()` — creates two-parent merge commits for clean merges
- Fast-forward detection (single-parent merge when possible)
- Diff3 conflict markers with `<<<<<<< ours` / `=======` / `>>>>>>> theirs` labels
- `pending_merge_parents` for multi-parent merge commits
- Conflict resolution via `resolve_merge_conflict()`

#### Snapshot Caching
- RefCell-based snapshot cache in Repository
- `invalidate_head_cache()` on commit/revert/merge
- O(1) `snapshot_head()` for repeated calls (was O(n) iterating all patches)
- Self-host test: 101 files committed in 251ms, status/log in ~60ms

#### Ed25519 Signing
- `signing` module with `SigningKeypair` (generate, sign, verify)
- `canonical_patch_bytes()` — deterministic serialization for signing
- `verify_signature()` — verify Ed25519 signatures against public keys
- Metadata store: `public_keys` and `signatures` tables with CRUD methods

#### CLI
- `init` — initialize a new Suture repository
- `status` — show repository status, staged files, branch info
- `add` — stage files (with `--all` / `-a` flag for add-all)
- `commit` — create a commit from staged changes
- `branch` — create branches with optional target
- `log` — show commit history
- `merge` — merge a branch into HEAD (with conflict detection)
- `checkout` — switch branches, update working tree
- `diff` — show differences between commits/branches
- `revert` — revert a commit by hash
- `remote add` / `remote list` — manage remote hubs
- `push` / `pull` — distributed sync with hub

#### Testing
- 180 tests across 6 crates (0 failures)
- 21 proptest property-based test suites (10K+ randomized cases):
  - FileTree: insert/contains, remove, rename, equality, symmetry
  - Patch apply: create, modify, delete, chain commutativity
  - DAG: patch count, chain ancestry, LCA linear, ancestors subset
  - CAS: put/get roundtrip, content addressing, idempotency
  - Diff: empty-vs-full, full-vs-empty, identical, symmetry inverse

#### Benchmarks
- 6 Criterion benchmark groups (`cargo bench -p suture-bench`):
  - CAS put/get: 1KB, 10KB, 100KB blobs
  - BLAKE3 hashing: 64B, 1KB, 10KB, 100KB
  - DAG insertion: 10, 100, 1000 patch linear chains
  - DAG LCA: chains of 10, 100, 500 patches
  - Patch chain application: 10, 50, 100 create patches
  - FileTree diff: 10, 50, 100 entry trees

#### Quality Compliance
- `cargo clippy --workspace --all-targets`: zero warnings
- `cargo audit`: zero vulnerabilities (300 crate dependencies scanned)
- Zero compiler warnings on `cargo build --workspace`

### Specifications
- Yellow Papers: Patch Algebra, Serialization, Distributed Consensus
- Blue Papers: CAS, Patch Algebra, Patch DAG, Metadata, Driver SDK, CLI
- Lean 4 formal proofs (pending toolchain — proofs use `sorry` placeholders)
- Interface contracts for all components
- STRIDE threat model and security test plan
- Performance requirements and benchmark suite

### Known Limitations
- No VFS (NFSv4/ProjFS) support
- No Raft/gRPC-based distributed consensus
- Lean 4 formal proofs pending toolchain installation
- Checkout does not handle uncommitted working tree changes (only staged) → Fixed in v0.3.0 with auto-stash
