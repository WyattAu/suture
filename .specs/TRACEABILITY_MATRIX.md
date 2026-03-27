# Traceability Matrix

Maps requirements to implementation components and tests.

## Requirement → Component → Test Mapping

| # | Requirement | Component | Source | Tests |
|---|------------|-----------|--------|-------|
| R1 | Content-addressed blob storage | `suture-core::cas` (BlobStore) | REQ-001 | `cas/store.rs::tests` |
| R2 | BLAKE3 hashing | `suture-core::cas::hasher` | REQ-001 | `cas/hasher.rs::tests`, `common::tests::test_hash_*` |
| R3 | Zstd compression | `suture-core::cas::compressor` | REQ-002 | `cas/store.rs::test_compressed_store` |
| R4 | Deduplication | `suture-core::cas::BlobStore::put_blob` | REQ-003 | `cas/store.rs::test_deduplication` |
| R5 | Hash integrity verification | `suture-core::cas::BlobStore::get_blob` | REQ-004 | `cas/store.rs::test_hash_integrity` |
| R6 | Patch data structure | `suture-core::patch::types` | REQ-005 | `patch/types.rs::tests` |
| R7 | Touch set commutativity | `suture-core::patch::commute` | REQ-006, THM-COMM-001 | `patch/commute.rs::tests` |
| R8 | Patch merge | `suture-core::patch::merge` | REQ-007, THM-MERGE-001 | `patch/merge.rs::tests` |
| R9 | Conflict detection | `suture-core::patch::conflict` | REQ-008, THM-CONF-001 | `patch/conflict.rs::tests` |
| R10 | Patch DAG (acyclic) | `suture-core::dag::graph` | REQ-009, THM-DAG-001 | `dag/graph.rs::tests` |
| R11 | Branch management | `suture-core::dag::branch` | REQ-010 | `dag/branch.rs::tests` |
| R12 | LCA computation | `suture-core::dag::merge` | REQ-011 | `dag/merge.rs::tests` |
| R13 | SQLite metadata store | `suture-core::metadata` | REQ-012 | `metadata/store.rs::tests` |
| R14 | WAL mode for concurrency | `suture-core::metadata::store` | REQ-012 | `metadata/store.rs::tests` |
| R15 | Repository init/open | `suture-core::repository` | REQ-013 | `repository/repository.rs::tests` |
| R16 | Stage + commit workflow | `suture-core::repository` | REQ-014 | `repository/repository.rs::tests` |
| R17 | Status command | `suture-cli::cmd_status` | REQ-015 | Manual + integration |
| R18 | Log command | `suture-cli::cmd_log` | REQ-016 | Manual + integration |
| R19 | Branch command | `suture-cli::cmd_branch` | REQ-017 | Manual + integration |
| R20 | Merge plan command | `suture-cli::cmd_merge` | REQ-018 | Manual + integration |
| R21 | OTIO driver (parse) | `suture-driver-otio::OtioDriver` | REQ-019 | `otio/lib.rs::test_parse_*` |
| R22 | OTIO touch sets | `suture-driver-otio::compute_touch_set` | REQ-020 | `otio/lib.rs::test_compute_touch_set*` |
| R23 | OTIO visual diff | `suture-driver-otio::serialize_diff` | REQ-021 | `otio/lib.rs::test_serialize_diff*` |
| R24 | Ed25519 patch signing | *Deferred to v0.2* | SEC-001 | — |
| R25 | Audit logging | *Deferred to v0.2* | SEC-002 | — |

## Specification Cross-References

| Spec | Document | Covers Requirements |
|------|----------|-------------------|
| YP-ALGEBRA-PATCH-001 | `.specs/01_research/` | R6–R9 |
| YP-ALGEBRA-PATCH-002 | `.specs/01_research/` | R1–R4 |
| YP-DIST-CONSENSUS-001 | `.specs/01_research/` | R25 (future) |
| BP-CAS-001 | `.specs/02_architecture/` | R1–R5 |
| BP-PATCH-ALGEBRA-001 | `.specs/02_architecture/` | R6–R9 |
| BP-PATCH-DAG-001 | `.specs/02_architecture/` | R10–R12 |
| BP-METADATA-001 | `.specs/02_architecture/` | R13–R14 |
| BP-DRIVER-SDK-001 | `.specs/02_architecture/` | R21–R23 |
| BP-CLI-001 | `.specs/02_architecture/` | R15–R20 |
| STRIDE Threat Model | `.specs/03_security/` | R24–R25 |
| Performance Reqs | `.specs/04_performance/` | R1 (throughput), R8 (latency) |

## Coverage Summary

- **Implemented:** R1–R23
- **Deferred to v0.2:** R24–R25
- **Total requirements tracked:** 25
