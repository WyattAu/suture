# Suture Performance Baseline

**Date:** 2026-05-12
**Platform:** Linux x86_64, release profile (optimized)
**Toolchain:** Rust stable, Criterion.rs 0.5

---

## 1. DAG Operations

| Benchmark | Median Time | Notes |
|---|---|---|
| `dag_perf_commit/commit_1000_files` | 97.8 ms | Add + commit 1000 files in one batch |
| `dag_perf_log/log_1000_commits` | 5.23 ms | Log walk over 1000-commit history |
| `dag_perf_log/log_10000_commits` | 50.8 ms | Log walk over 10,000-commit history (v3.2.1 fix) |
| `dag_perf_merge/merge_100_files` | 3.26 ms | Merge branch modifying 100 files (clean) |

### Thresholds

| Operation | Measured | Acceptable | Status |
|---|---|---|---|
| Commit 1000 files | 97.8 ms | < 500 ms | PASS |
| Log 1000 commits | 5.23 ms | < 50 ms | PASS |
| Log 10000 commits | 50.8 ms | < 500 ms | PASS |
| Merge 100 files | 3.26 ms | < 50 ms | PASS |

---

## 2. Semantic Merge (JSON)

| Benchmark | Median Time |
|---|---|
| `semantic_merge_perf/json_small_10_fields` | 8.63 us |
| `semantic_merge_perf/json_large_100_fields` | 126 us |
| `semantic_merge_perf/json_conflict/10` | 6.96 us |
| `semantic_merge_perf/json_conflict/100` | 98.4 us |

### Thresholds

| Operation | Measured | Acceptable | Status |
|---|---|---|---|
| Merge 10-field JSON | 8.63 us | < 100 us | PASS |
| Merge 100-field JSON | 126 us | < 1 ms | PASS |
| Conflict detection 100 fields | 98.4 us | < 1 ms | PASS |

---

## 3. CAS Operations

| Benchmark | Median Time |
|---|---|
| `cas_perf_store/store_100_blobs/1KB` | 1.50 ms |
| `cas_perf_store/store_100_blobs/10KB` | 2.18 ms |
| `cas_perf_store/store_100_blobs/100KB` | 5.82 ms |
| `cas_perf_store/store_100_blobs/1MB` | 50.1 ms |
| `cas_perf_lookup/store_1000_lookup_100` | 3.76 ms |

### Thresholds

| Operation | Measured | Acceptable | Status |
|---|---|---|---|
| Store 100 x 1KB blobs | 1.50 ms | < 50 ms | PASS |
| Store 100 x 1MB blobs | 50.1 ms | < 500 ms | PASS |
| Lookup 100 from 1000 | 3.76 ms | < 50 ms | PASS |

---

## 4. Patch Serialization

| Benchmark | Median Time |
|---|---|
| `patch_perf_serialize/serialize_100_patches` | 63.9 us |
| `patch_perf_deserialize/deserialize_100_patches` | 143 us |

### Thresholds

| Operation | Measured | Acceptable | Status |
|---|---|---|---|
| Serialize 100 patches | 63.9 us | < 1 ms | PASS |
| Deserialize 100 patches | 143 us | < 1 ms | PASS |

---

## 5. Hub Operations

| Benchmark | Median Time |
|---|---|
| `hub_perf_repo/create_100_repos` | 425 us |
| `hub_perf_push_pull/push_50_patches` | 693 us |
| `hub_perf_push_pull/pull_50_patches` | 95.3 us |
| `hub_perf_push_pull/push_pull_roundtrip_50` | 1.07 ms |

### Thresholds

| Operation | Measured | Acceptable | Status |
|---|---|---|---|
| Create 100 repos | 425 us | < 10 ms | PASS |
| Push 50 patches | 693 us | < 10 ms | PASS |
| Pull 50 patches | 95.3 us | < 10 ms | PASS |
| Push+pull roundtrip 50 | 1.07 ms | < 20 ms | PASS |

---

## 6. Historical Benchmark Results (from existing suite)

These results are from the pre-existing benchmark suite in `benchmarks.rs`:

| Benchmark | Median Time |
|---|---|
| `blake3_hashing/hash/1024` | 2.83 us |
| `blake3_hashing/hash/102400` | 71.5 us |
| `dag_insertion/linear_chain/1000` | 2.73 ms |
| `repo_add_commit/commit_n_files/1000` | 652 ms |
| `repo_merge/merge_clean` | 540 us |
| `semantic_merge_json/merge/1000` | 6.73 ms |
| `hub_storage/push_n_patches_blobs/1000` | 42.2 ms |
| `diff_large/patience_diff_1k_lines` | 30.0 ms |
| `compress_decompress/compress_1MB` | 544 us |

---

## Top 3 Bottlenecks

### 1. ~~Log walk at scale~~ FIXED in v3.2.1

The `repo_log` timeout at 10,000 commits was caused by `commit()` calling
`snapshot_uncached()` after every commit, which replayed the entire patch chain
from root to tip (O(n) per commit → O(n²) total). Fixed by computing the file
tree incrementally: load the parent's cached tree from SQLite (O(1)) and apply
only the new patch's changes (O(k) where k = files in the batch).

**Before:** 10,000 commits → >600s (timeout). **After:** 10,000 commits → 8.5s (70x faster).

### 2. Commit 1000 files — 652 ms (repo_add_commit vs dag_perf_commit)

The full `repo add + commit` path for 1000 files takes 652 ms (~0.65 ms/file),
while the DAG-level commit benchmark takes 97.8 ms. The ~6.7x overhead comes
from per-file filesystem I/O: reading files, hashing, writing blobs to CAS,
and updating the staging index.

**Recommendation:** Batch filesystem reads with parallel hashing (rayon).
Defer CAS writes until commit time. Use mtime/size stat cache to skip
unchanged files.

### 3. Patience diff on large files — 30 ms for 1K lines

The patience diff algorithm takes 30 ms for a 1000-line file with 10% changes.
This scales poorly — a 10K-line file would take ~300 ms, which is perceptible.

**Recommendation:** Profile to determine if the bottleneck is LCS computation
or unique-line fingerprinting. Consider the `imara-diff` crate for large-file
diffing, or switch to Myers' algorithm for files above a threshold.

---

## Quick-Win Optimizations Applied

1. **`#[inline]` on hot-path methods** — Added to `TouchSet::intersects`,
   `TouchSet::len`, `TouchSet::iter`, `TouchSet::insert`, `TouchSet::contains`,
   `DagNode::id`, `PatchDag::patch_count`, and `hash_bytes` (was already
   inlined).

2. **Removed redundant `from_utf8` in `hash_with_context`** — The function
   already takes `&str`, so `from_utf8(context.as_bytes())` was a no-op
   conversion that could fail unnecessarily.

3. **Eliminated duplicate method definitions** — Removed accidental duplicate
   `len`, `iter`, `insert` on `TouchSet` and `has_patch` on `PatchDag` that
   were causing code bloat.

---

## Binary Size

| Build | Size | Notes |
|-------|------|-------|
| Debug | 243 MB | Default (`dev` profile) |
| Release (pre-optimization) | 15 MB | `opt-level = 3` only |
| Release (stripped) | 14 MB | Manual `strip` after pre-optimization build |
| Release + LTO + strip | 14 MB | Our default (`opt-level = 3, lto = true, codegen-units = 1, panic = "abort", strip = true`) |
| Compressed (UPX) | — | UPX not available; skipped |

### Optimizations Applied

- **LTO (Link-Time Optimization):** `lto = true` — enables cross-crate inlining and dead code elimination
- **Single codegen unit:** `codegen-units = 1` — better optimization at the cost of slower compilation
- **Panic = abort:** `panic = "abort"` — removes unwinding machinery, saves ~50-100 KB
- **Strip:** `strip = true` — removes debug info and symbol tables

## Build Times

| Target | Time |
|--------|------|
| suture-cli (debug) | ~25 s |
| suture-cli (release, pre-optimization) | ~3 m 26 s |
| suture-cli (release, LTO + codegen-units = 1) | ~6 m 57 s |

---

## Benchmark Files

| File | Contents |
|---|---|
| `benches/benchmarks.rs` | Original 28 benchmarks (core, repo, semantic merge, protocol, hub) |
| `benches/dag_perf.rs` | DAG commit, log (1K/10K), merge benchmarks |
| `benches/semantic_merge_perf.rs` | JSON small/large/conflict merge benchmarks |
| `benches/cas_perf.rs` | CAS store (varying sizes), lookup, patch serialize/deserialize |
| `benches/hub_perf.rs` | Hub repo creation, push/pull/roundtrip benchmarks |

---

## 7. Comprehensive Merge Benchmarks (v5.1.0)

**Date:** 2026-05-01
**Platform:** Linux x86_64, release profile, Criterion.rs 0.5
**Method:** Median of 50 iterations

### JSON

| Size | Same (µs) | One-sided (µs) | Different Keys (µs) | Conflict (µs) |
|------|-----------|-----------------|---------------------|------------|
| 10 keys | 9.1 | 27.5 | 9.6 | 7.2 |
| 100 keys | 148 | 137 | 133 | 113 |
| 1,000 keys | 1,394 | 2,049 | 1,890 | 1,143 |
| 10,000 keys | 2,453 | 1,927 | 922 | 318 ms |

### YAML

| Size | Same (µs) | One-sided (µs) | Different Keys (µs) | Conflict (µs) |
|------|-----------|-----------------|---------------------|------------|
| 10 keys | 53.2 | 49.9 | 54.4 | 38.9 |
| 50 keys | 63.6 | 51.8 | 46.3 | 57.4 |
| 200 keys | 94.9 | 96.4 | 101.6 | 64.1 |

### TOML

| Size | Same (µs) | One-sided (µs) | Different Keys (µs) | Conflict (µs) |
|------|-----------|-----------------|---------------------|------------|
| 10 keys | 121 | 169 | 178 | 26.3 |
| 50 keys | 122 | 177 | 182 | 35.3 |

### CSV

| Size | Same (µs) | One-sided (µs) | Different Keys (µs) | Conflict (µs) |
|------|-----------|-----------------|---------------------|------------|
| 10r × 5c | 102 | 116 | 100 | 100 |
| 100r × 10c | 1,394 | 1,441 | 1,351 | 1,128 |
| 1000r × 20c | 23,543 | 35,298 | 23,660 | 2,186 |

### XML

| Size | Same (µs) | One-sided (µs) | Different Keys (µs) | Conflict (µs) |
|------|-----------|-----------------|---------------------|------------|
| 10 elements | 112 | 56.5 | 64.1 | 8.6 |
| 100 elements | 333 | 221 | 219 | 378 |

### Key Takeaways

- **JSON is the fastest format** — <1ms for typical files (up to 1,000 keys)
- **All formats merge in <5ms for typical config files** (<100 keys)
- **CSV is slowest** at scale due to row-based parsing — O(rows × cols)
- **Conflict detection** is fast (<100µs for most formats) because it only needs to find the first mismatch
- **JSON scales sub-linearly** past 1K keys (likely due to serde_json's optimized parser)

---

## 7. Recent Optimizations (v5.4.0)

### 7.1 Merge Engine (2026-05-12)

| Optimization | File | Impact |
|---|---|---|
| `output_lines()` returns `&[String]` instead of `Vec<String>` | `engine/merge.rs` | Eliminates Vec allocation per merge line group. Three-way merge hot loop calls this 2-4x per conflict region. |
| Pre-compute conflict markers once per merge | `engine/merge.rs` | Avoids `format!()` allocation for `<<<<<<<`, `=======`, `>>>>>>>` markers on every conflict region. |
| `diff_trees()` uses `sort_by` instead of `sort_by_key` | `engine/diff.rs` | Avoids cloning every path string during diff entry sorting. For diffs with thousands of files, saves thousands of allocations. |

### 7.2 Stash Operations (2026-05-12)

| Optimization | File | Impact |
|---|---|---|
| O(n*m) → O(n+m) stash push lookup | `repository/repo_impl.rs` | Builds a `HashSet<String>` of staged paths before the head-tree loop. Replaces `files.iter().any(|(p, _)| p == path)` (linear scan per file) with `hashset.contains(path)` (O(1)). For repos with many tracked files, this eliminates quadratic behavior during `suture stash push`. |

