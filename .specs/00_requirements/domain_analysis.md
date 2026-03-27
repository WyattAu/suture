# Domain Analysis: Suture — Universal Semantic Version Control System

**Document ID:** SPEC-DA-001
**Status:** Draft
**Date:** 2026-03-27

---

## 1. Primary Domain

Suture operates in the domain of **Semantic Version Control for Non-Textual Data**. Unlike traditional VCS (Git, Mercurial, Subversion) designed for source code — where lines of text provide natural merge granularity — Suture targets structured binary formats where merge semantics must be derived from the data model itself.

**Core Problem:** Professional creative and analytical tools (NLEs, spreadsheets, CAD) produce files that are opaque to line-based diff/merge. Concurrent collaboration on these files results in either merge paralysis (full-file conflicts) or destructive manual overrides, costing studios and enterprises millions in rework.

**Suture's Resolution:** Model files as sequences of commutative operations (patches) over a directed acyclic graph (DAG), enabling deterministic, conflict-free merging at the semantic level.

---

## 2. Subdomains

### 2.1. Patch Theory (Applied Category Theory)

Suture's versioning model is grounded in the algebraic properties of patches:

- **Commutativity:** Patches P1 and P2 commute when P1 ∘ P2 = P2 ∘ P1. This holds when patches operate on non-overlapping semantic regions (e.g., editing cell A1 vs. cell B2 in a spreadsheet).
- **Associativity:** (P1 ∘ P2) ∘ P3 = P1 ∘ (P2 ∘ P3), enabling parallel application of independent patch sequences.
- **Identity:** A null patch (no-op) serves as the identity element for every merge operation.
- **Conflict as a First-Class Citizen:** Non-commuting patches produce explicit conflict nodes in the DAG rather than corrupting state. Conflicts are resolvable at the application layer via driver-specific heuristics.

**Formal Model:**
- Project state S_n = S_0 + {P_1, P_2, ..., P_n} where addition is set-union under commutativity constraints.
- Merge M(A, B) = patches(A) ∪ patches(B) where patches that do not commute with counterparts in the opposing set are flagged.

### 2.2. Content Addressable Storage (CAS)

All blobs (file contents, patch metadata, driver IR) are addressed by their BLAKE3 hash. This provides:

- **Deduplication:** Identical content across branches, projects, and users maps to a single physical blob.
- **Integrity:** Any tampering with blob content invalidates the hash, detectable in O(1).
- **Garbage Collection Reachability:** Unreferenced blobs are reclaimable via DAG reachability analysis.

### 2.3. Distributed Systems

Suture's Hub architecture requires consensus, replication, and partition tolerance:

- **Raft Consensus:** Leader election for global state coordination, lease management, and atomic lock acquisition.
- **QUIC Transport:** Multiplexed, encrypted, UDP-based transport that eliminates TCP head-of-line blocking — critical for remote editors on lossy networks.
- **Eventual Consistency for CAS:** CAS blobs are write-once, globally addressable, and converge via content identity rather than consensus.

### 2.4. File System Virtualization

Suture presents a unified logical filesystem to applications while mapping to heterogeneous physical storage:

- **NFSv4 Loopback (macOS/Linux):** User-space NFS server translating logical paths to physical paths at syscall intercept time.
- **Windows ProjFS:** Native Projected File System API integration for zero-overhead virtualization on Windows.
- **FUSE3 Fallback:** For legacy Linux environments where NFS loopback is unavailable.
- **Reflink/Copy-on-Write:** Kernel-level CoW (FICLONERANGE on XFS/Btrfs, APFS, ReFS) enables instantaneous branching of multi-terabyte datasets without data duplication.

---

## 3. Stakeholder Analysis

| Stakeholder | Role | Primary Needs | Pain Points Addressed | Priority |
|:---|:---|:---|:---|:---|
| **Media Professionals** (Editors, Colorists, VFX Artists) | End users editing timelines, compositions, and projects in NLEs | Non-destructive concurrent editing; instant branching of multi-TB projects; zero-downtime VFS access | Merge paralysis on .drp, .aep, .prproj files; broken media links across machines; hours of manual conflict resolution | P0 — Critical |
| **VFX / Animation Studios** (Pipeline TDs, Supervisors) | System integrators managing multi-tool pipelines (Resolve, Nuke, Maya, Houdini) | Semantic diffing of scene graphs; USD composition versioning; deterministic replay of pipeline operations | No version control for binary-heavy formats; artist collision on shared shots; non-reproducible renders | P0 — Critical |
| **Financial Analysts** (Quants, Modelers) | Users of complex Excel/financial models requiring audit trails | Cell-level versioning of spreadsheets; cryptographically signed change history; regulatory compliance export | Excel's limited revision history; no merge capability for concurrent model edits; audit trail gaps for SOX compliance | P1 — High |
| **Enterprise IT** (DevOps, Security, Compliance) | Operators managing deployment, access control, and compliance | RBAC/SSO integration; air-gapped deployment support; immutable audit logs; SOC2/ISO 27001 export | Git LFS operational complexity; lack of cryptographic identity in legacy VCS; compliance gaps in creative tooling | P1 — High |
| **Open-Source Contributors** | Developers building Suture drivers, plugins, and integrations | Clean driver SDK trait interface; comprehensive API docs; idiomatic Rust patterns | Steep learning curve for domain-specific patch theory; lack of reference implementations | P2 — Medium |
| **Cloud / SaaS Operators** | Hosters of the Suture Hub for multi-tenant studios | Horizontal scaling of Hub; S3 backend integration; PostgreSQL sharding; tenant isolation | On-prem vendor lock-in; no cloud-native VCS for non-code assets | P2 — Medium |

---

## 4. Domain Complexity Assessment

| Dimension | Complexity | Justification |
|:---|:---|:---|
| **Mathematical** | Very High | Commutative patch theory requires correct algebraic implementation; incorrect commutativity checks cause silent data corruption |
| **Concurrency** | Very High | Lock-free DAG traversal, SHM IPC, multi-writer SQLite WAL, distributed Raft consensus — all must be formally correct |
| **File System** | High | Three distinct virtualization backends (NFSv4, ProjFS, FUSE3) with OS-specific behavior and edge cases |
| **Network** | High | QUIC stream multiplexing, gRPC codegen, NAT traversal, lease heartbeats with sub-2s timeout windows |
| **Security** | High | Ed25519 key management, TLS 1.3, TPM integration, immutable audit ledger — cryptographic correctness is safety-critical |
| **Data Format** | Medium-High | Each driver (OTIO, XLSX, USD) has its own semantic model; correctness depends on domain expertise per format |
| **Serialization** | Medium | Flatbuffers schema evolution, Zstd compression tuning, zero-copy invariants |
| **User Experience** | Medium | CLI ergonomics (clap), VFS mount UX, status ticker — must feel instant to creative professionals |

**Overall Assessment:** This is a systems-programming project of very high complexity, comparable in scope to building a distributed database with a custom file system layer. Formal verification of critical paths (patch commutativity, CAS integrity, Raft state transitions) is strongly recommended.

---

## 5. Multi-Lingual Requirements

Suture targets global creative and financial markets. The following language support is required:

| Language | Code | Priority | Scope | Notes |
|:---|:---|:---|:---|:---|
| **English** | EN | P0 — Primary | All documentation, CLI, error messages, API | Default language for all interfaces |
| **Chinese (Simplified)** | ZH-CN | P1 — High | CLI help text, error messages, driver SDK docs | Critical for Chinese film/animation studios (basefx, More VFX) and Shenzhen financial sector |
| **Japanese** | JA | P1 — High | CLI help text, error messages | Essential for Japanese anime studios (Toei, Madhouse) and Tokyo-based post-production houses |
| **Korean** | KO | P2 — Medium | CLI help text, error messages | Growing market for Seoul-based VFX and game cinematics |

**Implementation Strategy:**
- Use the `fluent` crate (Mozilla's localization system) for i18n of CLI strings and error messages.
- Store translation files (`.ftl`) in a `locales/` directory at the crate root.
- CLI framework (`clap`) supports custom help templates that can be localized.
- Flatbuffers-generated code is language-agnostic; no localization needed for wire formats.

---

## 6. Technology Landscape Analysis

### 6.1. Direct Predecessors and Inspirations

| System | Approach | Relevance to Suture | Key Differences |
|:---|:---|:---|:---|
| **Pijul** | Commutative patch theory for text | Foundational theoretical model | Text-only; no binary/semantic awareness; no VFS; no enterprise features |
| **Darcs** | Theory of patches (earlier formalism) | Historical precedent for patch algebra | Performance issues with large repositories; no binary support; largely dormant |
| **Git** | Snapshot-based (content-addressed) | Dominant VCS; sets user expectations | Binary blob treatment causes merge paralysis; no semantic layer; LFS is bolt-on |

### 6.2. Enterprise / Industry-Specific Systems

| System | Domain | Relevance to Suture | Key Differences |
|:---|:---|:---|:---|
| **Perforce (Helix Core)** | Game dev, VFX studios | Industry standard for large binary assets | Centralized; lock-based (no concurrent editing); expensive licensing; no semantic merging |
| **Avid NEXIS / bin-locking** | Video editorial | Current industry workflow for Resolve/Premiere | File-level locking only; no branching; no merge capability; vendor-locked to Avid ecosystem |
| **ShotGrid (Autodesk)** | VFX production tracking | Project management context for Suture Hub | Asset management, not version control; no patch theory; no VFS |
| **Git LFS** | Binary asset tracking in Git | Addresses storage bloat for binaries | Still snapshot-based; merge conflicts on LFS pointers; pointer corruption risk |

### 6.3. Adjacent Technologies

| Technology | Relevance | Integration Opportunity |
|:---|:---|:---|
| **OpenTimelineIO (OTIO)** | Industry-standard editorial interchange format | Reference driver implementation; primary validation target for patch theory |
| **Universal Scene Description (USD)** | Pixar's scene composition format | Future driver for 3D/VFX pipeline versioning |
| **Excel Open XML (.xlsx)** | Financial modeling standard | Cell-level patch granularity is well-defined (row/column intersection) |
| **BLAKE3** | Cryptographic hash function | SIMD-accelerated, parallelizable, proven — ideal for CAS addressing |

---

## 7. Key Domain Constraints

### 7.1. Determinism (Non-Negotiable)

Every operation in Suture must be **deterministic and idempotent**:
- Given the same set of patches in any valid order, the resulting state must be identical.
- Hash computation (BLAKE3) must produce identical output for identical input across all platforms.
- DAG traversal and merge resolution must be order-independent for commuting patches.

**Rationale:** Non-determinism in a version control system leads to divergent histories, undetectable data corruption, and loss of audit integrity. For HFT-grade systems, this is a correctness invariant, not a performance optimization.

### 7.2. Zero-Copy Data Paths

- **Flatbuffers** enable direct memory-mapped access to serialized patch data without deserialization.
- **Reflink/Copy-on-Write** at the filesystem layer eliminates data duplication during branching.
- **SHM IPC** avoids context-switch overhead for nanosecond-level status queries between daemon and UI.

**Rationale:** Creative datasets (4K/8K video, multi-layer EXR sequences) are too large to copy. Any copy operation on the critical path introduces latency that disrupts the creative workflow.

### 7.3. Sub-Millisecond Latency for Metadata Operations

| Operation | Target Latency | Measurement Point |
|:---|:---|:---|
| File lock status check | < 500ns | SHM resident hash map lookup |
| BLAKE3 hash (256-bit) | ~350ns per 64-byte block | SIMD-accelerated single core |
| Patch commutativity check | < 1μs | Flatbuffers zero-copy field comparison |
| DAG merge (100 patches) | < 10ms | Set-union with commutativity filter |
| VFS path translation | < 1μs | O(1) hash map in shared memory |

**Rationale:** Metadata operations must be invisible to the user. Any perceptible delay in status checks, lock acquisition, or VFS path resolution breaks the "invisible VFS" contract and causes user distrust.

### 7.4. Cryptographic Integrity

- Every patch must be signed with **Ed25519**.
- The audit ledger must be **append-only and tamper-evident**.
- CAS blob identity (BLAKE3 hash) serves as both address and integrity check.
- Key material must be storable in hardware security modules (TPM/Secure Enclave).

### 7.5. Platform Ubiquity

Suture must support macOS, Linux (x86_64 + aarch64), and Windows with feature parity. Docker and WSL2 are first-class deployment targets. This constrains VFS implementation to cross-platform abstractions and requires per-OS testing matrices.

---

## 8. Domain Vocabulary

| Term | Definition |
|:---|:---|
| **Patch** | A commutative, cryptographically signed operation that transforms project state |
| **Patch-DAG** | The directed acyclic graph of patches representing full project history |
| **Suture** | A merge operation in Suture (noun); the act of merging (verb) |
| **Driver** | A plugin implementing the `SutureDriver` trait for a specific file format |
| **CAS** | Content Addressable Storage — blob store keyed by BLAKE3 hash |
| **VFS** | Virtual File System — the user-space file virtualization layer |
| **Hub** | The enterprise coordination service (Raft + PostgreSQL + S3) |
| **Lease** | A time-bound exclusive lock on a non-mergeable resource |
| **SHM** | Shared Memory — zero-copy IPC mechanism for daemon-to-UI communication |
| **IR** | Intermediate Representation — the driver-specific serialized form of a patch |
