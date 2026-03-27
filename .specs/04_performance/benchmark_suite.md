# Suture Benchmark Suite v1.0

## 1. Framework
- **Tool**: criterion.rs (https://github.com/bheisler/criterion.rs)
- **Location**: crates/suture-core/benches/
- **Run**: cargo bench

## 2. Benchmark Categories

### 2.1 Hashing Benchmarks
- bench_blake3_1kb: Hash 1 KB of data
- bench_blake3_1mb: Hash 1 MB of data
- bench_blake3_100mb: Hash 100 MB of data
- bench_blake3_1gb: Hash 1 GB of data (multi-threaded)

### 2.2 Compression Benchmarks
- bench_zstd_compress_1mb: Compress 1 MB at level 3
- bench_zstd_decompress_1mb: Decompress 1 MB
- bench_zstd_roundtrip_1mb: Compress + decompress 1 MB

### 2.3 CAS Benchmarks
- bench_cas_put_1kb: Store 1 KB blob
- bench_cas_put_1mb: Store 1 MB blob
- bench_cas_get_1mb: Retrieve 1 MB blob
- bench_cas_has_blob: Check blob existence
- bench_cas_dedup: Store duplicate blob (dedup path)

### 2.4 Patch Algebra Benchmarks
- bench_commute_disjoint: Check commutativity of disjoint patches (1000 addresses)
- bench_commute_overlapping: Check commutativity of overlapping patches (1000 addresses)
- bench_merge_10_patches: Merge two branches with 10 patches each
- bench_merge_1000_patches: Merge two branches with 1000 patches each
- bench_merge_10000_patches: Merge two branches with 10000 patches each
- bench_detect_conflicts_100: Conflict detection with 100 patches per branch

### 2.5 DAG Benchmarks
- bench_dag_add_patch_linear: Add patch to linear chain
- bench_dag_add_patch_branch: Add patch creating new branch
- bench_dag_ancestors_100: Ancestor query on 100-node DAG
- bench_dag_ancestors_1000: Ancestor query on 1000-node DAG
- bench_dag_lca_1000: LCA query on 1000-node DAG

### 2.6 Metadata Benchmarks
- bench_meta_store_patch: Insert patch record
- bench_meta_get_patch: Query patch by ID
- bench_meta_set_branch: Update branch pointer
- bench_meta_list_branches: List all branches

## 3. Regression Detection
- Store baseline results in .specs/06_5_regression/baseline_metrics.toml
- CI compares against baseline; fail if > 20% regression
- Track trends over time

## 4. Profiling Strategy
- Use perf (Linux) and Instruments (macOS) for profiling
- Focus areas: hash throughput, compression ratio, DAG traversal, SQLite query plans
- Profile on representative workloads: 10K-patch OTIO timeline, 1K-file spreadsheet
