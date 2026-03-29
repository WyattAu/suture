# Suture Version

- **Current Version:** 0.11.0
- **Current Phase:** 10 (v0.11 Release)
- **Status:** Complete
- **Last Updated:** 2026-03-29
- **Rust Edition:** 2024
- **Lean 4:** Not installed (formal verification pending — proofs use `sorry` placeholders)

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 319 passing | 0 failures across 17 crates |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 6 Criterion groups | CAS, hashing, DAG, apply, diff, LCA |
| Clippy | ✅ Zero warnings | `cargo clippy --workspace` clean |
| Audit | ✅ Zero vulnerabilities | `cargo audit` exit code 0 |
| Ed25519 signing | ✅ Wired into push | `suture key generate`, auto-sign on push |
| E2E tests | ✅ 7 integration tests | init→commit→branch→merge→gc→fsck→bisect→tag→stash |
| Patch Algebra | ✅ Formal foundation | Composition, commutativity, conflict classification |
| Lean 4 proofs | ⏳ Pending | Proof files exist with `sorry` placeholders |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 207 | Core engine (CAS, DAG, patches, repo, engine, signing, merge, stash, reset, cherry-pick, rebase, blame, reflog, rm, mv, notes, gc, fsck, squash, patch composition, conflict classification) |
| suture-cli | 0 | CLI binary (36 commands) |
| suture-hub | 15 | Hub daemon with SQLite persistence and Ed25519 auth |
| suture-daemon | 1 | Daemon placeholder |
| suture-driver | 0 | SutureDriver trait, DriverRegistry, semantic diff/merge types |
| suture-ooxml | 4 | Shared OOXML infrastructure (ZIP, part navigation) |
| suture-driver-otio | 13 | OpenTimelineIO reference driver (element ID fix) |
| suture-driver-json | 16 | JSON semantic driver with diff, merge, RFC 6901 paths |
| suture-driver-yaml | 10 | YAML semantic driver with diff, merge |
| suture-driver-toml | 7 | TOML semantic driver with diff, merge |
| suture-driver-csv | 13 | CSV semantic driver with diff, merge |
| suture-driver-xml | 9 | XML semantic driver with diff, merge |
| suture-driver-docx | 7 | DOCX semantic driver (paragraph diff/merge) |
| suture-driver-xlsx | 5 | XLSX semantic driver (cell-level diff) |
| suture-driver-pptx | 7 | PPTX semantic driver (slide-level diff) |
| suture-e2e | — | End-to-end integration tests (7 workflow tests) |
| suture-bench | — | Criterion benchmarks (6 groups) |
