# Applicable Standards: Suture — Universal Semantic Version Control System

**Document ID:** SPEC-AS-001
**Status:** Draft
**Date:** 2026-03-27

---

## 1. Overview

This document identifies the international standards, frameworks, and specifications applicable to the design, implementation, and operation of Suture. Each standard is mapped to the project components it governs, with a priority classification based on the project's safety, security, and quality requirements.

**Priority Classification:**
- **P0 — Mandatory:** Required for correctness, safety, or legal compliance. Non-compliance is a release blocker.
- **P1 — Required:** Required for target market adoption or enterprise certification. Must be addressed before GA.
- **P2 — Recommended:** Best practices that improve quality, maintainability, or interoperability. Should be adopted where cost-effective.

---

## 2. Standards Catalog

### 2.1. ISO/IEC 12207 — Systems and Software Life Cycle Processes

| Attribute | Detail |
|:---|:---|
| **Standard** | ISO/IEC 12207:2017 (Systems and software engineering — Software life cycle processes) |
| **Scope** | Defines the technical and management processes for the full software lifecycle |
| **Priority** | P1 — Required |
| **Applicability** | Cross-cutting — governs all phases from requirements through decommission |
| **Key Requirements for Suture** | Requirements traceability (patch theory correctness ↔ requirements); configuration management (CAS as the configuration item store); verification & validation (formal proofs of commutativity); risk management (distributed consensus failure modes) |

**Application:** Suture's development lifecycle should map requirements from this document and the architectural spec through to test cases. The CAS and DAG together serve as the system's configuration management backbone — every patch is a tracked configuration item.

### 2.2. IEEE 1016 — Software Design Descriptions

| Attribute | Detail |
|:---|:---|
| **Standard** | IEEE 1016-2009 (Standard for Information Technology — System Design — Software Design Descriptions) |
| **Scope** | Prescribes the structure and content of software design documentation |
| **Priority** | P1 — Required |
| **Applicability** | Architecture specification, component design, interface contracts |
| **Key Requirements for Suture** | Decomposition description (crate structure: core, daemon, cli, driver-otio); interface descriptions (`SutureDriver` trait, gRPC service definitions); dynamic behavior (DAG state machine, Raft consensus protocol, lease lifecycle) |

**Application:** The architectural_spec.md and component-level design documents should conform to IEEE 1016 structure: design viewpoint, design elements, and rationale. The `SutureDriver` trait interface and gRPC protobuf definitions serve as the formal interface descriptions.

### 2.3. IEC 61508 — Functional Safety of Electrical/Electronic/Programmable Electronic Systems

| Attribute | Detail |
|:---|:---|
| **Standard** | IEC 61508 (all parts: 1-7) |
| **Scope** | Functional safety lifecycle for programmable electronic systems |
| **Priority** | P2 — Recommended (adapted for data integrity) |
| **Applicability** | Data integrity guarantees, patch theory correctness, CAS consistency |
| **Key Requirements for Suture** | Safety integrity levels mapped to data integrity levels (SIL 1 equivalent for non-destructive editing); failure mode analysis for DAG corruption; proven-in-use arguments for BLAKE3 and Ed25519; systematic capability for the Rust toolchain |

**Application:** While Suture is not a safety-critical system in the traditional sense (no risk of physical harm), the financial and creative data it manages can be mission-critical. IEC 61508 principles are adapted here as a **Data Integrity Assurance Framework**: patch commutativity must be provably correct, CAS writes must be atomic, and DAG state transitions must be failure-safe (no partial writes).

**Adaptation:** Formal verification via Lean 4 proofs for commutativity predicates; property-based testing (proptest) for CAS consistency invariants; exhaustive state machine testing for the DAG merge algorithm.

### 2.4. NIST SP 800-53 — Security and Privacy Controls

| Attribute | Detail |
|:---|:---|
| **Standard** | NIST SP 800-53 Rev. 5 |
| **Scope** | Comprehensive catalog of security and privacy controls for information systems |
| **Priority** | P0 — Mandatory (for enterprise deployment) |
| **Applicability** | Authentication, authorization, audit, cryptographic module usage, communication security |
| **Key Control Families for Suture** | |
| | AC (Access Control) — RBAC for Hub multi-tenant projects; lease-based access for binary assets |
| | AU (Audit and Accountability) — Immutable append-only patch ledger; Ed25519 signature chain; exportable for SOC2 |
| | SC (System and Communications Protection) — TLS 1.3 for all network traffic; QUIC encryption; TPM-bound key storage |
| | IA (Identification and Authentication) — Ed25519 cryptographic identity; SSO/SAML integration (enterprise) |
| | CM (Configuration Management) — CAS as the authoritative configuration store; signed DAG roots |
| | SI (System and Information Integrity) — BLAKE3 content verification; zero-trust blob validation on fetch |

### 2.5. NIST SP 800-202 — Key Management Guideline

| Attribute | Detail |
|:---|:---|
| **Standard** | NIST SP 800-57 Part 1 Rev. 5 (Recommendation for Key Management) |
| **Scope** | Best practices for cryptographic key generation, distribution, storage, and destruction |
| **Priority** | P0 — Mandatory |
| **Applicability** | Ed25519 key pair lifecycle for patch signing; Hub TLS certificate management |
| **Key Requirements for Suture** | Key generation: Ed25519 key pairs generated via cryptographically secure RNG (Rust `rand` with `OsRng` entropy source); Key storage: Optional TPM/Secure Enclave integration; hardware-bound on supported platforms; Key rotation: Support for key revocation and replacement without invalidating historical signatures (key metadata in DAG); Key destruction: Secure zeroization of key material from memory (`zeroize` crate) |

### 2.6. ISO/IEC 27001 — Information Security Management Systems

| Attribute | Detail |
|:---|:---|
| **Standard** | ISO/IEC 27001:2022 |
| **Scope** | Requirements for establishing, implementing, maintaining, and continually improving an ISMS |
| **Priority** | P1 — Required (for enterprise certification path) |
| **Applicability** | Organizational security posture; risk treatment; security controls for Hub deployment |
| **Key Requirements for Suture** | Annex A controls mapped to Suture's threat model: A.8 (Cryptography) — Ed25519 patch signing, BLAKE3 integrity; A.12 (Operations Security) — CAS access controls, VFS mount authorization; A.5 (Organizational) — Separation of duties in multi-tenant Hub; Data sovereignty — Air-gapped deployment mode for A.16 compliance |

### 2.7. FIPS 140-3 — Cryptographic Module Validation Program

| Attribute | Detail |
|:---|:---|
| **Standard** | FIPS 140-3 (Cryptographic Module Validation Program) |
| **Scope** | Security requirements for cryptographic modules used in federal systems |
| **Priority** | P2 — Recommended (for US government / defense contracts) |
| **Applicability** | BLAKE3 hash function, Ed25519 signature scheme |
| **Key Considerations** | BLAKE3 is not currently FIPS-approved (FIPS 202 approves SHA-3/SHAKE, not BLAKE3). A FIPS-compliant mode could use SHA3-256 as a fallback. Ed25519 is approved under FIPS 186-5. If FIPS 140-3 certification is required for target markets, Suture must implement a pluggable hash backend (BLAKE3 default, SHA3-256 for FIPS mode). |

---

## 3. Component-Standard Priority Matrix

| Standard | suture-core (Patch Engine) | suture-daemon (VFS) | suture-cli | suture-driver-otio | Suture Hub (Enterprise) | CAS Layer | gRPC/QUIC Transport |
|:---|:---|:---|:---|:---|:---|:---|:---|
| **ISO/IEC 12207** | P1 | P1 | P1 | P1 | P1 | P1 | P1 |
| **IEEE 1016** | P1 | P1 | P2 | P1 | P1 | P1 | P1 |
| **IEC 61508** (adapted) | P0 | P1 | P2 | P1 | P1 | P0 | P2 |
| **NIST SP 800-53** | P1 | P1 | P1 | P1 | P0 | P1 | P0 |
| **NIST SP 800-57** | P0 | P1 | P1 | P1 | P0 | P1 | P0 |
| **ISO/IEC 27001** | P1 | P1 | P2 | P1 | P0 | P1 | P0 |
| **FIPS 140-3** | P2 | P2 | P2 | P2 | P2 | P2 | P2 |

**Legend:**
- **P0 — Mandatory:** Required for initial release. Non-compliance blocks ship.
- **P1 — Required:** Required before GA. Must be planned into roadmap.
- **P2 — Recommended:** Adopt where cost-effective; track for future certification.

---

## 4. Additional Technical Specifications

### 4.1. Wire Protocol and Serialization Standards

| Specification | Applicability | Notes |
|:---|:---|:---|
| **gRPC / Protocol Buffers (proto3)** | Suture Hub ↔ Daemon communication | Primary RPC framework; tonic + prost crates |
| **QUIC (RFC 9000, 9001, 9002)** | Transport layer for all network communication | Implemented via `quinn` crate |
| **NFSv4 (RFC 8881)** | VFS loopback server on macOS/Linux | User-space implementation |
| **SMB3 (MS-SMB2)** | VFS loopback server on Windows | Alternative to ProjFS for cross-protocol support |
| **ProjFS (Microsoft)** | VFS on Windows | Native-speed file projection API |
| **FUSE3 (RFC pending)** | VFS fallback on Linux | Legacy environments |

### 4.2. Cryptographic Specifications

| Specification | Applicability | Implementation |
|:---|:---|:---|
| **BLAKE3** (blake3.org) | CAS content addressing | `blake3` crate; SIMD-accelerated; not FIPS-approved |
| **Ed25519** (RFC 8032) | Patch signing, identity | `ed25519-dalek` crate; FIPS 186-5 approved |
| **TLS 1.3** (RFC 8446) | All network traffic | `rustls` or `ring` via `quinn`/`tonic` |
| **XChaCha20-Poly1305** | Optional patch payload encryption | `chacha20poly1305` crate; for confidential patches |

### 4.3. Data Format Standards

| Specification | Applicability | Notes |
|:---|:---|:---|
| **OpenTimelineIO (OTIO)** | Reference video editorial driver | Industry-standard editorial interchange; ASWF project |
| **OpenDocument / OOXML** | Future spreadsheet/document drivers | `.xlsx`, `.docx` — well-documented XML-based formats |
| **USD (Universal Scene Description)** | Future 3D/CAD driver | Pixar's open-source scene description; complex composition model |
| **FlatBuffers** | Internal serialization for patches | Zero-copy, schema-evolvable; Google open-source |

---

## 5. Standard Conflict Analysis

### 5.1. BLAKE3 vs. FIPS 140-3

**Conflict:** BLAKE3 is not a FIPS-approved hash algorithm. FIPS 140-3 requires SHA-3 family (SHA3-256, SHA3-512) or SHA-2 family for cryptographic hashing.

**Resolution:** Implement a **pluggable hash backend** trait:

```
trait ContentHasher {
    fn hash(data: &[u8]) -> HashDigest;
    fn algorithm_id() -> AlgorithmId;
}
```

- Default backend: BLAKE3 (for performance).
- FIPS backend: SHA3-256 (for FIPS 140-3 compliance).
- Algorithm ID is stored alongside each CAS entry, allowing mixed-mode repositories during migration.
- Hash algorithm is a repository-level configuration, set at `suture init --hash-algorithm blake3|sha3-256`.

### 5.2. Deterministic Merging vs. Enterprise Locking (NIST SP 800-53 AC-4)

**Conflict:** Patch theory enables conflict-free concurrent editing (no locks needed for commuting patches). However, NIST SP 800-53 AC-4 (Information Flow Enforcement) may require mandatory access controls that restrict concurrent modification in certain enterprise contexts (e.g., regulated financial models under SOX).

**Resolution:** Suture supports **dual-mode access control**:
- **Optimistic mode** (default): Patch-based merging with first-class conflict nodes. No locks for commuting patches.
- **Pessimistic mode** (configurable per-project or per-file): Lease-based locking (DLM) for non-mergeable assets. All modifications require lease acquisition via Raft.
- Mode is a project-level policy, configurable via `suture config set merge.policy optimistic|pessimistic`.

### 5.3. Air-Gapped Deployment vs. Cloud-Native Features (ISO 27001 A.16)

**Conflict:** Suture's Hub architecture targets cloud deployment (S3, PostgreSQL, Redis). However, data sovereignty requirements (defense, classified media, certain financial regulations) require air-gapped, on-prem-only operation.

**Resolution:** Suture Hub is designed with **deployment topology abstraction**:
- **Connected mode:** Full Hub with S3 backend, PostgreSQL, Redis, and QUIC sync.
- **Air-gapped mode:** Local Hub with embedded PostgreSQL, local filesystem CAS, and no external network dependency. Synchronization between air-gapped nodes uses physical media (sneakernet) with signed bundle export/import.
- All cryptographic operations (signing, verification) function identically in both modes.

### 5.4. Ed25519 Key Rotation vs. Immutable Audit Trail (NIST SP 800-57 vs. NIST SP 800-53 AU)

**Conflict:** NIST SP 800-57 requires periodic key rotation. However, Suture's immutable audit ledger is anchored to Ed25519 public keys — rotating keys could create ambiguity about historical signature validity.

**Resolution:** Key rotation is implemented as a **key chain**:
- Each key has a metadata record: `{key_id, public_key, created_at, revoked_at, successor_id}`.
- Historical patches remain signed with their original key. Verification checks the key chain for validity at the time of signing.
- New patches after rotation use the successor key.
- The key chain itself is embedded in the DAG as a special key-management patch type, inheriting immutability.
- Revocation does not invalidate historical signatures — it only prevents future use.

### 5.5. Zero-Copy Performance vs. Safety (IEC 61508 Adapted)

**Conflict:** Zero-copy data paths (Flatbuffers, mmap, Reflink) bypass Rust's ownership model in favor of raw pointer manipulation for performance. This creates potential for undefined behavior if invariants are violated.

**Resolution:**
- All zero-copy access is encapsulated in `unsafe` blocks with explicit safety invariants documented in code comments.
- Property-based tests (proptest) verify that zero-copy reads produce identical results to safe deserialization paths.
- The `loom` crate is used for concurrency testing of SHM and lock-free data structures to verify memory ordering guarantees.
- Miri (Rust UB detector) is run in CI for all code paths containing `unsafe`.

---

## 6. Compliance Roadmap

| Phase | Standards Addressed | Deliverables |
|:---|:---|:---|
| **Phase 1: Foundation** | ISO/IEC 12207, IEEE 1016 | Requirements traceability matrix; design documentation per IEEE 1016 |
| **Phase 2: Core Engine** | IEC 61508 (adapted), NIST SP 800-57 | Formal commutativity proofs (Lean 4); property-based tests for CAS; Ed25519 key management implementation |
| **Phase 3: Enterprise** | NIST SP 800-53, ISO/IEC 27001 | RBAC implementation; immutable audit ledger; SOC2 export format; air-gapped deployment guide |
| **Phase 4: Certification** | FIPS 140-3 | Pluggable hash backend (SHA3-256); FIPS mode documentation; module validation submission |
