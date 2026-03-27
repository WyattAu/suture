---
document_id: YP-ALGEBRA-PATCH-002
version: 1.0.0
status: DRAFT
domain: Storage Systems
subdomains: [Cryptography, Data Compression, Serialization]
created: 2026-03-27
author: DeepThought
confidence_level: 0.95
tqa_level: 4
---

# YP-ALGEBRA-PATCH-002: Serialization and Content Addressing

## 1. Executive Summary

Suture's Content-Addressable Storage (CAS) layer is the foundation for all deduplication,
integrity verification, and zero-copy branching. Every blob stored in the CAS is identified
by its cryptographic hash, computed via BLAKE3. Blobs are optionally compressed with Zstd
before storage, and transparently decompressed on retrieval.

This Yellow Paper defines the formal properties of the CAS: content addressing, compression
round-trip guarantees, deduplication correctness, and the virtual blob mechanism for
lazy-referencing large external files without copying them into the store.

**Scope:**
- Blob hashing, compression, storage, and retrieval.
- Deduplication via content-address identity.
- Virtual blobs for zero-copy references to external large files.
- Wire format overview (FlatBuffers schema references deferred to implementation specs).

**Out of Scope:**
- Distributed CAS replication and consistency (→ YP-DIST-CONSENSUS-001).
- Cryptographic signing of blobs (→ YP-SEC-001).
- FlatBuffers schema definition (→ implementation specs).

---

## 2. Nomenclature

| Symbol | Description | Type |
|--------|-------------|------|
| $\text{Blob}$ | Opaque byte sequence | $\text{Blob} \in \{0,1\}^*$ |
| $H(S)$ | BLAKE3-256 hash of byte sequence $S$ | $H : \{0,1\}^* \to \{0,1\}^{256}$ |
| $C(\text{data})$ | Zstd compression of `data` | $C : \{0,1\}^* \to \{0,1\}^*$ |
| $D(\text{data})$ | Zstd decompression of `data` | $D : \{0,1\}^* \to \{0,1\}^*$ |
| $\text{CAS}$ | Content-addressable store | Mapping $\{0,1\}^{256} \to \text{Blob}$ |
| $\text{VRef}$ | Virtual blob reference | $\text{VRef} = (\text{path}, \text{offset}, \text{length}, \text{hash})$ |
| $\text{Addr}$ | Content address (BLAKE3 digest) | 32-byte identifier |
| $n$ | Blob size in bytes | $n \in \mathbb{N}$ |

---

## 3. Theoretical Foundation

### 3.1 Axioms

**AX-001 (BLAKE3 Collision Resistance).** BLAKE3 with 256-bit output provides
$2^{128}$ second-preimage resistance and $2^{128}$ collision resistance. For any two
distinct blobs $B_1 \neq B_2$:

$$\Pr[H(B_1) = H(B_2)] \leq 2^{-128}$$

*Rationale: BLAKE3 is based on BLAKE2s and inherits its cryptographic security proofs.
The 256-bit output provides 128-bit security against collision attacks, which exceeds the
threshold for practical exploitation by a factor of $> 10^{30}$.*

### 3.2 Definitions

**DEF-001 (Content Address).** The content address of a blob $B$ is its BLAKE3-256 hash:

$$\text{addr}(B) = H(B)$$

A content address uniquely identifies blob content to within $2^{-128}$ probability of
collision. The CAS is a partial function:

$$\text{CAS} : \{0,1\}^{256} \rightharpoonup \text{Blob}$$

**DEF-002 (Virtual Blob).** A virtual blob is a lazy reference to an external file,
stored as a 4-tuple:

$$\text{VRef} = (\text{path} \in \text{String},\ \text{offset} \in \mathbb{N},\ \text{length} \in \mathbb{N},\ \text{hash} \in \{0,1\}^{256})$$

The virtual blob does not contain the referenced data. Instead, the CAS materializes the
blob on demand by reading `length` bytes from `path` at `offset` and verifying that
$H(\text{read}) = \text{hash}$. If verification fails, the CAS returns an integrity error.

*Use case: Multi-terabyte video files are never copied into the CAS. The VRef allows
Suture to track and verify them without consuming storage proportional to file size.*

### 3.3 Theorems

**THM-001 (CAS Integrity).**

> *If $H(B_{\text{stored}}) = H(B_{\text{original}})$, then $B_{\text{stored}} = B_{\text{original}}$
>   with probability $\geq 1 - 2^{-128}$.*

*Proof.* Directly from AX-001 (BLAKE3 collision resistance). If two distinct blobs produced
the same hash, this would constitute a collision. The probability of such a collision is
at most $2^{-128}$, which is negligible for all practical purposes. Therefore, hash equality
implies content equality with overwhelming probability. ∎

**THM-002 (Compression Round-Trip).**

> *For all byte sequences $\text{data} \in \{0,1\}^*$: $D(C(\text{data})) = \text{data}$.*

*Proof.* This is a design guarantee of the Zstd compression format (RFC 8878). The Zstd
decompressor is specified to exactly recover the original input for any valid compressed
stream. The Zstd library's `decompress` function will return an error rather than produce
incorrect output, providing fail-safe behavior. ∎

*Corollary (C-001):* Compressed storage introduces no information loss. A blob retrieved
from the CAS is bit-for-bit identical to the blob that was stored.

**THM-003 (Deduplication Correctness).**

> *If two blobs $B_1, B_2$ are stored via `put_blob`, and $H(B_1) = H(B_2)$, then the CAS
>   stores exactly one copy of the data.*

*Proof.* The `put_blob` algorithm (Section 4) computes $\text{addr} = H(B)$ before writing.
If a blob with address `addr` already exists in the CAS, the write is skipped (no-op).
By THM-001, $H(B_1) = H(B_2)$ implies $B_1 = B_2$ with overwhelming probability, so
skipping the write preserves correctness. ∎

---

## 4. Algorithm Specification

### 4.1 ALG-CAS-001: put_blob

Stores a blob in the CAS, with optional compression.

```
ALG-CAS-001: put_blob
=======================

Input:
  blob        : Blob        — Byte sequence to store
  compress    : Boolean     — Whether to apply Zstd compression

Output:
  addr        : Addr        — BLAKE3-256 content address

1:  function PUT_BLOB(blob, compress = true)
2:    addr ← H(blob)
3:
4:    if CAS.contains(addr) then
5:      return addr                    // Dedup: already stored
6:    end if
7:
8:    if compress then
9:      stored_data ← C(blob)
10:     meta ← { hash: addr, uncompressed_len: |blob|, compressed: true }
11:   else
12:     stored_data ← blob
13:     meta ← { hash: addr, compressed: false }
14:   end if
15:
16:   CAS.write(addr, stored_data, meta)
17:   return addr
18: end function
```

**Complexity:** $O(n)$ for hashing, $O(n)$ for compression, $O(n)$ for write. BLAKE3
achieves >1 GB/s throughput on modern hardware with SIMD (AVX-512 / NEON).

### 4.2 ALG-CAS-002: get_blob

Retrieves a blob from the CAS, decompressing if necessary.

```
ALG-CAS-002: get_blob
=======================

Input:
  addr        : Addr        — BLAKE3-256 content address

Output:
  blob        : Blob        — Original byte sequence, or Error

1:  function GET_BLOB(addr)
2:    entry ← CAS.read(addr)
3:    if entry = ∅ then
4:      return Error("Blob not found: " || hex(addr))
5:    end if
6:
7:    if entry.meta.compressed then
8:      blob ← D(entry.data)
9:      assert |blob| = entry.meta.uncompressed_len
10:   else
11:     blob ← entry.data
12:   end if
13:
14:   assert H(blob) = addr         // Integrity verification
15:   return blob
16: end function
```

**Integrity Guarantee:** Line 14 verifies that the retrieved (and possibly decompressed)
blob hashes to the requested address. If verification fails, an integrity error is raised
rather than returning corrupt data.

### 4.3 ALG-CAS-003: Dedup Check

Determines whether a blob is already stored without reading its contents.

```
ALG-CAS-003: Dedup Check
==========================

Input:
  blob        : Blob

Output:
  addr        : Addr        — BLAKE3-256 content address
  exists      : Boolean     — Whether the blob is already in the CAS

1:  function DEDUP_CHECK(blob)
2:    addr ← H(blob)
3:    exists ← CAS.contains(addr)
4:    return (addr, exists)
5: end function
```

**Complexity:** $O(n)$ — single pass for hashing. No I/O if the blob is not stored.

---

## 5. Performance Constraints

The following performance constraints derive from the BLAKE3 specification and Zstd
benchmarks, assuming modern server hardware (x86_64 with AVX-2 or ARM64 with NEON).

| Operation | Throughput | Latency (typical) | Notes |
|-----------|-----------|-------------------|-------|
| BLAKE3 hash | >1 GB/s (SIMD) | $O(n / \text{bandwidth})$ | Parallelizable across cores |
| Zstd compress (level 3) | ~500 MB/s | $O(n / \text{bandwidth})$ | Default level for Suture |
| Zstd decompress | >2 GB/s | $O(n / \text{bandwidth})$ | Memory-mapped I/O compatible |
| CAS write (SSD) | ~3 GB/s (NVMe) | $O(n / \text{device\_bw})$ | Sequential write pattern |
| Dedup check | >1 GB/s | Hash only, no I/O on miss | Single-pass BLAKE3 |

**Memory Budget:** BLAKE3 requires 1 KiB of state regardless of input size. Zstd
decompression requires a window buffer sized to the dictionary (typically 8 MiB max).

---

## 6. Virtual Blob Lifecycle

### 6.1 ALG-VBLOB-001: Register Virtual Blob

```
ALG-VBLOB-001: Register Virtual Blob
======================================

Input:
  path        : String      — Filesystem path to external file
  offset      : usize       — Byte offset within the file
  length      : usize       — Number of bytes to reference
  hash        : Addr        — Expected BLAKE3-256 of the referenced region

Output:
  addr        : Addr        — Content address (equal to hash)
  vref        : VRef        — Stored virtual reference

1:  function REGISTER_VBLOB(path, offset, length, hash)
2:    addr ← hash
3:    vref ← (path, offset, length, hash)
4:
5:    if CAS.contains(addr) then
6:      return (addr, CAS.get_vref(addr))
7:    end if
8:
9:    CAS.write_vref(addr, vref)
10:   return (addr, vref)
11: end function
```

### 6.2 ALG-VBLOB-002: Materialize Virtual Blob

```
ALG-VBLOB-002: Materialize Virtual Blob
=========================================

Input:
  addr        : Addr        — Content address of the virtual blob

Output:
  blob        : Blob        — The referenced byte range, or Error

1:  function MATERIALIZe_VBLOB(addr)
2:    vref ← CAS.get_vref(addr)
3:    if vref = ∅ then
4:      return Error("Not a virtual blob: " || hex(addr))
5:    end if
6:
7:    region ← fs.read(vref.path, vref.offset, vref.length)
8:
9:    if H(region) ≠ vref.hash then
10:     return Error("Integrity check failed: external file modified")
11:   end if
12:
13:   return region
14: end function
```

**Safety:** Line 9 ensures that if the external file has been modified or corrupted since
the VRef was created, the CAS detects the tampering rather than returning corrupt data.

---

## 7. Relationship to Requirements

| Requirement | Satisfied By |
|-------------|-------------|
| REQ-CAS-001 (BLAKE3 content addressing) | AX-001, DEF-001 |
| REQ-CAS-006 (>1 GB/s throughput) | Section 5 performance table |
| REQ-CORE-002 (determinism) | THM-001, THM-002 |
| Zero-copy branching (Reflinks) | DEF-002 (Virtual Blobs) |
| Deduplication | THM-003, ALG-CAS-001 |

---

## 8. Bibliography

1. **BLAKE3 Specification.** Jack O'Connor, Samuel Neves, et al.
   *https://github.com/BLAKE3-team/BLAKE3/specs/*.

2. **Zstandard Compression and the 'application/zstd' Media Type.** IETF RFC 8878.
   S. Collet, Y. Frauchiger. 2021.

3. **Git Internals: Pack Files and Content-Addressable Storage.**
   Provides the foundational CAS pattern that Suture extends with compression and virtual blobs.

---

## 9. Revision History

| Version | Date | Author | Description |
|---------|------|--------|-------------|
| 1.0.0 | 2026-03-27 | DeepThought | Initial draft. Defines CAS operations, virtual blobs, and performance constraints. |

---

*End of YP-ALGEBRA-PATCH-002*
