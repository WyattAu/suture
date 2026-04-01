# Contributing to Suture

Thank you for your interest in contributing to Suture!

## Development Setup

### Prerequisites
- Rust stable (see `rust-toolchain.toml`)
- Nix (optional, for reproducible builds)

### Building
```bash
cargo build --release
```

### Testing
```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p suture-core

# Run with output
cargo test --workspace -- --nocapture

# Run benchmarks
cargo bench
```

### Linting
```bash
cargo clippy --workspace -- -D warnings
cargo fmt --check --all
```

## Code Style
- Follow `rustfmt` defaults
- No `unwrap()` in production code — use `?` or proper error handling
- All public functions must have `///` doc comments
- New patches must include tests

## Project Structure
- `crates/suture-core/` — Core engine (CAS, DAG, patches, merge)
- `crates/suture-cli/` — Command-line interface
- `crates/suture-hub/` — Remote Hub server
- `crates/suture-driver-*/` — Semantic drivers for different file formats
- `crates/suture-protocol/` — Shared protocol types
- `crates/suture-e2e/` — End-to-end integration tests
- `crates/suture-bench/` — Criterion benchmarks

## Pull Request Process
1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes with tests
4. Ensure `cargo test --workspace` passes
5. Ensure `cargo clippy --workspace -- -D warnings` passes
6. Submit a PR with a clear description

## Reporting Bugs
Please open a GitHub issue with:
- Suture version (`suture version`)
- Operating system
- Steps to reproduce
- Expected vs actual behavior

## License
By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
