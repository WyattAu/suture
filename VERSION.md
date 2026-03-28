# Suture Version

- **Current Version:** 0.3.0
- **Current Phase:** 10 (v0.3 Release)
- **Status:** Complete
- **Last Updated:** 2026-03-28
- **Rust Edition:** 2024
- **Lean 4:** Not installed (formal verification pending — proofs use `sorry` placeholders)

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 213 passing | 0 failures across 6 crates |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 6 Criterion groups | CAS, hashing, DAG, apply, diff, LCA |
| Clippy | ✅ Zero warnings | `cargo clippy --workspace` clean |
| Audit | ✅ Zero vulnerabilities | `cargo audit` exit code 0 |
| Ed25519 signing | ✅ Wired into push | `suture key generate`, auto-sign on push |
| Lean 4 proofs | ⏳ Pending | Proof files exist with `sorry` placeholders |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 176 | Core engine (CAS, DAG, patches, repo, engine, signing, merge, stash) |
| suture-cli | 0 | CLI binary (init, status, add, commit, branch, log, merge, checkout, diff, revert, tag, config, key, push, pull, remote, stash, completions) |
| suture-hub | 15 | Hub daemon with SQLite persistence and Ed25519 auth |
| suture-daemon | 1 | Daemon placeholder |
| suture-driver-otio | 12 | OpenTimelineIO reference driver |
| suture-bench | — | Criterion benchmarks (6 groups) |
