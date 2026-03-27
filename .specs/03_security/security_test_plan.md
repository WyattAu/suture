# Suture Security Test Plan v1.0

**Document ID:** STP-SEC-001  
**Version:** 1.0.0  
**Status:** APPROVED  
**Created:** 2026-03-27  
**Companion:** TM-SEC-001 (STRIDE Threat Model)

---

## 1. Scope

This plan defines security-specific tests for Suture v0.1, covering all threats identified
in TM-SEC-001. Tests are categorized by threat class and mapped to specific threat IDs.

**Schedule:** Every test in this plan runs on every CI build. Security tests are gated — a
failure blocks merging.

---

## 2. Tooling

| Tool | Purpose | Integration |
|------|---------|-------------|
| `cargo audit` | Scan `Cargo.lock` for known CVEs in dependencies | CI pipeline, fails on high/critical |
| `proptest` | Property-based fuzzing with adversarial inputs | `#[cfg(test)]` modules |
| AddressSanitizer (ASan) | Detect memory unsafety: buffer overflows, use-after-free | CI via `RUSTFLAGS="-Z sanitizer=address"` |
| `cargo clippy` | Static analysis for common vulnerability patterns | CI pipeline, `#![deny(clippy::all)]` |
| `cargo test` | Unit + integration tests | CI pipeline |
| Known-answer vectors | BLAKE3 and Zstd test vectors from upstream projects | Unit tests |

---

## 3. Test Categories

### 3.1 Input Validation (THM-E-001, THM-E-002, THM-E-004)

| Test ID | Description | Threat | Method | Expected |
|---------|-------------|--------|--------|----------|
| SEC-INP-001 | Branch name with shell metacharacters (`; rm -rf /`) | THM-E-001 | Unit: pass to branch creation | Rejected with validation error |
| SEC-INP-002 | Branch name with path traversal (`../../etc`) | THM-E-001 | Unit: pass to branch creation | Rejected with validation error |
| SEC-INP-003 | Branch name with null bytes (`branch\x00evil`) | THM-E-001 | Unit: pass to branch creation | Rejected with validation error |
| SEC-INP-004 | Branch name with Unicode homoglyphs | THM-E-001 | Unit: pass to branch creation | Rejected or normalized |
| SEC-INP-005 | File path traversal in virtual blob registration (`../../etc/passwd`) | THM-E-002 | Unit: `register_virtual_blob` with traversal path | Rejected with path validation error |
| SEC-INP-006 | Virtual blob path outside repository root | THM-E-002 | Unit: `register_virtual_blob` with `/etc/shadow` | Rejected unless explicitly allowed |
| SEC-INP-007 | SQL injection in patch metadata | THM-E-004 | Unit: patch author = `"'; DROP TABLE patches; --"` | Data stored verbatim; no SQL executed |
| SEC-INP-008 | Blob path canonicalization race (TOCTOU) | THM-E-002 | Integration: concurrent canonicalize + symlink | No race condition; operation is atomic |
| SEC-INP-009 | Oversized CLI argument (>1 MiB string) | THM-E-001 | Unit: pass to argument parser | Rejected with size limit error |

### 3.2 CAS Integrity (THM-T-001, THM-T-005, THM-T-006)

| Test ID | Description | Threat | Method | Expected |
|---------|-------------|--------|--------|----------|
| SEC-CAS-001 | Blob file replaced with different content after storage | THM-T-001 | Integration: `put_blob` → modify file on disk → `get_blob` | `CasError::IntegrityCheckFailed` |
| SEC-CAS-002 | Blob file truncated (partial write) | THM-T-001 | Integration: truncate file on disk → `get_blob` | `CasError::IntegrityCheckFailed` or `CasError::IoError` |
| SEC-CAS-003 | Blob file header corrupted (magic bytes changed) | THM-T-001 | Integration: overwrite magic bytes → `get_blob` | `CasError::IoError` (invalid format) |
| SEC-CAS-004 | Zstd decompression bomb (100 KiB → 2 GiB) | THM-T-006, THM-D-004 | Unit: crafted compressed payload exceeding limit | Rejected with decompression size error |
| SEC-CAS-005 | Malformed Zstd stream (truncated frame) | THM-T-006 | Unit: truncated Zstd data to `decompress` | `CasError::CompressionError` |
| SEC-CAS-006 | Deduplication after blob replacement | THM-T-001 | Integration: `put_blob(A)` → replace file → `put_blob(A)` | Second put detects corruption, returns integrity error |
| SEC-CAS-007 | Empty blob storage and retrieval | THM-T-001 | Unit: `put_blob(&[])` | Rejected (precondition: non-empty) or handled gracefully |
| SEC-CAS-008 | Blob exceeding configured size limit (e.g., 2 GiB) | THM-D-001 | Unit: `put_blob` with oversized data | Rejected with size limit error |

### 3.3 Patch Algebra & DAG Integrity (THM-T-002, THM-T-004, THM-T-005)

| Test ID | Description | Threat | Method | Expected |
|---------|-------------|--------|--------|----------|
| SEC-DAG-001 | Patch with forged ID (SHA-256 mismatch) | THM-T-002 | Unit: create patch, modify content, keep old ID | Detected: recomputed ID differs |
| SEC-DAG-002 | Patch with externally-supplied touch set (empty) to bypass conflict detection | THM-T-004 | Unit: manually construct patch with empty touch set but conflicting payload | Touch set is ignored if not from driver; algebra engine uses driver-computed touch set |
| SEC-DAG-003 | Conflict node modification after creation | THM-T-005 | Integration: store conflict → modify serialized form → reload | Integrity check detects tampering |
| SEC-DAG-004 | Cycle injection in DAG (patch references itself) | THM-T-002 | Unit: create patch with `parent_ids` containing its own ID | Rejected: cycle detection |
| SEC-DAG-005 | Merge with tampered base branch | THM-T-002 | Integration: modify base patch content in CAS → attempt merge | Merge fails with integrity error when base patches are loaded |
| SEC-DAG-006 | Determinism under adversarial patch ordering | THM-T-002 | Proptest: `merge(base, a, b) == merge(base, b, a)` with random patches | Always equal |

### 3.4 Metadata Tampering (THM-T-003, THM-R-002)

| Test ID | Description | Threat | Method | Expected |
|---------|-------------|--------|--------|----------|
| SEC-MET-001 | Direct SQLite modification of blob size record | THM-T-003 | Integration: modify `cas_blobs.size` → `get_blob` | Integrity check catches size mismatch (actual vs recorded) |
| SEC-MET-002 | Delete a blob from SQLite metadata but leave file on disk | THM-T-003 | Integration: delete metadata row → `get_blob` | Returns error (not found in metadata) |
| SEC-MET-003 | Insert a fake metadata row pointing to non-existent file | THM-T-003 | Integration: insert row → `get_blob` | Returns `CasError::IoError` (file not found) |
| SEC-MET-004 | Merge log append-only verification | THM-R-002 | Unit: verify merge records are never updated or deleted | All merge operations produce INSERT-only queries |
| SEC-MET-005 | WAL file integrity after unclean shutdown | THM-T-003 | Integration: kill process mid-write → reopen DB | SQLite WAL recovery restores consistent state |

### 3.5 Denial of Service Resilience (THM-D-001 through THM-D-006)

| Test ID | Description | Threat | Method | Expected |
|---------|-------------|--------|--------|----------|
| SEC-DOS-001 | Store blob at exactly the configured size limit | THM-D-001 | Unit: `put_blob` with data at limit | Accepted |
| SEC-DOS-002 | Store blob exceeding configured size limit by 1 byte | THM-D-001 | Unit: `put_blob` with limit + 1 bytes | Rejected with size error |
| SEC-DOS-003 | Zstd decompression to exactly the configured decompression limit | THM-D-004 | Unit: craft payload decompressing to limit | Accepted |
| SEC-DOS-004 | Zstd decompression exceeding limit by 1 byte | THM-D-004 | Unit: craft payload decompressing to limit + 1 | Rejected with decompression error |
| SEC-DOS-005 | Create 10,000 trivial patches and measure merge time | THM-D-002 | Benchmark: `merge` with large patch sets | Completes within acceptable time threshold |
| SEC-DOS-006 | Virtual blob reference chain at maximum depth | THM-D-006 | Unit: create VRef chain of depth 16 | Accepted (at limit) |
| SEC-DOS-007 | Virtual blob reference chain exceeding maximum depth | THM-D-006 | Unit: create VRef chain of depth 17 | Rejected with depth error |
| SEC-DOS-008 | Concurrent SQLite write operations | THM-D-003 | Integration: 100 concurrent `put_blob` calls | All complete; no lock timeouts after `busy_timeout` |

### 3.6 Dependency Vulnerability Scanning

| Test ID | Description | Method | Expected |
|---------|-------------|--------|----------|
| SEC-DEP-001 | `cargo audit` on `Cargo.lock` | CI step: `cargo audit` | Zero high/critical CVEs |
| SEC-DEP-002 | License compliance scan | CI step: `cargo-deny` (if adopted) | No GPL/AGPL in Apache-2.0 project |

---

## 4. Property-Based Test Invariants

These invariants are tested with `proptest` using 10,000 random iterations each:

| Invariant | Description | Threat Coverage |
|-----------|-------------|----------------|
| SEC-PROP-001 | For all byte sequences B: `get_blob(put_blob(B)) == B` | THM-T-001 |
| SEC-PROP-002 | For all byte sequences B: `H(get_blob(put_blob(B))) == H(B)` | THM-T-001 |
| SEC-PROP-003 | For all compressed C: decompression result size ≤ configured limit | THM-D-004, THM-T-006 |
| SEC-PROP-004 | For all patch pairs (P1, P2): `commute(P1, P2)` is deterministic | THM-T-002 |
| SEC-PROP-005 | For all patch triples: `merge(base, a, b) == merge(base, b, a)` | THM-T-002 |
| SEC-PROP-006 | For all paths P containing `..`: `register_virtual_blob(P, ...)` fails | THM-E-002 |
| SEC-PROP-007 | For all branch names N with non-ASCII: branch creation fails or normalizes | THM-E-001 |

---

## 5. CI Integration

### 5.1 Pipeline Steps

```yaml
security-tests:
  stage: test
  steps:
    - name: Dependency audit
      run: cargo audit --deny high --deny critical

    - name: Security unit tests
      run: cargo test --lib sec_ -- --test-threads=4

    - name: Security integration tests
      run: cargo test --test sec_integration -- --test-threads=2

    - name: ASan build and test
      run: |
        RUSTFLAGS="-Z sanitizer=address" \
        cargo +nightly test --target x86_64-unknown-linux-gnu \
        --lib sec_ --test-threads=1

    - name: Clippy security lints
      run: cargo clippy -- -D clippy::all -D clippy::security

    - name: Property-based tests
      run: cargo test --lib sec_prop_ -- --test-threads=4
```

### 5.2 Failure Policy

- **SEC-DEP-001 (cargo audit):** Hard fail. Block merge until dependency is updated or ignored with documented justification.
- **SEC-CAS-001 through SEC-DOS-008:** Hard fail. Any integrity or DoS test failure is a release blocker.
- **SEC-PROP-* (property-based tests):** Hard fail. A single counterexample invalidates the invariant.
- **Clippy security lints:** Hard fail. Zero warnings policy.

### 5.3 Test Naming Convention

All security tests use the prefix `sec_` in test function names:
- Unit tests: `fn sec_inp_001_branch_shell_metacharacters()`
- Integration tests: `fn sec_cas_001_blob_replacement_detection()`
- Property tests: `fn sec_prop_001_cas_round_trip()`

---

## 6. Test Execution Schedule

| Trigger | Scope |
|---------|-------|
| Every CI build (push + PR) | All SEC-* tests, cargo audit, clippy |
| Nightly | Full proptest suite (100,000 iterations), ASan full test suite |
| Release candidate | Full security test suite + manual penetration test of CLI |

---

## 7. Traceability to Threat Model

| Threat ID | Test IDs |
|-----------|----------|
| THM-T-001 | SEC-CAS-001, SEC-CAS-002, SEC-CAS-003, SEC-CAS-006, SEC-PROP-001, SEC-PROP-002 |
| THM-T-002 | SEC-DAG-001, SEC-DAG-004, SEC-DAG-005, SEC-DAG-006 |
| THM-T-003 | SEC-MET-001, SEC-MET-002, SEC-MET-003, SEC-MET-005 |
| THM-T-004 | SEC-DAG-002 |
| THM-T-005 | SEC-DAG-003 |
| THM-T-006 | SEC-CAS-004, SEC-CAS-005 |
| THM-D-001 | SEC-CAS-008, SEC-DOS-001, SEC-DOS-002 |
| THM-D-002 | SEC-DOS-005 |
| THM-D-003 | SEC-DOS-008 |
| THM-D-004 | SEC-CAS-004, SEC-DOS-003, SEC-DOS-004, SEC-PROP-003 |
| THM-D-006 | SEC-DOS-006, SEC-DOS-007 |
| THM-E-001 | SEC-INP-001 through SEC-INP-004, SEC-INP-009, SEC-PROP-007 |
| THM-E-002 | SEC-INP-005, SEC-INP-006, SEC-INP-008, SEC-PROP-006 |
| THM-E-004 | SEC-INP-007 |
| THM-R-002 | SEC-MET-004 |
| Dependency risks | SEC-DEP-001, SEC-DEP-002 |

---

## 8. Review History

| Date | Reviewer | Changes |
|------|----------|---------|
| 2026-03-27 | Security Engineering (Phase 3) | Initial test plan |

---

*End of STP-SEC-001*
