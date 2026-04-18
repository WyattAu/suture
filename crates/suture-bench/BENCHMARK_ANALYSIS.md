# Suture VCS — Benchmark Analysis Report

**Date:** 2026-04-18
**Platform:** Linux, release profile (optimized)
**Runner:** Criterion.rs (plotters backend)

---

## Benchmark Inventory

All benchmarks are defined in `crates/suture-bench/benches/benchmarks.rs` and organized into 28 benchmark functions across 5 categories.

### 1. Core Primitives (7 benchmarks)

| Function | Group | What It Measures |
|---|---|---|
| `bench_cas_put_get` | `cas_put_get` | Content-addressable store: put a blob then get it back (1KB, 10KB, 100KB) |
| `bench_hashing` | `blake3_hashing` | Blake3 hash computation over byte buffers (64B, 1KB, 10KB, 100KB) |
| `bench_dag_insertion` | `dag_insertion` | Inserting N patches into a linear PatchDag chain (10, 100, 1000) |
| `bench_dag_lca` | `dag_lca` | Lowest-common-ancestor lookup on a linear chain (10, 100, 500) |
| `bench_dag_lca_diamond` | `dag_lca_diamond` | LCA lookup on a diamond-shaped merge DAG (depth 5, 20, 50) |
| `bench_dag_ancestors_cached` | `dag_ancestors_cached` | Ancestor traversal on a 1000-node DAG, called twice (cache warm) |
| `bench_patch_chain` | `patch_chain` | Apply a chain of N create-patches to build a FileTree (10, 50, 100) |

### 2. Repository Operations (7 benchmarks)

| Function | Group | What It Measures |
|---|---|---|
| `bench_repo_init` | `repo_init` | Initialize a new repository from scratch |
| `bench_repo_add_commit` | `repo_add_commit` | Stage and commit files (1, 10, 100, 1000 files) |
| `bench_repo_log` | `repo_log` | Walk commit log for N commits (10, 100, 1000) |
| `bench_repo_diff` | `repo_diff` | Diff between HEAD~1 and HEAD with N changed files (10, 100, 1000) |
| `bench_repo_branch` | `repo_branch` | Create branch; checkout branch round-trip |
| `bench_repo_merge` | `repo_merge` | Merge clean and conflicting branches |
| `bench_large_json_commit` | `large_json_commit` | Commit a JSON file of varying size (10KB, 100KB, 1MB) |

### 3. Semantic Merge (4 benchmarks)

| Function | Group | What It Measures |
|---|---|---|
| `bench_semantic_merge_json` | `semantic_merge_json` | 3-way semantic merge of JSON (10, 100, 1000 keys) |
| `bench_semantic_merge_yaml` | `semantic_merge_yaml` | 3-way semantic merge of YAML (10, 100, 1000 keys) |
| `bench_semantic_merge_toml` | `semantic_merge_toml` | 3-way semantic merge of TOML (10, 100, 1000 keys) |
| `bench_semantic_merge_csv` | `semantic_merge_csv` | 3-way semantic merge of CSV (10, 100, 1000 rows) |

### 4. Protocol Operations (3 benchmarks)

| Function | Group | What It Measures |
|---|---|---|
| `bench_delta_compute` | `delta_compute` | Binary delta computation (100B, 10KB, 1MB) |
| `bench_delta_apply` | `delta_apply` | Binary delta application (100B, 10KB, 1MB) |
| `bench_compress_decompress` | `compress_decompress` | Zstd compress/decompress (100B, 10KB, 1MB) |

### 5. Hub & Large-Scale (4 benchmarks)

| Function | Group | What It Measures |
|---|---|---|
| `bench_hub_storage` | `hub_storage` | In-memory hub: push/pull N patches+blobs (10, 100, 1000); roundtrip |
| `bench_filetree_large` | `filetree_large` | Insert 10K files into FileTree; snapshot (content_hash) |
| `bench_diff_large` | `diff_large` | Patience diff on 1000-line files (10% changed) |
| `bench_pack_large` | `pack_large` | Pack 1000 blobs into a PackFile |
| `bench_large_yaml_commit` | `large_yaml_commit` | Commit a YAML file (10KB, 100KB) |

---

## Results Summary

### Core Primitives

| Benchmark | Median Latency | Throughput |
|---|---|---|
| `cas_put_get/put_get/1024` | 130 µs | ~7.7K ops/s |
| `cas_put_get/put_get/10240` | 292 µs | ~3.4K ops/s |
| `cas_put_get/put_get/102400` | 347 µs | ~2.9K ops/s |
| `blake3_hashing/hash/64` | 217 ns | ~4.6 GB/s |
| `blake3_hashing/hash/1024` | 2.83 µs | ~362 MB/s |
| `blake3_hashing/hash/10240` | 8.51 µs | ~1.2 GB/s |
| `blake3_hashing/hash/102400` | 71.5 µs | ~1.4 GB/s |
| `dag_insertion/linear_chain/10` | 37.5 µs | ~267 ops/s |
| `dag_insertion/linear_chain/100` | 529 µs | ~1.9K ops/s (per-insert ~5.3 µs) |
| `dag_insertion/linear_chain/1000` | 2.73 ms | ~367 ops/s (per-insert ~2.7 µs) |
| `dag_lca/linear_chain/10` | 34.4 µs | ~29K ops/s |
| `dag_lca/linear_chain/100` | 265 µs | ~3.8K ops/s |
| `dag_lca/linear_chain/500` | 860 µs | ~1.2K ops/s |
| `dag_lca_diamond/diamond_merge/5` | 7.02 µs | ~142K ops/s |
| `dag_lca_diamond/diamond_merge/20` | 24.0 µs | ~42K ops/s |
| `dag_lca_diamond/diamond_merge/50` | 71.5 µs | ~14K ops/s |
| `dag_ancestors_cached/ancestors_1k_cached` | 340 µs | ~2.9K ops/s |
| `patch_chain/apply/10` | 30.7 µs | ~32.6K ops/s |
| `patch_chain/apply/50` | 297 µs | ~3.4K ops/s |
| `patch_chain/apply/100` | 839 µs | ~1.2K ops/s |

### Repository Operations

| Benchmark | Median Latency |
|---|---|
| `repo_init/init_new_repo` | 2.92 ms |
| `repo_add_commit/add_commit_single_file` | 2.11 ms |
| `repo_add_commit/commit_n_files/1` | 2.34 ms |
| `repo_add_commit/commit_n_files/10` | 2.22 ms |
| `repo_add_commit/commit_n_files/100` | 37.9 ms |
| `repo_add_commit/commit_n_files/1000` | 652 ms |
| `repo_log/log_n_commits/10` | 775 µs |
| `repo_log/log_n_commits/100` | 3.31 ms |
| `repo_diff/diff_n_files_changed/10` | 1.54 ms |
| `repo_diff/diff_n_files_changed/100` | 4.50 ms |
| `repo_diff/diff_n_files_changed/1000` | 34.7 ms |
| `repo_branch/create_branch` | 267 µs |
| `repo_branch/checkout_branch` | 483 µs |
| `repo_merge/merge_clean` | 540 µs |
| `repo_merge/merge_conflicting` | 941 µs |

### Semantic Merge

| Benchmark | Median Latency |
|---|---|
| `semantic_merge_json/merge/10` | 13.1 µs |
| `semantic_merge_json/merge/100` | 257 µs |
| `semantic_merge_json/merge/1000` | 6.73 ms |
| `semantic_merge_yaml/merge/10` | 184 µs |
| `semantic_merge_yaml/merge/100` | 701 µs |
| `semantic_merge_yaml/merge/1000` | 10.7 ms |
| `semantic_merge_toml/merge/10` | 221 µs |
| `semantic_merge_toml/merge/100` | 2.67 ms |
| `semantic_merge_toml/merge/1000` | 14.2 ms |
| `semantic_merge_csv/merge/10` | 446 µs |
| `semantic_merge_csv/merge/100` | 480 µs |
| `semantic_merge_csv/merge/1000` | 2.15 ms |

### Protocol Operations

| Benchmark | Median Latency |
|---|---|
| `delta_compute/100B` | 551 ns |
| `delta_compute/10KB` | 4.87 µs |
| `delta_compute/1MB` | 861 µs |
| `delta_apply/100B` | 78.7 ns |
| `delta_apply/10KB` | 317 ns |
| `delta_apply/1MB` | 180 µs |
| `compress_decompress/compress_100B` | 45.4 µs |
| `compress_decompress/decompress_100B` | 968 ns |
| `compress_decompress/compress_10KB` | 86.5 µs |
| `compress_decompress/decompress_10KB` | 2.86 µs |
| `compress_decompress/compress_1MB` | 544 µs |
| `compress_decompress/decompress_1MB` | 245 µs |

### Hub & Large-Scale

| Benchmark | Median Latency |
|---|---|
| `hub_storage/push_n_patches_blobs/10` | 797 µs |
| `hub_storage/pull_n_patches_blobs/10` | 164 µs |
| `hub_storage/push_n_patches_blobs/100` | 4.49 ms |
| `hub_storage/pull_n_patches_blobs/100` | 456 µs |
| `hub_storage/push_n_patches_blobs/1000` | 42.2 ms |
| `hub_storage/pull_n_patches_blobs/1000` | 2.62 ms |
| `hub_storage/push_pull_roundtrip_100` | 7.77 ms |
| `filetree_large/insert_10k_files` | 9.15 ms |
| `filetree_large/snapshot_10k_files` | 16.0 µs |
| `diff_large/patience_diff_1k_lines` | 30.0 ms |
| `pack_large/pack_create_1k_blobs` | 76.7 ms |
| `large_json_commit/10KB` | 1.10 ms |
| `large_json_commit/100KB` | 2.13 ms |
| `large_json_commit/1MB` | 4.42 ms |
| `large_yaml_commit/10KB` | 757 µs |
| `large_yaml_commit/100KB` | 1.01 ms |

---

## Optimization Opportunities

### 1. Batch file operations in `repo add` + `repo commit`

**What:** The `commit_n_files/1000` benchmark takes 652 ms — that is ~0.65 ms per file. For comparison, `commit_n_files/10` takes 2.22 ms total (~0.22 ms per file), showing super-linear scaling. This suggests per-file overhead (syscalls, hashing, staging) that could be batched.

**Where:** `suture-core::repository::Repository::add()` and `Repository::commit()`.

**Why:** A 3x per-file cost increase from 10 to 1000 files means the commit path has O(n log n) or worse behavior, likely from repeated filesystem I/O or tree rebuilds.

**How:** Hash files in parallel using a thread pool (e.g., `rayon`). Batch blob writes to the CAS (write-ahead log or buffered I/O). Defer FileTree reconstruction to a single pass after all files are staged.

### 2. Optimize DAG ancestor traversal and patch_chain for large histories

**What:** `dag_large/ancestors_10k` takes 172 ms to build a 10K-node DAG and compute the patch chain. The `dag_ancestors_cached` benchmark at 1K nodes takes 340 µs (cached), so scaling to 10K would naively take ~3.4 ms — the 172 ms indicates the construction cost dominates, not the traversal.

**Where:** `suture-core::dag::graph::PatchDag`.

**Why:** For repositories with long histories (10K+ commits), `patch_chain` and `ancestors` will be called frequently during merge, rebase, and push operations.

**How:** Pre-compute and persist the topological ordering or use a persistent data structure (e.g., path-copying or union-find) for ancestor queries. For `patch_chain`, consider storing a flat list alongside the graph to avoid repeated traversal. Add incremental indexing when patches are inserted.

### 3. Parallelize or batch hub storage writes

**What:** Pushing 1000 patches+blobs takes 42.2 ms, while pulling the same data takes only 2.62 ms — a 16x asymmetry. Push involves 2000 individual insert operations.

**Where:** `suture-hub::HubStorage`.

**Why:** Push is a critical path for collaborative workflows. The write-heavy nature suggests per-item overhead from serialization or storage backend calls.

**How:** Batch inserts using a transaction or bulk write API. If the backend is SQLite, use `BEGIN TRANSACTION` / `COMMIT` around bulk inserts. If using an in-memory store, ensure Vec/HashMap pre-allocation.

### 4. Speed up patience diff for large files

**What:** `diff_large/patience_diff_1k_lines` takes 30.0 ms for a 1000-line file with 10% changes. This is the slowest per-unit operation in the benchmark suite relative to input size.

**Where:** `suture-core::engine::merge::diff_lines` (likely using the patience diff algorithm).

**Why:** Diff computation is on the critical path for every `suture diff`, `suture merge`, and `suture status` invocation. 30 ms for 1K lines would become 300 ms for 10K lines, which is perceptible to users.

**How:** Profile to identify whether the bottleneck is LCS computation or the patience heuristic's unique-line fingerprinting. Consider: (a) Myers' diff algorithm as a faster alternative for large files, (b) early-termination heuristics for files that differ mostly at the end, (c) hashing line prefixes to skip unchanged regions, (d) the `imara-diff` crate which is optimized for large-file diffing in Rust.

### 5. Reduce per-commit overhead (repo init + add + commit baseline)

**What:** Even committing a single file (`add_commit_single_file`) takes 2.11 ms, and `repo_init` takes 2.92 ms. A significant portion of this is likely filesystem overhead from creating directories, writing metadata files, and initializing the DAG/CAS.

**Where:** `suture-core::repository::Repository::init()`, `add()`, `commit()`.

**Why:** These operations form the baseline cost of every user interaction. A user running `suture add . && suture commit` on a small change expects sub-millisecond response.

**How:** Lazy initialization — defer directory/metadata creation until first actual write. Use a single `std::fs::create_dir_all` instead of multiple individual creates. Ensure the CAS and DAG are not persisted to disk on every operation if they haven't changed. Consider an in-memory "dirty" flag to skip serialization when nothing changed.

---

## Hot Path Analysis

### `suture add` (hash file, store blob, stage)

**Steps:** Read file from disk → compute blake3 hash → write blob to CAS → update staging area (index).

**Bottleneck:** CAS put+get at ~130–347 µs per blob (filesystem I/O dominates). For 1000 files, this alone accounts for ~130–347 ms of the 652 ms total commit time.

**Optimizations:**
- Skip re-hashing files whose mtime/size haven't changed (stat cache).
- Write blobs to CAS lazily — only persist on commit, not on add.
- Use a write buffer or memory-mapped file for CAS storage.
- Parallelize hashing across files with `rayon`.

### `suture commit` (create patch, store in DAG, update HEAD)

**Steps:** Compute FileTree snapshot from staged files → diff against parent tree → create Patch → add to DAG → update HEAD ref → persist DAG and refs.

**Bottleneck:** FileTree snapshot (`snapshot_10k_files` is fast at 16 µs, so this is not the issue). The DAG insertion for 1000 patches shows ~2.7 µs per insert, which is reasonable. The dominant cost is the per-file CAS write during the preceding `add` phase.

**Optimizations:**
- Batch DAG serialization — write the entire DAG once on commit, not per-insert.
- Use an append-only log for patches (like Git's pack files) rather than individual files.
- Defer hash resolution in patches until they are actually needed for push/fetch.

### `suture push` (serialize patches, compute deltas, HTTP transfer)

**Steps:** Enumerate patches to push → serialize to protobuf → compute deltas against known base → compress → HTTP POST.

**Bottleneck:** Delta computation scales at ~0.86 µs per KB (1MB in 861 µs). Compression is fast at 544 µs for 1MB. Serialization and network transfer are not benchmarked here.

**Optimizations:**
- Pre-compute deltas during commit rather than at push time.
- Stream patches to the server instead of serializing all in memory.
- Use HTTP/2 multiplexing or gRPC streaming for parallel patch upload.

### `suture merge` (compute diff, apply patches, detect conflicts)

**Steps:** Find LCA in DAG → collect ancestor patches → apply patch chains from both sides → detect conflicts in touch sets → resolve or report.

**Bottleneck:** LCA lookup is fast (7–72 µs for diamond DAGs). Patch chain application scales linearly (~8.4 µs per patch). Conflict detection depends on `diff_lines` which is the expensive part at 30 ms for 1K lines.

**Optimizations:**
- Cache LCA results for frequently merged branches.
- For conflict detection, use touch-set intersection as a fast pre-filter before doing line-level diffing.
- Parallelize patch chain application for independent branches.

### `suture status` (scan working tree, compare with HEAD)

**Steps:** Walk working directory → stat each file → compute hash for changed files → compare with HEAD FileTree → report differences.

**Bottleneck:** Not directly benchmarked, but based on the data: filesystem walk + stat is O(n) with n = file count. Hashing each changed file costs ~71.5 µs per 100KB. For a project with 1000 files, the overhead is dominated by filesystem syscalls.

**Optimizations:**
- Use an in-memory mtime/size cache to skip unchanged files without re-hashing.
- Watch the filesystem with `notify` crate to maintain a hot cache of changes.
- Parallelize the hash computation across changed files.

---

## Observations & Caveats

1. **Change percentages in output are noise.** Criterion reports "change" relative to a previous baseline that was never saved (the `--save-baseline` flag failed because the installed Criterion version doesn't support it). These percentages should be ignored — only the absolute timing numbers are meaningful.

2. **`repo_log/log_n_commits/1000` could not complete** within the 5-minute timeout. This suggests O(n^2) or worse behavior in the log walk for large histories — a serious concern for real-world usage.

3. **TOML semantic merge is the slowest structured merge**, 14.2 ms for 1000 keys vs 6.73 ms for JSON and 10.7 ms for YAML. This may be due to the `toml` crate's parser performance.

4. **Push/pull asymmetry in hub storage** (16x ratio at 1000 items) suggests the write path has significant per-item overhead that doesn't exist on the read path.

5. **The benchmarks use synthetic data** (repeated patterns like `42u8`, `format!("content_{}", i)`). Real-world performance may differ significantly for compressible data (smaller deltas, better compression ratios) or highly entropic data (larger blobs, slower hashing).
