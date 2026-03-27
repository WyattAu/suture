# Suture Version

- **Current Version:** 0.2.0-dev
- **Current Phase:** 8 (v0.2 Development) — Hub Persistence + Auth + Line-level Merge
- **Status:** In Development
- **Last Updated:** 2026-03-27
- **Rust Edition:** 2024
- **Lean 4:** Not installed (formal verification pending — proofs use `sorry` placeholders)

## Quality Gate Compliance

| Gate | Status | Details |
|------|--------|---------|
| Tests | ✅ 199 passing | 0 failures across 6 crates |
| Property-based tests | ✅ 21 proptest suites | 10K+ cases via proptest |
| Benchmarks | ✅ 6 Criterion groups | CAS, hashing, DAG, apply, diff, LCA |
| Clippy | ✅ Zero warnings | `cargo clippy --all-targets` clean |
| Audit | ✅ Zero vulnerabilities | `cargo audit` exit code 0 |
| Ed25519 signing | ✅ Module ready | `signing.rs` with keypair, canonical bytes, verify |
| Lean 4 proofs | ⏳ Pending | Proof files exist with `sorry` placeholders |

## Workspace Crates

| Crate | Tests | Description |
|-------|-------|-------------|
| suture-common | 8 | Shared types (Hash, BranchName, RepoPath) |
| suture-core | 162 | Core engine (CAS, DAG, patches, repo, engine, signing, merge) |
| suture-cli | 0 | CLI binary (init, status, add, commit, branch, log, merge, checkout, diff, revert, push, pull, remote) |
| suture-hub | 15 | Hub daemon with SQLite persistence and Ed25519 auth |
| suture-daemon | 1 | Daemon placeholder |
| suture-driver-otio | 12 | OpenTimelineIO reference driver |
| suture-bench | — | Criterion benchmarks (6 groups) |
