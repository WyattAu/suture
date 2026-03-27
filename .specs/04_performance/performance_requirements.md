# Suture Performance Requirements v1.0

## 1. Performance Budget

| Operation | Latency Target | Throughput Target | Measurement Method |
|-----------|---------------|-------------------|-------------------|
| BLAKE3 Hash (single thread) | < 1s per GB | > 1 GB/s | criterion.rs microbenchmark |
| BLAKE3 Hash (multi-thread) | < 0.3s per GB | > 3 GB/s | criterion.rs microbenchmark |
| Zstd Compress (level 3) | < 2s per GB | > 500 MB/s | criterion.rs microbenchmark |
| Zstd Decompress | < 0.5s per GB | > 2 GB/s | criterion.rs microbenchmark |
| CAS put_blob (1MB file) | < 10ms | N/A | integration test |
| CAS get_blob (1MB file) | < 5ms | N/A | integration test |
| CAS has_blob | < 0.1ms | N/A | microbenchmark |
| Patch commutativity check | < 0.01ms | N/A | microbenchmark (1000 addresses) |
| Patch merge (10,000 patches) | < 10ms | N/A | integration test |
| Patch merge (100,000 patches) | < 100ms | N/A | integration test |
| DAG add_patch | < 0.1ms | N/A | microbenchmark |
| DAG ancestors query | < 1ms | N/A | microbenchmark (1000-node DAG) |
| DAG LCA query | < 1ms | N/A | microbenchmark (1000-node DAG) |
| Metadata store_patch | < 1ms | N/A | integration test |
| Metadata get_patch | < 0.5ms | N/A | integration test |
| CLI init | < 100ms | N/A | end-to-end test |
| CLI status (small repo) | < 50ms | N/A | end-to-end test |
| CLI commit (100 files) | < 2s | N/A | end-to-end test |

## 2. Resource Budgets

| Resource | Limit | Rationale |
|----------|-------|-----------|
| Peak RAM (CLI) | < 100 MB | Run on developer workstations |
| Peak RAM (daemon, future) | < 512 MB | Background service |
| Disk overhead per repo | < 10% of working data | CAS deduplication + compression |
| SQLite DB size (100K patches) | < 500 MB | Indexed patch metadata |
| Temp disk during operations | < 2x working data | Compression buffers |

## 3. Concurrency Requirements

| Metric | Target |
|--------|--------|
| Max concurrent readers (SQLite) | Unlimited (WAL mode) |
| Writer contention | < 5ms wait time for lock |
| Lock-free operations | CAS reads, hash computation |
| RwLock-protected | DAG writes, metadata writes |

## 4. Scalability Targets

| Dimension | v0.1 Target | v0.2 Target |
|-----------|-------------|-------------|
| Repository size | < 10 GB | < 100 GB |
| Patch count | < 100,000 | < 10,000,000 |
| Branch count | < 1,000 | < 100,000 |
| File count per commit | < 10,000 | < 1,000,000 |
| Concurrent users | 1 (local only) | < 100 (via Hub) |
