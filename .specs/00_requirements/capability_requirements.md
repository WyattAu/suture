# Capability Requirements: Suture — Universal Semantic Version Control System

**Document ID:** SPEC-CR-001
**Status:** Draft
**Date:** 2026-03-27
**Environment:** Checked against development host (Linux x86_64, Nix 3.15.2, Rust 1.94.1)

---

## 1. Overview

This document catalogs all tools, libraries, and capabilities required to build, test, verify, and deploy Suture. Each capability is assessed for current availability, version requirements, and gaps that must be addressed before the corresponding development phase begins.

---

## 2. Core Language and Toolchain

### 2.1. Rust Stable Toolchain

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Compiler** | rustc (stable channel, edition 2024) | **Available** — rustc 1.94.1 (2026-03-25) |
| **Package Manager** | cargo (matching rustc version) | **Available** — cargo 1.94.1 |
| **Edition** | 2024 | **Supported** — edition 2024 stabilized in Rust 1.85+ |
| **Components** | rust-src, rust-analyzer, clippy, rustfmt | **Available** — configured via Nix flake extensions |
| **Target Triples** | x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin, x86_64-pc-windows-msvc | **Configurable** — via `rustup target add` or Nix overlays |

**Notes:** Edition 2024 is mandatory as it stabilizes features required for Suture's API surface (notably `gen_blocks` and improved `unsafe` semantics). The Nix flake pins to `rust-bin.stable.latest.default`.

### 2.2. Lean 4 (Formal Verification)

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Language** | Lean 4 (for formal proofs of patch commutativity and CAS invariants) | **NOT AVAILABLE** on development host |
| **Purpose** | Machine-checked proofs of algebraic properties: commutativity, associativity, identity for patch operations; CAS consistency invariants; DAG merge correctness |
| **Priority** | P2 — Recommended (critical for correctness assurance, but not blocking Phase 1 development) |

**Action Required:**
- Add Lean 4 to Nix flake `buildInputs`: `lean4` package is available in nixpkgs.
- Create a `proofs/` directory at workspace root for Lean source files.
- Define proof targets in CI (separate from Rust build pipeline).
- Phase 1 can proceed with property-based testing (proptest) as a surrogate for formal verification. Formal proofs should be completed before Phase 2 GA.

### 2.3. Nix Flakes (Build Environment)

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Nix** | Determinate Nix or Lix ( flakes-enabled ) | **Available** — Determinate Nix 3.15.2 |
| **Flake** | `flake.nix` with dev shell, multi-target build, and CI support | **Available** — `flake.nix` exists at workspace root |
| **Current Dev Shell** | rustToolchain, pkg-config, openssl, sqlite, protobuf, fuse3, libiconv | **Configured** |
| **Missing from Dev Shell** | lean4, protobuf-compiler (for tonic-build), cargo-audit, cargo-deny, just or make | **Gaps identified** (see Section 8) |

---

## 3. Testing and Verification Tools

### 3.1. Concurrency Testing: `loom`

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Crate** | `loom` ( concurrency model testing for Rust ) | **Available** as a crate dependency |
| **Purpose** | Exhaustive exploration of interleavings for lock-free data structures (SHM, atomic DAG state machine) |
| **Usage** | Dev and test dependency only; `#[cfg(test)]` gated; not included in release builds |
| **Note** | `loom` is not a cargo subcommand — it is a Rust crate used in test code to simulate different thread schedulings and memory orderings |

**Integration Pattern:**
```toml
[dev-dependencies]
loom = "0.7"
```

### 3.2. Property-Based Testing: `proptest`

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Crate** | `proptest` ( property-based testing framework for Rust ) | **Not yet in workspace** |
| **Alternative** | `quickcheck` (Haskell-style property testing) | **Not yet in workspace** |
| **Recommendation** | Use `proptest` — more mature Rust integration, better shrunk counterexamples, built-in strategy combinators |
| **Priority** | P0 — Mandatory for Phase 1 |

**Target Properties:**
- CAS write-then-read roundtrip: `forall blob: hash(blob) == blake3(blob)` and `read(hash) == blob`
- Patch commutativity: `forall commuting patches (P1, P2): apply(P1, apply(P2, S)) == apply(P2, apply(P1, S))`
- DAG merge idempotency: `merge(merge(A, B), C) == merge(A, merge(B, C))` for commuting patches
- Flatbuffers serialization roundtrip: `forall patch: deserialize(serialize(patch)) == patch`

### 3.3. Benchmarking: `criterion`

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Crate** | `criterion` ( benchmarking framework for Rust ) | **Not yet in workspace** |
| **Purpose** | Continuous performance regression detection for metadata lookups, hash throughput, merge latency |
| **Priority** | P1 — Required for Phase 2 (performance-sensitive VFS and daemon work) |

**Target Benchmarks:**
- BLAKE3 throughput (single-thread, multi-thread SIMD)
- Patch commutativity check latency (zero-copy Flatbuffers vs. deserialized)
- DAG merge latency scaling (10, 100, 1000, 10000 patches)
- SQLite WAL write throughput (concurrent writers)
- SHM lookup latency (nanosecond-precision via `std::time::Instant`)

### 3.4. Dependency Security: `cargo-audit`

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Tool** | `cargo-audit` ( audits Cargo dependencies for known vulnerabilities ) | **Not in Nix dev shell** |
| **Purpose** | CI gate: fail build if any dependency has a known CVE (RUSTSEC advisory) |
| **Priority** | P0 — Mandatory for Phase 1 |

**Action Required:** Add `cargo-audit` to Nix flake `buildInputs` and configure CI pipeline to run `cargo audit` on every PR.

### 3.5. Dependency Linting: `cargo-deny`

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Tool** | `cargo-deny` ( linting for dependency licenses, duplicates, and advisories ) | **Not in Nix dev shell** |
| **Purpose** | Enforce Apache-2.0 compatibility for all transitive dependencies; detect duplicate crates with different versions |
| **Priority** | P1 — Required for Phase 1 GA |

**Action Required:** Create `deny.toml` at workspace root. Configure:
- `[advisories]` — deny all RUSTSEC advisories
- `[licenses]` — allow Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-DFS-2016
- `[bans]` — deny duplicate crates (e.g., multiple versions of `syn` or `serde`)

---

## 4. Core Dependencies

### 4.1. Currently Declared in Workspace

| Crate | Version | Crate | Status | Notes |
|:---|:---|:---|:---|:---|
| `blake3` | 1.8.3 | suture-core | **Available** | SIMD-accelerated; parallelizable; default CAS hasher |
| `rusqlite` | 0.39.0 | suture-core | **Available** | Bundled SQLite; WAL mode feature needed: `features = ["bundled"]` |
| `flatbuffers` | 25.12.19 | suture-core | **Available** | Zero-copy serialization; need to add `flatc` to Nix shell for schema compilation |
| `tonic` | 0.14.5 | suture-core | **Available** | gRPC framework; requires `tonic-build` as build dependency for proto compilation |
| `prost` | 0.14.3 | suture-core | **Available** | Protobuf codec for tonic; replaces older `protobuf` crate |
| `tokio` | 1.50.0 | suture-core | **Available** | Async runtime; `features = ["full"]` |
| `serde` | 1.0.228 | suture-core | **Available** | Serialization framework; needed for config, CLI args |
| `tracing` | 0.1.44 | suture-core | **Available** | Structured logging; spans for distributed tracing |
| `tracing-subscriber` | 0.3.23 | suture-core | **Available** | Log formatting and output |

### 4.2. Required but Not Yet Declared

| Crate | Purpose | Target Crate | Priority | Notes |
|:---|:---|:---|:---|:---|
| `quinn` | QUIC transport (encrypted, multiplexed UDP) | suture-core | P0 | Used by tonic for QUIC transport; also standalone for lease heartbeats |
| `clap` | CLI argument parsing with derive macros | suture-cli | P0 | `features = ["derive", "env"]` for `suture init`, `suture status`, `suture mount` |
| `ed25519-dalek` | Ed25519 signing and verification | suture-core | P0 | Patch signature creation and verification; `features = ["rand_core"]` |
| `zeroize` | Secure memory zeroization for key material | suture-core | P0 | `derive` feature for auto-zeroize on Drop |
| `zstd` | Zstd compression for CAS blob storage | suture-core | P1 | Streaming compression for large files; `features = ["zstdmt"]` for multi-threaded |
| `flatc` | Flatbuffers schema compiler | Build tool | P1 | Must be in Nix dev shell for `.fbs` → `.rs` codegen |
| `tonic-build` | gRPC code generation from `.proto` files | Build dep | P0 | `build.rs` integration for compiling proto definitions |
| `proptest` | Property-based testing | Dev dep (all) | P0 | See Section 3.2 |
| `criterion` | Benchmarking | Dev dep (suture-core) | P1 | See Section 3.3 |
| `loom` | Concurrency model testing | Dev dep (suture-core) | P1 | See Section 3.1 |
| `iceoryx2` or custom SHM | Shared memory IPC between daemon and UI | suture-daemon | P1 | `iceoryx2` is Rust-native; custom SHM via `shared_memory` crate is simpler alternative |
| `fluent` + `fluent-bundle` | i18n/l10n for CLI and error messages | suture-cli, suture-core | P2 | Mozilla's localization framework; `.ftl` translation files |
| `pyo3` | Python bindings for driver SDK | suture-driver-otio | P2 | DaVinci Resolve Python API integration; deferred to Phase 3 |

---

## 5. Build and Infrastructure Tools

### 5.1. SQLite

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Library** | SQLite 3.x (embedded via rusqlite "bundled" feature) | **Available** — bundled with rusqlite 0.39.0 |
| **CLI** | `sqlite3` command-line tool (development/debugging) | **Available** — in Nix dev shell |
| **Mode** | WAL (Write-Ahead Logging) for concurrent read/write | **Configured** — set at DB open time in suture-core |
| **Extensions** | FTS5 (full-text search for patch messages), JSON1 (patch metadata) | **Bundled** — available via rusqlite feature flags |

### 5.2. Protocol Buffers (gRPC Definitions)

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Compiler** | `protoc` (Protocol Buffer Compiler) | **Available** — `protobuf` in Nix dev shell |
| **Rust Plugin** | `prost-build` (via `tonic-build`) | **Not yet declared** — needed as build dependency |
| **Schema Location** | `proto/` directory at workspace root | **To be created** |
| **Definitions Needed** | `suture/patch.proto`, `suture/cas.proto`, `suture/dag.proto`, `suture/lease.proto`, `suture/sync.proto` | **Phase 1** |

### 5.3. Flatbuffers Schema Compiler

| Attribute | Requirement | Current Status |
|:---|:---|:---|
| **Compiler** | `flatc` (Flatbuffers compiler) | **Not in Nix dev shell** |
| **Schema Location** | `schemas/` directory at workspace root | **To be created** |
| **Definitions Needed** | `patch.fbs`, `dag_node.fbs`, `driver_ir.fbs`, `vfs_entry.fbs` | **Phase 1** |

**Action Required:** Add `flatbuffers` (provides `flatc`) to Nix flake `buildInputs`. Note: the nixpkgs `flatbuffers` package provides the compiler; the Rust crate is separate.

---

## 6. Capability Matrix: Available vs. Required

### 6.1. Language and Build

| Capability | Required | Available | Gap | Action |
|:---|:---|:---|:---|:---|
| Rust stable (edition 2024) | Yes | Yes (1.94.1) | None | — |
| Lean 4 (formal proofs) | Recommended | No | **Install** | Add `lean4` to Nix flake; create `proofs/` directory |
| Nix Flakes (dev environment) | Yes | Yes (3.15.2) | None | — |
| `flatc` compiler | Yes | No | **Install** | Add `flatbuffers` package to Nix flake `buildInputs` |
| `protoc` compiler | Yes | Yes | None | — |
| `tonic-build` (gRPC codegen) | Yes | No | **Add dep** | Add to `[build-dependencies]` in suture-core |
| `cargo-audit` | Yes | No | **Install** | Add to Nix flake `buildInputs`; configure CI gate |
| `cargo-deny` | Recommended | No | **Install** | Add to Nix flake; create `deny.toml` |

### 6.2. Testing and Verification

| Capability | Required | Available | Gap | Action |
|:---|:---|:---|:---|:---|
| `proptest` (property testing) | Yes | No | **Add dep** | Add to `[dev-dependencies]` in suture-core |
| `loom` (concurrency testing) | Yes | No | **Add dep** | Add to `[dev-dependencies]` in suture-core |
| `criterion` (benchmarking) | Yes | No | **Add dep** | Add to `[dev-dependencies]` in suture-core |
| `cargo-nextest` (test runner) | Recommended | No | **Install** | Add to Nix flake; faster parallel test execution |
| Miri (UB detection) | Recommended | No | **Install** | Add `rust-miri` to Nix flake; run in CI for `unsafe` code |

### 6.3. Runtime Dependencies

| Capability | Required | Available | Gap | Action |
|:---|:---|:---|:---|:---|
| `blake3` (hashing) | Yes | Yes (1.8.3) | None | — |
| `rusqlite` (metadata DB) | Yes | Yes (0.39.0) | None | — |
| `flatbuffers` (serialization) | Yes | Yes (25.12.19) | None | — |
| `tonic` (gRPC) | Yes | Yes (0.14.5) | None | — |
| `quinn` (QUIC transport) | Yes | No | **Add dep** | Add to suture-core dependencies |
| `clap` (CLI framework) | Yes | No | **Add dep** | Add to suture-cli dependencies |
| `ed25519-dalek` (signing) | Yes | No | **Add dep** | Add to suture-core dependencies |
| `zeroize` (key zeroization) | Yes | No | **Add dep** | Add to suture-core dependencies |
| `zstd` (compression) | Recommended | No | **Add dep** | Add to suture-core dependencies |
| SHM IPC crate | Yes | No | **Select & add** | Evaluate `iceoryx2` vs. `shared_memory` crate |
| `fluent` (i18n) | Recommended | No | **Add dep** | Add to suture-cli and suture-core |
| `pyo3` (Python bindings) | Phase 3 | No | **Deferred** | Not needed until Phase 3 driver SDK |

---

## 7. Nix Flake Dev Shell Gaps

Current `flake.nix` `buildInputs`:
```
rustToolchain, pkg-config, openssl, sqlite, protobuf, fuse3, libiconv
```

**Recommended additions:**
```
flatbuffers     # flatc compiler for .fbs → .rs codegen
cargo-audit     # dependency vulnerability scanning
cargo-deny      # dependency license and duplicate linting
cargo-nextest   # faster test runner
lean4           # formal verification proofs
just            # task runner (alternative to Makefile)
```

---

## 8. CI/CD Pipeline Requirements

| Stage | Tool | Purpose | Priority |
|:---|:---|:---|:---|
| **Lint** | `cargo clippy -- -D warnings` | Catch common mistakes, enforce idiomatic Rust | P0 |
| **Format** | `cargo fmt -- --check` | Enforce consistent code style | P0 |
| **Test** | `cargo nextest run` | Parallel test execution with better output | P0 |
| **Audit** | `cargo audit` | Block PRs with known CVEs in dependencies | P0 |
| **Deny** | `cargo deny check` | License compatibility, duplicate detection | P1 |
| **Miri** | `cargo +nightly miri test` | Detect undefined behavior in `unsafe` code | P1 |
| **Benchmark** | `cargo criterion` | Track performance regressions over time | P2 |
| **Lean** | `lake build` | Verify formal proofs compile and check | P2 |

---

## 9. Development Environment Prerequisites

| Requirement | Minimum Version | Notes |
|:---|:---|:---|
| Nix (with flakes) | 2.33+ | Determinate Nix or Lix recommended |
| Rust stable | 1.85+ | Edition 2024 support required |
| SQLite | 3.40+ | WAL mode, FTS5, JSON1 extensions |
| Protobuf compiler | 3.20+ | For tonic-build gRPC codegen |
| Flatbuffers compiler | 24.0+ | For .fbs schema compilation |
| Git | 2.40+ | For version control of Suture itself |
| Platform | Linux (x86_64/aarch64), macOS, Windows | Feature parity required |

---

## 10. Summary of Immediate Actions

1. **Add missing Nix packages** to `flake.nix`: `flatbuffers`, `cargo-audit`, `cargo-deny`, `just`.
2. **Declare missing Rust dependencies** in `Cargo.toml` files: `quinn`, `clap`, `ed25519-dalek`, `zeroize`, `tonic-build` (build-dep), `proptest`, `criterion`, `loom` (all as dev-deps).
3. **Create `deny.toml`** at workspace root with license allowlist.
4. **Create `proto/` directory** with initial gRPC service definitions.
5. **Create `schemas/` directory** with initial Flatbuffers schema definitions.
6. **Evaluate Lean 4** for formal verification; add to Nix flake and create `proofs/` directory.
7. **Evaluate SHM IPC crate** — benchmark `iceoryx2` vs. `shared_memory` for daemon-to-UI communication.
