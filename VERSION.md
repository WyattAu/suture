# Suture Version

- **Current Version:** 0.9.0
- **Current Phase:** 10 (v0.9 Release)
- **Status:** Complete
- **Last Updated:** 2026-03-29
- **Rust Edition:** 2024
- **Lean 4:** Not installed (formal verification pending — proofs use `sorry` placeholders)

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 274 passing | 0 failures across 14 crates |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 6 Criterion groups | CAS, hashing, DAG, apply, diff, LCA |
| Clippy | ✅ Zero warnings | `cargo clippy --workspace` clean |
| Audit | ✅ Zero vulnerabilities | `cargo audit` exit code 0 |
| Ed25519 signing | ✅ Wired into push | `suture key generate`, auto-sign on push |
| E2E tests | ✅ 7 integration tests | init→commit→branch→merge→gc→fsck→bisect→tag→stash |
| Lean 4 proofs | ⏳ Pending | Proof files exist with `sorry` placeholders |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 191 | Core engine (CAS, DAG, patches, repo, engine, signing, merge, stash, reset, cherry-pick, rebase, blame, reflog, rm, mv, notes, gc, fsck) |
| suture-cli | 0 | CLI binary (35 commands) |
| suture-hub | 15 | Hub daemon with SQLite persistence and Ed25519 auth |
| suture-daemon | 1 | Daemon placeholder |
| suture-driver | 0 | SutureDriver trait, DriverRegistry, semantic diff/merge types |
| suture-driver-otio | 12 | OpenTimelineIO reference driver |
| suture-driver-json | 16 | JSON semantic driver with diff, merge, RFC 6901 paths |
| suture-driver-yaml | 10 | YAML semantic driver with diff, merge |
| suture-driver-toml | 7 | TOML semantic driver with diff, merge |
| suture-driver-csv | 5 | CSV semantic driver with diff |
| suture-driver-xml | 9 | XML semantic driver with diff, merge |
| suture-e2e | — | End-to-end integration tests (7 workflow tests) |
| suture-bench | — | Criterion benchmarks (6 groups) |
