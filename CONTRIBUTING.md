# Contributing to Suture

Thank you for your interest in contributing to Suture! This guide covers everything you need to get started.

## Prerequisites

- **Rust 1.85+** (edition 2024) — managed via `rust-toolchain.toml`
- **protoc** — for `suture-hub` (protobuf compilation)
- **SQLite development headers** — for `suture-hub` and `suture-platform`
- **Git** — for version control
- **libfuse3-dev** (Linux, optional) — only needed for `suture-vfs` FUSE integration tests

## Local Development

### Quick Start

1. Build all crates:
   ```bash
   cargo build --release
   ```

2. Run the hub (coordination server):
   ```bash
   cargo run -p suture-hub -- --addr 127.0.0.1:8080 --db ./hub-data/hub.db
   ```

3. Run the platform (hosted SaaS):
   ```bash
   export SUTURE_JWT_SECRET=dev-secret
   cargo run -p suture-platform -- --addr 127.0.0.1:3000 --jwt-secret "$SUTURE_JWT_SECRET"
   ```

4. Open http://localhost:3000

### Docker Compose

```bash
cp .env.example .env  # Edit with your values
docker compose up --build
```

Services:
- **suture-hub** — http://localhost:8080
- **suture-platform** — http://localhost:3000

### Running Tests

```bash
cargo test --workspace --exclude suture-py --exclude suture-e2e --exclude suture-wasm-plugin
```

### E2E Tests (require compiled binary)

```bash
cargo build -p suture-cli
cargo test -p suture-e2e -- --test-threads=1
```

## Getting Started

### Clone and Build

```bash
git clone https://github.com/WyattAu/suture.git
cd suture
cargo build --release
# Binary available at target/release/suture
```

### Build Just the CLI

```bash
cargo build -p suture-cli --release
```

### Run Tests

```bash
# All workspace tests (suture-py excluded — requires Python dev headers)
cargo test --workspace --exclude suture-py -- --test-threads=1

# Specific crate
cargo test -p suture-core

# With output
cargo test -p suture-core -- --nocapture

# Benchmarks (requires nightly or stable with bench support)
cargo bench
```

Tests in `suture-cli` use `set_current_dir` internally via a mutex guard. Run with `--test-threads=1` to avoid race conditions.

### Linting

```bash
cargo clippy --workspace --exclude suture-py -- -D warnings
cargo fmt --check --all
```

### Quick Development Commands

The `justfile` provides shortcuts:

```bash
just test      # cargo test --workspace
just lint      # cargo clippy --workspace -- -D warnings
just fmt       # cargo fmt --workspace
just build     # cargo build --workspace --release
just run       # cargo run --bin suture-cli
```

## Project Structure

| Crate | Description |
|-------|-------------|
| `suture-common` | Shared types: `Hash`, `BranchName`, `RepoPath`, `FileStatus`, errors |
| `suture-core` | Core engine: CAS, DAG, patches, repository, merge, stash, rebase, blame, audit, signing |
| `suture-protocol` | Wire protocol, delta encoding, Zstd compression |
| `suture-driver` | `SutureDriver` trait, `DriverRegistry`, plugin system (Wasmtime) |
| `suture-ooxml` | OOXML parsing shared by DOCX/XLSX/PPTX drivers |
| `suture-merge` | Standalone semantic merge library (feature-gated per format) |
| `suture-cli` | CLI binary (`suture` command) — 58 subcommands via clap |
| `suture-tui` | Terminal UI (ratatui + crossterm) |
| `suture-hub` | Central server: HTTP + gRPC + SQLite, auth, replication, webhooks |
| `suture-daemon` | Background daemon: file watcher, SHM, mount manager, auto-sync |
| `suture-vfs` | FUSE3 read/write mount + WebDAV server |
| `suture-raft` | Raft consensus protocol for hub clustering |
| `suture-s3` | S3-compatible blob storage (AWS SigV4) |
| `suture-node` | Node.js native addon (napi-rs) |
| `suture-lsp` | Language Server Protocol server |
| `suture-bench` | Criterion benchmarks (6 bench suites) |
| `suture-fuzz` | Fuzz targets (7 targets: patch deserialize, hash parse, merge, etc.) |
| `suture-e2e` | End-to-end tests (requires compiled binary) |
| `suture-driver-*` | 17 semantic merge drivers (see table below) |
| `suture-py` | Python bindings (PyO3, excluded from workspace) |

### Semantic Merge Drivers

| Driver | Extensions | Format |
|--------|-----------|--------|
| `suture-driver-json` | `.json` | JSON |
| `suture-driver-yaml` | `.yaml`, `.yml` | YAML |
| `suture-driver-toml` | `.toml` | TOML |
| `suture-driver-csv` | `.csv` | CSV |
| `suture-driver-xml` | `.xml` | XML |
| `suture-driver-markdown` | `.md` | Markdown |
| `suture-driver-html` | `.html` | HTML |
| `suture-driver-svg` | `.svg` | SVG |
| `suture-driver-docx` | `.docx` | DOCX (binary) |
| `suture-driver-xlsx` | `.xlsx` | XLSX (binary) |
| `suture-driver-pptx` | `.pptx` | PPTX (binary) |
| `suture-driver-sql` | `.sql` | SQL |
| `suture-driver-image` | `.png`, `.jpg`, etc. | Image |
| `suture-driver-pdf` | `.pdf` | PDF |
| `suture-driver-ical` | `.ics` | iCalendar |
| `suture-driver-feed` | `.rss`, `.atom` | RSS/Atom |
| `suture-driver-otio` | `.otio` | OpenTimelineIO |
| `suture-driver-example` | — | Example/reference driver |

## Development Workflow

### Branch Naming

- `feature/description` — new features
- `fix/description` — bug fixes
- `refactor/description` — code improvements
- `docs/description` — documentation changes

Always branch from `main`.

### Workflow

1. **Create a branch** from `main`
2. **Make changes** following the code style below
3. **Run tests**: `cargo test -p <crate>` or the full workspace command
4. **Run clippy**: `cargo clippy -p <crate> -- -D warnings`
5. **Format**: `cargo fmt`
6. **Submit a PR** with a clear description

## Quality Gates

Every PR must pass:

- `cargo check --workspace --exclude suture-py` — compiles cleanly
- `cargo test --workspace --exclude suture-py` — all tests pass
- `cargo clippy --workspace --exclude suture-py -- -D warnings` — zero warnings
- `cargo fmt --check` — formatted

## Adding a New Driver

Drivers are format-specific plugins that implement the `SutureDriver` trait from `suture-driver`.

### Step-by-Step

1. **Create the crate**:

   ```bash
   mkdir -p crates/suture-driver-<name>/src
   ```

   `Cargo.toml`:
   ```toml
   [package]
   name = "suture-driver-<name>"
    version = "5.3.1"
   edition = "2024"

   [dependencies]
   suture-driver = { path = "../suture-driver", version = "5.0.0" }
   suture-common = { path = "../suture-common", version = "5.0.0" }
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   thiserror = "2"
   ```

2. **Implement `SutureDriver`** in `src/lib.rs`:

   ```rust
   use suture_driver::{DriverError, SemanticChange, SutureDriver};

   pub struct MyDriver;

   impl SutureDriver for MyDriver {
       fn name(&self) -> &str { "MyFormat" }
       fn supported_extensions(&self) -> &[&str] { &[".myext"] }
       fn diff(&self, base: Option<&str>, new: &str) -> Result<Vec<SemanticChange>, DriverError> { /* ... */ }
       fn format_diff(&self, base: Option<&str>, new: &str) -> Result<String, DriverError> { /* ... */ }
       fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> { /* ... */ }
   }
    ```

   Alternatively, for structured formats (JSON, YAML, TOML, etc.), use the `impl_structured_driver!` macro instead of implementing the trait manually. See `suture-driver-json` for an example:

   ```rust
   use suture_driver::impl_structured_driver;

   impl_structured_driver! {
       name: "MyFormat",
       extensions: &[".myext"],
       parse_fn: my_parse,
       merge_fn: my_merge,
   }
   ```

   See `crates/suture-driver-example/` for a complete reference implementation.

3. **Add tests** — minimum 10 tests per driver. Unit tests in `src/lib.rs`, integration tests in `tests/`

4. **Add to workspace** — the glob `crates/*` in the root `Cargo.toml` picks it up automatically

5. **Wire into CLI** — add the dependency to `suture-cli/Cargo.toml` and register it in `suture-cli/src/driver_registry.rs`:

   ```rust
   registry.register(Box::new(suture_driver_myext::MyDriver));
   ```

   If using the `impl_structured_driver!` macro, the generated `register` function can be called directly:
   ```rust
   suture_driver_myext::register(&mut registry);
   ```

6. **Optionally add to `suture-merge`** — add as an optional dependency with a feature flag, following the pattern of existing drivers in `suture-merge/Cargo.toml`

7. **Add to publish order** — append to `PUBLISH_ORDER` in `scripts/publish.sh`

## Code Style

- Follow `rustfmt` defaults (`cargo fmt`)
- Clippy must pass with `-D warnings`
- Use `thiserror` for error types (see `DriverError`, `RepoError`, `CasError` for patterns)
- No `.unwrap()` or `.expect()` in library/production code — use `?` or proper error handling
- All public functions must have `///` doc comments
- Test coverage >80% for new code
- Rust edition 2024 — note that `std::env::set_var` requires an `unsafe` block
- New features must include tests

## Testing

### Unit Tests
Inline `#[cfg(test)] mod tests` blocks within each module.

### Property-Based Tests
`suture-core` and `suture-driver` use `proptest`. See `suture-core/src/patch/types.rs` and `suture-core/src/lib.rs` for examples.

### Integration Tests
Place in a `tests/` directory within the crate. Tests that use `set_current_dir` require `--test-threads=1`.

### Fuzzing
`crates/suture-fuzz/` contains 7 fuzz targets using `libfuzzer-sys`. Run with:
```bash
cargo +nightly fuzz run fuzz_json_merge
```

### End-to-End Tests
`crates/suture-e2e/` runs against a compiled `suture` binary. See `TESTING_GUIDE.md` for the full manual test walkthrough.

### Benchmarks
`crates/suture-bench/` provides 6 Criterion benchmark suites:
```bash
cargo bench
```

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(cli): add interactive rebase
fix(merge): resolve conflict in YAML three-way merge
docs(readme): update install instructions
refactor(hub): extract pagination into helper module
test(core): add property tests for patch commutativity
chore: update dependencies
```

Scope is typically the crate name without the `suture-` prefix.

## Pull Requests

1. Fork the repository
2. Create a feature branch from `main`
3. Make changes with tests
4. Run all quality gates (check, test, clippy, fmt)
5. Open a PR with:
   - Clear description of the change and motivation
   - Link to any related issues
   - Confirmation that all quality gates pass

## Reporting Bugs

Open a GitHub issue with:

- Suture version (`suture version`)
- Operating system and architecture
- Steps to reproduce
- Expected vs actual behavior
- Relevant log output

## Release Process

1. **Version bump** — update the version in all `Cargo.toml` files (`workspace.package.version` and individual crate versions)
2. **Update CHANGELOG.md** — add entries under the new version
3. **Run full test suite** — `cargo test --workspace --exclude suture-py --exclude suture-e2e --exclude suture-wasm-plugin`
4. **Run quality gates** — `cargo clippy`, `cargo fmt --check`
5. **Push tag** — pushing a `v*` tag triggers the CI release workflow

```bash
# Dry run (no actual publishing)
scripts/publish.sh

# Real publish
scripts/publish.sh --real
```

The script publishes all 37 crates to crates.io in dependency order.

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
