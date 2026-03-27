# Standard Conflicts and Resolutions: Suture — Universal Semantic Version Control System

**Document ID:** SPEC-SC-001
**Status:** Draft
**Date:** 2026-03-27

---

## 1. BLAKE3 vs. FIPS 140-3

### Context

BLAKE3 is the default content hash algorithm for Suture's Content Addressable Storage. It provides SIMD-accelerated throughput exceeding 1 GB/s and is widely regarded as cryptographically secure. However, FIPS 140-3 (Cryptographic Module Validation Program) does not currently approve BLAKE3 — only the SHA-2 family (SHA-256, SHA-512) and SHA-3 family (SHA3-256, SHA3-512) are FIPS-approved hash algorithms.

### Conflict Description

Target markets for Suture include US government agencies, defense contractors, and financial institutions subject to FIPS 140-3 compliance mandates. Using BLAKE3 as the sole hash algorithm would render Suture non-compliant for these deployments, blocking a significant enterprise market segment.

Simultaneously, BLAKE3 offers 2-4x the throughput of SHA3-256 on modern hardware. Mandating SHA3-256 as the default would impose a performance penalty on all users for the benefit of a subset.

### Resolution

Implement a **pluggable hash backend** via a `ContentHasher` trait:

```rust
trait ContentHasher: Send + Sync {
    fn hash(data: &[u8]) -> HashDigest;
    fn algorithm_id() -> AlgorithmId;
    fn name() -> &'static str;
}
```

- **Default backend:** BLAKE3 (for maximum performance on all platforms).
- **FIPS backend:** SHA3-256 via the `sha3` crate (for FIPS 140-3 compliance).
- Hash algorithm is a **repository-level configuration** set at `suture init --hash-algorithm blake3|sha3-256`.
- The `AlgorithmId` is stored alongside each CAS entry, allowing mixed-mode inspection during migration.
- Algorithm selection is immutable after `suture init` to prevent hash collision across algorithms within a single repository.
- FIPS-certified builds link against validated cryptographic modules; the trait abstraction enables swapping at compile time.

### ADR Reference

**ADR-001: Pluggable Hash Backend for FIPS 140-3 Compliance**

### Priority

P2 — Recommended. The pluggable backend SHALL be implemented in v0.1. The SHA3-256 FIPS backend and formal FIPS certification submission are deferred to a future phase.

---

## 2. Optimistic Merge vs. Enterprise Locking Requirements

### Context

Suture's patch theory enables optimistic, lock-free concurrent editing — patches with disjoint touch sets commute and merge deterministically without coordination. This is the default and preferred mode for creative workflows where multiple users edit different parts of a timeline, spreadsheet, or scene.

However, NIST SP 800-53 AC-4 (Information Flow Enforcement) and certain enterprise governance policies (SOX for financial models, ITAR for defense content) may require mandatory access controls that restrict concurrent modification of the same resource, regardless of semantic commutativity.

### Conflict Description

The optimistic merge model assumes that users can freely edit and conflicts are resolved at merge time. Enterprise locking models require that users acquire exclusive access before modifying a resource. These are fundamentally different concurrency control strategies:

- **Optimistic:** No coordination overhead; conflicts are detected late (at merge); requires first-class conflict resolution.
- **Pessimistic:** Coordination overhead (lock acquisition/release); conflicts are detected early (at lock time); no merge conflicts but potential for deadlocks and blocked workflows.

For certain regulated industries, the optimistic model may be deemed non-compliant because it allows concurrent modification of the same logical resource without prior authorization.

### Resolution

Suture supports **dual-mode access control**, configurable at the project and file level:

- **Optimistic mode** (default): Patch-based merging with first-class conflict nodes. No locks required for commuting patches. Conflict nodes preserve full data from both sides.
- **Pessimistic mode** (configurable): Lease-based locking via the distributed lock manager (DLM). Modifications require lease acquisition before patch creation. Leases have TTL-based expiration with heartbeat enforcement.
- Mode is configured via `suture config set merge.policy optimistic|pessimistic` and can be overridden per-file via `.sutureignore`-style patterns.
- In pessimistic mode, the DLM is backed by Raft consensus in the Hub (future phase). For v0.1 local-only mode, pessimistic locking uses filesystem-based advisory locks.
- The pessimistic mode does not replace patch theory — patches are still the unit of versioning. Locks only gate the *creation* of patches, not their *storage* or *merging*.

### ADR Reference

**ADR-002: Dual-Mode Merge Policy (Optimistic vs. Pessimistic)**

### Priority

P0 — Critical. Optimistic mode is the default and must ship in v0.1. Pessimistic mode with filesystem-based advisory locks should be available in v0.1. Raft-backed DLM is deferred to the Hub phase.

---

## 3. Air-Gap Mode vs. Cloud Sync Features

### Context

Suture's architecture includes a cloud-native Hub component (S3 storage, PostgreSQL, Redis, gRPC/QUIC sync). This enables real-time multi-user collaboration, global CAS deduplication, and centralized administration — essential features for studio and enterprise deployments.

However, data sovereignty regulations (defense classification, ITAR content, certain financial regulations under GDPR/SOX) and physical security policies require air-gapped deployment where the Suture instance has zero network connectivity. Defense contractors, classified media production facilities, and certain financial institutions operate in environments where cloud connectivity is prohibited.

### Conflict Description

The Hub architecture assumes persistent network connectivity for:
- CAS blob synchronization to S3
- Metadata replication to PostgreSQL
- Real-time liveness tracking via Redis
- gRPC/QUIC streaming for live collaboration
- TLS certificate validation for authentication

An air-gapped deployment must function with none of these capabilities while maintaining full local version control functionality and cryptographic integrity.

### Resolution

Suture is designed with **deployment topology abstraction**:

- **Connected mode:** Full Hub with S3, PostgreSQL, Redis, and QUIC sync. All features available.
- **Air-gapped mode:** Local-only operation. CAS uses local filesystem storage. Metadata uses local SQLite. No external network dependency. All cryptographic operations (signing, verification) function identically.
- **Sneakernet transfer (bridge mode):** Air-gapped nodes can export signed bundle archives (tar.zst with Ed25519-signed manifests) to physical media. Connected nodes can import these bundles, verify signatures, and merge into the global Hub.
- The `suture bundle export` and `suture bundle import` commands handle sneakernet transfer with full cryptographic verification.
- All repository operations (`init`, `add`, `commit`, `branch`, `merge`, `log`, `diff`) are fully functional in air-gapped mode.
- Air-gapped mode is the default for v0.1 (no Hub exists yet). The abstraction ensures that adding Hub connectivity in future phases does not require changes to the local workflow.

### ADR Reference

**ADR-003: Deployment Topology Abstraction (Connected vs. Air-Gapped)**

### Priority

P0 — Critical. Air-gapped mode is the only mode for v0.1. Bundle export/import should be available by v0.1 to prepare for future Hub integration.

---

## 4. Key Rotation vs. Immutable Audit Trail

### Context

NIST SP 800-57 (Key Management) requires periodic cryptographic key rotation to limit the exposure window of compromised keys. Suture's security model anchors every patch to an Ed25519 signature, creating an immutable audit trail.

The audit trail's integrity depends on the verifiability of historical signatures. If a key is rotated and the old key is destroyed, historical signatures become unverifiable — breaking the audit chain.

### Conflict Description

- **Key rotation** (NIST SP 800-57): Old keys must be retired and replaced on a regular schedule (typically 1-2 years for Ed25519). Destroying old key material reduces the attack surface.
- **Immutable audit trail** (NIST SP 800-53 AU): Every patch must be attributable to a specific identity via a verifiable signature. The audit chain must be intact for the full history of the repository.

Destroying old keys breaks audit trail verifiability. Keeping old keys indefinitely contradicts key management best practices and increases the risk of key compromise over time.

### Resolution

Key rotation is implemented as a **key chain** embedded in the DAG:

```
KeyChain {
    keys: [
        { key_id: "k1", public_key: "...", created_at: T0, revoked_at: T1, successor_id: "k2" },
        { key_id: "k2", public_key: "...", created_at: T1, revoked_at: null, successor_id: null },
    ]
}
```

- Each key has metadata: `key_id`, `public_key`, `created_at`, `revoked_at`, `successor_id`.
- Historical patches remain signed with their original key. The public key is retained in the key chain for verification. Only the **private key** is destroyed on rotation.
- Verification checks the key chain to determine which public key was valid at the time of signing (based on `created_at`/`revoked_at`).
- The key chain itself is stored as a special patch type in the DAG, inheriting immutability. Key rotation events are part of the versioned history.
- Revocation does not invalidate historical signatures — it only prevents future use of the private key.
- The `suture key rotate` command handles the full rotation workflow: generate new key, create key-chain patch, prompt for secure deletion of old private key.
- Key compromise (emergency revocation) is supported: `suture key revoke --compromised` immediately revokes the key and flags all patches signed after a configurable grace window for re-review.

### ADR Reference

**ADR-004: Key Chain Model for Ed25519 Key Rotation with Audit Trail Preservation**

### Priority

P0 — Critical. Key chain model must be implemented in v0.1. Emergency revocation is P1.

---

## 5. Zero-Copy Reflinks vs. Safety and Recovery Guarantees

### Context

Suture targets multi-terabyte creative datasets (4K/8K video, multi-layer EXR sequences). Copying this data for branching would be prohibitively expensive in time and storage. Reflinks (Copy-on-Write via `FICLONERANGE` on XFS/Btrfs, `clonefile` on APFS, `FSCTL_DUPLICATE_EXTENTS` on ReFS) enable instantaneous branching by creating filesystem-level CoW references instead of physical copies.

However, reflinks operate at the filesystem level and are invisible to application-level integrity checks. A CoW breakpoint (modification after reflink) creates a new physical copy, but filesystem-level corruption (bit rot, power failure during CoW) could affect multiple branches that share the same underlying extents.

### Conflict Description

- **Zero-copy performance:** Reflinks provide O(1) branching time and zero additional storage until modification. Essential for the "instantaneous branching" requirement for multi-TB datasets.
- **Safety guarantees:** CAS integrity relies on BLAKE3 hash verification. If a reflinked blob is corrupted at the filesystem level (before CAS detects it), all branches referencing that blob are affected. Reflinks bypass application-level checksumming during the copy operation itself.
- **Recovery:** If a CoW operation is interrupted (power failure), the filesystem journal should recover, but the recovery semantics vary between XFS, Btrfs, and APFS. Suture cannot control filesystem-level recovery.
- **Portability:** Reflinks are not available on all filesystems (ext4 without bigalloc, FAT32, NFS without server-side support). Suture must degrade gracefully.

### Resolution

Suture uses a **layered integrity strategy**:

1. **CAS is the source of truth:** All integrity verification is at the BLAKE3 hash level, not the filesystem level. After any read, the blob's hash is verified against its CAS key. This catches corruption regardless of whether the blob was copied or reflinked.
2. **Reflinks are a performance optimization, not a correctness mechanism:** When creating a branch, Suture attempts a reflink first. If the filesystem does not support it, Suture falls back to a physical copy. The CAS hash verification layer is identical in both cases.
3. **Eager verification after reflink:** After branching via reflink, Suture performs a background BLAKE3 verification pass on all reflinked blobs to confirm filesystem-level integrity before the branch is made available to the user.
4. **Copy-on-write isolation:** When a blob is modified on one branch, the CoW breakpoint creates an independent physical copy. The other branch's blob remains at the original extent. CAS detects this because the modified blob gets a new BLAKE3 hash.
5. **Filesystem capability detection:** At `suture init`, Suture probes the filesystem for reflink support (`ioctl(FICLONERANGE)` on Linux, `clonefile` on macOS, `FSCTL_DUPLICATE_EXTENTS` on Windows). The result is stored in repository configuration and used to select the branching strategy.
6. **Recovery tooling:** `suture fsck` performs a full CAS integrity scan, detecting and reporting any blobs whose BLAKE3 hash does not match their stored key. For reflinked repositories, this also detects shared-extent corruption.

### ADR Reference

**ADR-005: Reflink Strategy with Layered CAS Integrity Verification**

### Priority

P1 — High. Physical copy fallback is P0 (must ship in v0.1). Reflink optimization and background verification are P1. `suture fsck` is P0.
