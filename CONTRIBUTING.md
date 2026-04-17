# Contributing to Suture

Thank you for your interest in contributing to Suture!

## Development Setup

### Prerequisites
- Rust 1.85+ (edition 2024, see `rust-toolchain.toml`)
- Git
- `libfuse3-dev` (Linux only, optional — needed for FUSE integration tests)

### Building
```bash
cargo build --release
# Binary at target/release/suture
```

### Testing
```bash
# All tests (suture-py excluded — requires Python dev headers)
cargo test --workspace --exclude suture-py -- --test-threads=4

# Specific crate
cargo test -p suture-core

# With output
cargo test --workspace --exclude suture-py -- --nocapture

# Benchmarks
cargo bench
```

### Linting
```bash
cargo clippy --workspace --exclude suture-py -- -D warnings
cargo fmt --check --all
```

### Quality Gates
Every PR must pass:
- `cargo check --workspace --exclude suture-py` — compiles cleanly
- `cargo test --workspace --exclude suture-py` — all 872 tests pass
- `cargo clippy --workspace --exclude suture-py -- -D warnings` — zero warnings
- `cargo fmt --check` — formatted

## Project Structure
- `crates/suture-common/` — Shared types (Hash, BranchName, RepoPath)
- `crates/suture-core/` — Core engine (CAS, DAG, patches, repo, merge, stash, rebase, blame)
- `crates/suture-protocol/` — Wire protocol, V2 handshake, delta encoding, compression
- `crates/suture-cli/` — CLI binary (37 commands)
- `crates/suture-tui/` — Terminal UI (7 tabs)
- `crates/suture-hub/` — Hub server (HTTP + gRPC + SQLite, auth, replication, mirrors)
- `crates/suture-daemon/` — Background daemon (file watcher, SHM, mount manager, auto-sync)
- `crates/suture-vfs/` — FUSE read/write mount + WebDAV server
- `crates/suture-driver-*/` — 16 semantic merge drivers
- `crates/suture-raft/` — Raft consensus protocol
- `crates/suture-s3/` — S3-compatible blob storage (AWS SigV4)
- `crates/suture-node/` — Node.js native addon (napi-rs)
- `crates/suture-lsp/` — Language Server Protocol (hover, diagnostics)
- `crates/suture-py/` — Python bindings (PyO3, excluded from workspace)
- `jetbrains-plugin/` — IntelliJ plugin (Kotlin)
- `desktop-app/` — Tauri v2 scaffold (excluded from workspace)

## Architecture Notes
- **Patch-based VCS** — operations create patches, not snapshots
- **Semantic merge** — format-aware drivers detect structural vs. genuine conflicts
- **suture-py** excluded from workspace (PyO3 requires Python dev headers)
- **desktop-app** excluded from workspace (Tauri separate build)
- FUSE integration tests require root and are `#[ignore]`d

## Code Style
- Follow `rustfmt` defaults
- No `unwrap()` in production code — use `?` or proper error handling
- All public functions must have `///` doc comments
- New features must include tests
- Rust edition 2024 (e.g., `std::env::set_var` requires `unsafe` block)

## Commit Messages
Use Conventional Commits:
```
feat(cli): add interactive rebase
fix(merge): resolve conflict in YAML three-way merge
docs(readme): update install instructions
refactor(hub): extract pagination into helper module
```

## Pull Request Process
1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes with tests
4. Run all quality gates (check, test, clippy, fmt)
5. Submit a PR with a clear description using the PR template

## Reporting Bugs
Open a GitHub issue with:
- Suture version (`suture version`)
- Operating system and architecture
- Steps to reproduce
- Expected vs actual behavior
- Relevant log output

## License
By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
