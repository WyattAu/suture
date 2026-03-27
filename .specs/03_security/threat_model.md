# Suture STRIDE Threat Model v1.0

**Document ID:** TM-SEC-001  
**Version:** 1.0.0  
**Status:** APPROVED  
**Created:** 2026-03-27  
**Component Scope:** CAS, Patch-DAG, Metadata Store, CLI (v0.1)

---

## 1. System Boundary

### 1.1 In-Scope Components (v0.1)

| Component | Type | Trust Boundary |
|-----------|------|---------------|
| CAS (Content Addressable Storage) | Local filesystem | `.suture/objects/` |
| Patch-DAG | In-memory + SQLite | `.suture/metadata.db` |
| Metadata Store | SQLite (WAL mode) | `.suture/metadata.db` |
| CLI | User-facing binary | Process stdin/stdout |
| Compression Pipeline | Zstd (in-process) | Memory |

### 1.2 Out-of-Scope Components (Future Phases)

| Component | Phase | Notes |
|-----------|-------|-------|
| VFS (NFSv4/SMB3/ProjFS) | v0.3 | User-space filesystem server |
| Daemon | v0.3 | Background service, IPC attack surface |
| Hub (gRPC/QUIC) | v0.5 | Network attack surface, requires TLS 1.3 |
| Driver Plugins | v0.2 | Third-party `SutureDriver` implementations |
| Distributed Consensus (Raft) | v1.0 | Multi-node attack surface |

### 1.3 Trust Assumptions (v0.1)

1. **Single-user, single-machine.** No network exposure. The primary adversary is a
   local user with filesystem access to the `.suture/` directory.
2. **OS kernel is trusted.** Filesystem permissions are the primary access control mechanism.
3. **Rust type system and borrow checker provide memory safety.** No unsafe code in
   security-critical paths.
4. **BLAKE3 is collision-resistant** ($2^{128}$ security margin for 256-bit output).

---

## 2. Threat Analysis (STRIDE)

### 2.1 Spoofing

| Threat ID | Description | Component | Severity | Mitigation | Phase |
|-----------|-------------|-----------|----------|------------|-------|
| THM-S-001 | Impersonating another user's commits by forging the `author` field in a Patch struct | Patch Algebra | High | Ed25519 signatures on all patches bind author identity to patch content | v0.2 |
| THM-S-002 | Forged branch pointers redirecting a branch name to an attacker-controlled patch | DAG | Medium | Branch metadata includes creator signature; branch pointers reference signed patches | v0.2 |
| THM-S-003 | Spoofed BLAKE3 hash (preimage attack) to claim arbitrary content matches a known hash | CAS | Critical | BLAKE3 preimage resistance: $2^{256}$ work factor; not achievable with current or foreseeable compute | v0.1 (inherent) |

**v0.1 posture:** No cryptographic identity. The `author` field is a free-form string.
Acceptable because v0.1 is single-user. Mitigations for THM-S-001 and THM-S-002 are
deferred to v0.2 when Ed25519 signing is introduced.

---

### 2.2 Tampering

| Threat ID | Description | Component | Severity | Mitigation | Phase |
|-----------|-------------|-----------|----------|------------|-------|
| THM-T-001 | CAS blob poisoning: replace the file at `.suture/objects/ab/cdef...` with attacker content, causing `get_blob` to return wrong data | CAS | Critical | BLAKE3 integrity check on every `get_blob`: `H(retrieved) == requested_hash`. Mismatch returns `CasError::IntegrityCheckFailed` | v0.1 |
| THM-T-002 | DAG manipulation: inject a malicious patch node into the SQLite DAG table, corrupting merge history | DAG | Critical | Ed25519 patch signatures prevent insertion of unsigned patches (v0.2). v0.1: SHA-256 patch ID derived from content, tampering changes the ID and breaks all references | v0.1 (partial), v0.2 (full) |
| THM-T-003 | SQLite metadata tampering: direct modification of `metadata.db` to alter blob records, branch state, or config | Metadata | High | Filesystem permissions (0700 on `.suture/`). Future: HMAC on critical rows, append-only audit log | v0.1 |
| THM-T-004 | Touch-set forgery: craft a patch with an empty or manipulated touch set to bypass commutativity checks and force silent overwrites | Patch Algebra | Critical | Touch sets are **derived** from the patch payload by the driver, not user-supplied. The `SutureDriver::serialize()` computes the touch set. The patch algebra engine never trusts externally-provided touch sets | v0.1 |
| THM-T-005 | Conflict node manipulation: modify a stored Conflict to hide a real conflict or inject a false one | Patch Algebra | High | Conflict nodes are immutable once created. They are stored as FlatBuffers with BLAKE3 content addressing in the CAS. Tampering is detected by integrity check | v0.1 |
| THM-T-006 | Compressor bomb: supply a Zstd-compressed payload that decompresses to a size far exceeding the configured limit | CAS | Medium | Decompression size limit enforced before writing to disk. Configurable maximum (default: 1 GiB per blob) | v0.1 |

---

### 2.3 Repudiation

| Threat ID | Description | Component | Severity | Mitigation | Phase |
|-----------|-------------|-----------|----------|------------|-------|
| THM-R-001 | Author denies creating a patch | Patch Algebra | Medium | Ed25519 non-repudiation: patch signatures are cryptographically bound to author key. Cannot be forged without the private key | v0.2 |
| THM-R-002 | Denial of merge operation: user claims they did not merge branch B into A | DAG | Low | Append-only merge log in SQLite records every merge with timestamp, base branch, merged branch, and resulting patch set | v0.1 |
| THM-R-003 | Denial of blob deletion | CAS | Low | GC log records every garbage collection run with deleted blob hashes and timestamps | v0.1 |

**v0.1 posture:** Append-only logging provides basic audit trail. Cryptographic
non-repudiation (THM-R-001) requires Ed25519 and is deferred to v0.2.

---

### 2.4 Information Disclosure

| Threat ID | Description | Component | Severity | Mitigation | Phase |
|-----------|-------------|-----------|----------|------------|-------|
| THM-I-001 | Leak of project content via CAS: attacker reads `.suture/objects/` directly to access blob data | CAS | Medium | Filesystem permissions: `.suture/` directory created with mode 0700 (owner-only). Blobs are Zstd-compressed, providing obfuscation but not encryption | v0.1 |
| THM-I-002 | Leak of patch history: attacker reads SQLite metadata to see full patch DAG, author info, timestamps | Metadata | Medium | Filesystem permissions on `metadata.db`. Future: per-branch access control lists | v0.1 |
| THM-I-003 | Leak of branch structure to unauthorized collaborator | DAG | Low | Not applicable in v0.1 (single-user). Future: RBAC on branch visibility | Future |
| THM-I-004 | Timing side-channel in commutativity check: adversary infers touch-set intersection from response time | Patch Algebra | Low | Touch-set intersection is O(min(|T1|, |T2|)) — timing is data-dependent but the information disclosed (size of overlap) is not security-sensitive for local single-user use | v0.1 (accepted risk) |

---

### 2.5 Denial of Service

| Threat ID | Description | Component | Severity | Mitigation | Phase |
|-----------|-------------|-----------|----------|------------|-------|
| THM-D-001 | CAS disk exhaustion: store excessive or extremely large blobs to fill the filesystem | CAS | Medium | Configurable per-blob size limit (default: 1 GiB). Configurable total CAS size limit. `put_blob` rejects blobs exceeding the limit | v0.1 |
| THM-D-002 | DAG explosion: create millions of trivial patches to degrade merge performance | DAG | Medium | Configurable patch count limit per repository. Merge complexity is O(|a_only| × |b_only| × k̄) — warn when patch count exceeds threshold | v0.1 |
| THM-D-003 | SQLite lock contention: rapid concurrent writes to `metadata.db` causing WAL growth and lock timeouts | Metadata | Low | WAL mode with configured `busy_timeout`. Connection pooling via `SqlitePool`. Write serialization via the BlobStore write mutex | v0.1 |
| THM-D-004 | Zip bomb via compression: a small compressed blob that decompresses to enormous size | CAS | Medium | Decompression size limit checked before accepting the decompressed output. Configurable maximum decompressed size (default: 1 GiB) | v0.1 |
| THM-D-005 | Hash collision resource exhaustion: intentionally find BLAKE3 partial collisions to cause dedup failures | CAS | Low | BLAKE3 full 256-bit collision resistance ($2^{128}$). Not achievable. Partial-prefix collisions (for sharding) are not a vulnerability — they only affect directory balance, not correctness | v0.1 (inherent) |
| THM-D-006 | Virtual blob reference chain: create a chain of virtual blobs that reference each other, causing infinite materialization recursion | CAS | Medium | Maximum VRef chain depth (default: 16). Materialization rejects chains exceeding the limit | v0.1 |

---

### 2.6 Elevation of Privilege

| Threat ID | Description | Component | Severity | Mitigation | Phase |
|-----------|-------------|-----------|----------|------------|-------|
| THM-E-001 | CLI command injection: maliciously crafted branch names, patch data, or file paths cause unintended operations | CLI | High | All CLI arguments validated before processing. No shell execution. Branch names restricted to `[a-zA-Z0-9._/-]`. Paths resolved via `PathBuf::canonicalize` to prevent traversal | v0.1 |
| THM-E-002 | Path traversal via virtual blob registration: `register_virtual_blob` with a `../../etc/passwd` path reads arbitrary files | CAS | High | `register_virtual_blob` resolves and validates that the path is within the repository root or an explicitly-allowed external path. Rejects paths containing `..` components after canonicalization | v0.1 |
| THM-E-003 | Driver code execution: a malicious `SutureDriver` implementation executes arbitrary code during `serialize` or `deserialize` | Driver SDK | High | v0.1: Only the built-in OTIO driver is used (no third-party plugins). Future: drivers run in a sandboxed WASM or subprocess context with restricted filesystem access | v0.1 (mitigated by scope), Future (sandboxing) |
| THM-E-004 | SQLite injection: malicious patch data containing SQL is inserted into metadata queries | Metadata | Medium | All SQLite queries use parameterized statements via `rusqlite`. No string interpolation in SQL. Patch data is stored as BLOBs, not inline SQL values | v0.1 |

---

## 3. Attack Surface Summary

### 3.1 Primary Attack Surfaces (v0.1)

| # | Surface | Vector | Risk Level | Primary Control |
|---|---------|--------|-----------|----------------|
| 1 | CAS Filesystem | Physical access to `.suture/objects/` | Medium | BLAKE3 integrity verification |
| 2 | SQLite Database | Physical access to `.suture/metadata.db` | Medium | Filesystem permissions (0700) |
| 3 | CLI Arguments | User-supplied paths, branch names, patch data | High | Input validation, no shell execution |
| 4 | FlatBuffers Deserialization | Malformed patch payloads | Medium | FlatBuffers schema validation, bounds checking |
| 5 | Zstd Decompression | Crafted compressed payloads | Medium | Decompression size limit |

### 3.2 Future Attack Surfaces (Out of Scope)

| # | Surface | Phase | Planned Control |
|---|---------|-------|----------------|
| 6 | Driver Plugins | v0.2 | WASM sandbox, capability-based permissions |
| 7 | Daemon IPC (SHM/Unix sockets) | v0.3 | Access control on socket, message authentication |
| 8 | VFS Filesystem Operations | v0.3 | Kernel-level permission checks, path validation |
| 9 | gRPC/QUIC Network Endpoint | v0.5 | TLS 1.3, mTLS, token authentication |
| 10 | Raft Consensus Protocol | v1.0 | Node authentication, log integrity (Ed25519) |

---

## 4. Security Controls (v0.1)

### 4.1 Implemented Controls

| Control | Threat IDs Mitigated | Implementation |
|---------|---------------------|---------------|
| BLAKE3 content addressing | THM-T-001, THM-T-005, THM-S-003 | `hasher.rs` — verified on every `get_blob` |
| SHA-256 patch ID integrity | THM-T-002 (partial) | Patch ID = SHA-256 of serialized content |
| Filesystem permissions (0700) | THM-I-001, THM-I-002, THM-T-003 | `.suture/` created with restrictive permissions |
| Input validation on CLI args | THM-E-001 | Branch name regex, path canonicalization |
| No shell command execution | THM-E-001 | All operations are library calls, not subprocesses |
| Parameterized SQL queries | THM-E-004 | `rusqlite` with bound parameters exclusively |
| Decompression size limit | THM-D-004, THM-T-006 | Compressor enforces max decompressed size |
| Per-blob size limit | THM-D-001 | `put_blob` rejects oversized blobs |
| VRef chain depth limit | THM-D-006 | Max 16 levels of virtual blob indirection |
| Touch-set derivation (not user-supplied) | THM-T-004 | Driver computes touch sets; algebra engine never trusts external touch sets |
| Append-only merge log | THM-R-002 | SQLite INSERT-only table for merge records |
| Atomic writes (temp-then-rename) | THM-T-001 (crash safety) | Write to `.tmp`, then `rename` (atomic on POSIX) |

### 4.2 Deferred Controls

| Control | Threat IDs | Target Phase |
|---------|-----------|-------------|
| Ed25519 patch signing | THM-S-001, THM-S-002, THM-T-002, THM-R-001 | v0.2 |
| Per-branch access control | THM-I-002, THM-I-003 | v0.5 |
| Driver sandboxing (WASM) | THM-E-003 | v0.2 |
| TLS 1.3 for network | All network threats | v0.5 |
| RBAC for multi-user | THM-S-001, THM-I-002, THM-I-003 | v0.5 |
| Audit log with SOC2 export | THM-R-001, THM-R-002, THM-R-003 | v1.0 |
| HMAC on metadata rows | THM-T-003 | v0.3 |

---

## 5. Security Controls (Future)

### 5.1 Network Security (v0.5)

- **TLS 1.3** mandatory for all Daemon↔Hub communication.
- **mTLS** (mutual TLS) for node-to-node Raft communication.
- **Token-based authentication** for CLI↔Daemon IPC (HMAC-signed JWT with short TTL).
- **QUIC connection migration** resistance to off-path attacks.

### 5.2 Multi-User Access Control (v0.5)

- **RBAC** (Role-Based Access Control) with repository-level roles: Owner, Writer, Reader.
- **Branch-level permissions**: restrict read/write per branch.
- **Capability-based driver permissions**: each driver declares required filesystem paths and operations.

### 5.3 Driver Sandboxing (v0.2)

- **WASM runtime** (Wasmtime) for third-party drivers.
- **Capability model**: drivers request explicit filesystem paths, network access, and memory limits.
- **Resource limits**: max CPU time, max memory allocation per driver invocation.
- **No FFI**: drivers cannot call native code.

### 5.4 Audit Logging (v1.0)

- **Immutable, append-only log** of every DAG mutation, CAS write, and merge operation.
- **Signed log entries**: each entry HMAC'd with a per-repository key.
- **SOC2/ISO 27001 export**: structured JSON export for compliance audits.
- **Tamper detection**: log chain integrity verified via hash chaining.

### 5.5 Distributed Security (v1.0)

- **Raft log integrity**: every Raft log entry signed by the leader.
- **Node authentication**: Ed25519 node identities for cluster membership.
- **Split-brain detection**: lease-based leader election with heartbeat timeouts.
- **At-rest encryption**: optional AES-256-GCM encryption of CAS blobs and metadata.

---

## 6. Risk Acceptance

| Risk | Justification |
|------|--------------|
| No Ed25519 signing in v0.1 | Single-user, local-only. No multi-party trust required |
| No encryption at rest | Data sovereignty requirement (air-gapped mode) means keys cannot be centrally managed. Filesystem permissions provide adequate protection for local use |
| Timing side-channel in touch-set intersection (THM-I-004) | Information disclosed (overlap size) is not security-sensitive for local single-user use |
| No driver sandboxing in v0.1 | Only the built-in OTIO driver ships with v0.1. No third-party plugin loading |

---

## 7. Review History

| Date | Reviewer | Changes |
|------|----------|---------|
| 2026-03-27 | Security Engineering (Phase 3) | Initial threat model |

---

*End of TM-SEC-001*
