# Requirements Specification: Suture — Universal Semantic Version Control System

**Document ID:** SPEC-REQ-001
**Status:** Draft
**Date:** 2026-03-27
**Notation:** EARS (Easy Approach to Requirements Syntax)

**Legend:**
- SHALL = mandatory requirement
- SHOULD = desired requirement
- WILL NOT = explicit exclusion

---

## Core Engine (REQ-CORE)

**REQ-CORE-001:** The system SHALL implement a Patch-DAG based version control engine written in pure Rust.

**REQ-CORE-002:** The system SHALL be deterministic — every operation SHALL produce identical output given identical input, regardless of execution order for commuting patches.

**REQ-CORE-003:** The system SHALL be idempotent — replaying any patch set in any valid commutative order SHALL produce the same project state.

**REQ-CORE-004:** The system SHALL support concurrent read access via lock-free data structures or RwLock-protected data structures for the Patch-DAG.

**REQ-CORE-005:** The system SHALL serialize all metadata operations through a single-writer pipeline to guarantee DAG consistency under concurrent writes.

**REQ-CORE-006:** The system SHALL provide a public Rust library crate (`libsuture`) exposing the full patch algebra, CAS, and DAG APIs.

**REQ-CORE-007:** The system SHALL use the `tokio` async runtime for all I/O-bound operations.

---

## Content Addressable Storage (REQ-CAS)

**REQ-CAS-001:** The system SHALL use BLAKE3 as the default content hash algorithm for all CAS blob addressing.

**REQ-CAS-002:** The system SHALL store blobs with Zstd compression at a configurable level (default: level 3).

**REQ-CAS-003:** The system SHALL deduplicate identical blobs via BLAKE3 hash equality, storing only one physical copy per unique hash.

**REQ-CAS-004:** The system SHALL support virtual blobs — references to external large files (e.g., video media) that are stored by pointer without copying the underlying data.

**REQ-CAS-005:** The system SHALL guarantee lossless round-trip for all stored blobs — decompression of a stored blob SHALL produce byte-identical output to the original input.

**REQ-CAS-006:** The system SHALL achieve greater than 1 GB/s BLAKE3 hashing throughput on a single thread on modern x86_64 hardware with SIMD support.

**REQ-CAS-007:** The system SHALL achieve greater than 100 MB/s CAS write throughput with Zstd compression enabled.

**REQ-CAS-008:** The system SHALL support a pluggable hash backend trait (`ContentHasher`) to allow alternative hash algorithms (e.g., SHA3-256 for FIPS compliance).

**REQ-CAS-009:** The system SHALL perform garbage collection of unreferenced blobs via DAG reachability analysis.

**REQ-CAS-010:** The system SHALL store each blob atomically — a failed write SHALL not leave a partial or corrupt blob in the store.

---

## Patch Algebra (REQ-PATCH)

**REQ-PATCH-001:** The system SHALL define patches as typed operations (e.g., `UpdateNode`, `MoveClip`, `EditCell`) each carrying a touch set identifying the semantic regions they modify.

**REQ-PATCH-002:** The system SHALL detect commutativity of two patches based on disjoint touch sets — patches with non-overlapping touch sets SHALL commute.

**REQ-PATCH-003:** The system SHALL implement deterministic merging via set-union of independent (commuting) patches from two branches.

**REQ-PATCH-004:** The system SHALL produce first-class conflict nodes in the DAG when patches from merged branches have overlapping touch sets and cannot commute.

**REQ-PATCH-005:** The system SHALL preserve full information from both branches in conflict nodes — zero data loss from either side of the merge.

**REQ-PATCH-006:** The system SHALL support an identity patch (no-op) that serves as the identity element for the patch monoid.

**REQ-PATCH-007:** The system SHALL enforce associativity of patch composition — `(P1 ∘ P2) ∘ P3` SHALL equal `P1 ∘ (P2 ∘ P3)` for all valid patch triples.

**REQ-PATCH-008:** The system SHOULD provide property-based tests (via `proptest`) verifying commutativity, associativity, and identity axioms of the patch algebra.

**REQ-PATCH-009:** The system SHALL encode patches using FlatBuffers for zero-copy deserialization and schema evolution.

**REQ-PATCH-010:** The system SHALL represent patch metadata (touch sets, type, parent references) as FlatBuffers-encoded structures.

---

## Patch DAG (REQ-DAG)

**REQ-DAG-001:** The system SHALL maintain a directed acyclic graph (DAG) of patches as the authoritative version history.

**REQ-DAG-002:** The system SHALL guarantee that no cycles can be created in the DAG — all DAG mutation operations SHALL reject cycles.

**REQ-DAG-003:** The system SHALL support named branches as movable pointers to specific DAG nodes.

**REQ-DAG-004:** The system SHALL compute the Lowest Common Ancestor (LCA) of two DAG nodes for merge base identification.

**REQ-DAG-005:** The system SHALL compute the transitive patch set (all ancestors) for any given DAG node.

**REQ-DAG-006:** The system SHALL store the DAG topology persistently in the local metadata database (SQLite).

**REQ-DAG-007:** The system SHALL support creating, listing, renaming, and deleting named branches.

**REQ-DAG-008:** The system SHALL support detached HEAD mode where no named branch points to the current node.

**REQ-DAG-009:** The system SHALL provide a topological ordering of the DAG for log and history traversal.

---

## Metadata (REQ-META)

**REQ-META-001:** The system SHALL use SQLite in WAL (Write-Ahead Logging) mode for all local metadata storage.

**REQ-META-002:** The system SHALL track DAG topology, branch pointers, working set state, and configuration in the metadata database.

**REQ-META-003:** The system SHALL support schema migrations between versions using a versioned migration framework.

**REQ-META-004:** The system SHALL achieve sub-millisecond latency for DAG topology queries against the metadata database.

**REQ-META-005:** The system SHALL store all metadata in a `.suture/` directory within the project root, analogous to `.git/`.

**REQ-META-006:** The system SHALL persist repository configuration (hash algorithm, merge policy, driver settings) in the metadata database.

---

## Driver SDK (REQ-DRIVER)

**REQ-DRIVER-001:** The system SHALL define a `SutureDriver` trait providing `serialize`, `deserialize`, and `visualize` operations for format-specific semantic parsing.

**REQ-DRIVER-002:** The system SHALL provide the `serialize` operation that transforms a file at a given path into a `Patch` representing its semantic content.

**REQ-DRIVER-003:** The system SHALL provide the `deserialize` operation that applies a `Patch` to produce a file at a target path.

**REQ-DRIVER-004:** The system SHALL provide the `visualize` operation that produces a `VisualDiff` representation suitable for rendering in a UI.

**REQ-DRIVER-005:** The system SHALL include a reference driver implementation for OpenTimelineIO (`.otio`) as `suture-driver-otio`.

**REQ-DRIVER-006:** The system SHALL allow users to specify the active driver per file extension via repository configuration.

**REQ-DRIVER-007:** The system SHOULD provide PyO3 bindings to allow Python-based applications (e.g., DaVinci Resolve) to invoke `libsuture` directly.

---

## CLI (REQ-CLI)

**REQ-CLI-001:** The system SHALL provide a command-line interface supporting at minimum: `init`, `status`, `add`, `commit`, `branch`, `merge`, `log`, and `diff`.

**REQ-CLI-002:** The system SHALL use the `clap` crate for argument parsing with derive macros.

**REQ-CLI-003:** The system SHALL provide human-readable error messages for all failure modes, including suggested remediation actions.

**REQ-CLI-004:** The system SHALL provide a `--live` flag for the `status` command that displays a continuously updating status view.

**REQ-CLI-005:** The system SHALL support internationalization of CLI help text and error messages via the `fluent` crate (English, Chinese, Japanese minimum).

**REQ-CLI-006:** The system SHALL provide a `config` subcommand for repository-level configuration management.

**REQ-CLI-007:** The system SHALL provide a `key` subcommand for Ed25519 key pair generation, listing, and rotation.

**REQ-CLI-008:** The system SHALL display command execution timing in verbose mode for performance diagnostics.

---

## Security (REQ-SEC)

**REQ-SEC-001:** The system SHALL sign all patches with Ed25519 cryptographic keys.

**REQ-SEC-002:** The system SHALL maintain an immutable, append-only audit log of all patch signatures and DAG state transitions.

**REQ-SEC-003:** The system SHALL use TLS 1.3 for all network communication when the distributed Hub is implemented (future phase).

**REQ-SEC-004:** The system SHALL verify the Ed25519 signature of every patch on read to detect tampering.

**REQ-SEC-005:** The system SHALL support Ed25519 key rotation via a key chain model where historical signatures remain valid under their original keys.

**REQ-SEC-006:** The system SHALL securely zeroize key material from memory after use via the `zeroize` crate.

**REQ-SEC-007:** The system SHALL support optional TPM/Secure Enclave storage for Ed25519 private keys on supported platforms.

**REQ-SEC-008:** The system SHALL support air-gapped deployment mode where all cryptographic operations function without external network connectivity.

**REQ-SEC-009:** The system SHALL support export of the audit log in a format suitable for SOC2/ISO 27001 compliance reporting.

---

## Performance (REQ-PERF)

**REQ-PERF-001:** The system SHALL achieve merge latency of less than 10 milliseconds for a history of 10,000 patches.

**REQ-PERF-002:** The system SHALL achieve metadata lookup latency of less than 1 millisecond for DAG topology queries.

**REQ-PERF-003:** The system SHALL use SIMD-accelerated BLAKE3 via the `blake3` crate's native SIMD features.

**REQ-PERF-004:** The system SHOULD use Shared Memory (SHM) IPC for daemon-to-UI status communication when the daemon is implemented.

**REQ-PERF-005:** The system SHALL avoid blocking the calling thread during CAS write operations by performing I/O asynchronously.

---

## Build and Tooling (REQ-BUILD)

**REQ-BUILD-001:** The system SHALL use a Nix Flake for deterministic, reproducible build environments.

**REQ-BUILD-002:** The system SHALL target Rust edition 2024 on the stable channel.

**REQ-BUILD-003:** The system SHALL compile and run on Linux (x86_64 and aarch64), macOS (aarch64), and Windows (x86_64).

**REQ-BUILD-004:** The system SHALL pass all tests with `cargo test`, `cargo clippy`, and `cargo fmt --check` in CI.

**REQ-BUILD-005:** The system SHALL use the `zeroize` crate for all cryptographic key material to prevent secret leakage from memory.

---

## Exclusions — v0.1 Scope (REQ-EXCL)

**REQ-EXCL-001:** The system WILL NOT include the Virtual File System (NFSv4 loopback, ProjFS, FUSE3) in v0.1.

**REQ-EXCL-002:** The system WILL NOT include the distributed Hub with Raft consensus in v0.1.

**REQ-EXCL-003:** The system WILL NOT include S3 object storage or PostgreSQL backend in v0.1.

**REQ-EXCL-004:** The system WILL NOT include gRPC/QUIC networking in v0.1.

**REQ-EXCL-005:** The system WILL NOT include the Suture Desktop UI (Tauri) or Web Hub UI in v0.1.

**REQ-EXCL-006:** The system WILL NOT include enterprise RBAC/SSO integration in v0.1.

**REQ-EXCL-007:** The system WILL NOT include FIPS 140-3 certified hash mode in v0.1 (pluggable hash backend SHALL be implemented; SHA3-256 FIPS mode is deferred).

---

## Summary

| Category | Mandatory (SHALL) | Desired (SHOULD) | Exclusions (WILL NOT) |
|:---|:---:|:---:|:---:|
| REQ-CORE | 7 | 0 | 0 |
| REQ-CAS | 9 | 0 | 0 |
| REQ-PATCH | 9 | 1 | 0 |
| REQ-DAG | 9 | 0 | 0 |
| REQ-META | 6 | 0 | 0 |
| REQ-DRIVER | 6 | 1 | 0 |
| REQ-CLI | 7 | 0 | 0 |
| REQ-SEC | 9 | 0 | 0 |
| REQ-PERF | 4 | 1 | 0 |
| REQ-BUILD | 5 | 0 | 0 |
| REQ-EXCL | 0 | 0 | 7 |
| **Total** | **71** | **3** | **7** |
