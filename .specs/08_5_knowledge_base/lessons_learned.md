# Lessons Learned

## BLAKE3 `derive_key` API

The BLAKE3 `derive_key` function signature differs across crate versions. Early
versions used `derive_key(context, data)` while later versions changed to
`derive_key(context)` with a builder pattern or different argument ordering.
Pin the `blake3` crate version in `Cargo.lock` and test the hash output against
known test vectors after any upgrade.

## HashSet on Custom Hash Types

`HashSet` and `HashMap` work correctly with custom types that derive `Eq` and
`Hash`, provided the `Hash` implementation is stable and consistent with `Eq`.
Our `suture_common::Hash` type wraps `[u8; 32]` and delegates to the standard
library's `Hash` impl for byte arrays, which is deterministic. No issues
observed, but always verify with property tests after changing the `Hash` derive.

## SQLite WAL Mode

Write-Ahead Logging (WAL) mode is essential for concurrent read access. Without
it, readers block on writers and vice versa. Enable with `PRAGMA journal_mode=WAL`
immediately after opening the database. WAL also improves performance for
read-heavy workloads (metadata lookups) by avoiding shared lock contention.

## Zstd Compression for Small Blobs

Zstd adds overhead for blobs smaller than ~100 bytes. Consider a threshold below
which blobs are stored uncompressed. The CAS currently compresses all blobs;
adding a size check would reduce CPU time for small metadata payloads.

## Touch Set Granularity

Coarse-grained touch sets (e.g., file-level) produce false positives in conflict
detection. Fine-grained touch sets (e.g., field-level within a clip) reduce
false positives but require more driver logic. The OTIO driver uses element-level
granularity as a practical middle ground.
