# Suture Optimization Roadmap v1.0

## 1. Priority 1: Must-Have for v0.1
- [ ] BLAKE3 SIMD auto-detection (runtime feature detection)
- [ ] Zstd level tuning (target: level 3 for balance of speed/ratio)
- [ ] SQLite WAL mode with PRAGMA settings optimization
- [ ] Touch-set as HashSet (O(1) intersection check for commutativity)
- [ ] DAG adjacency list with parent HashMap (O(1) parent lookup)

## 2. Priority 2: Should-Have for v0.1
- [ ] Flatbuffers zero-copy for patch serialization
- [ ] Memory-mapped I/O for large blob reads
- [ ] Parallel hash computation for multi-file commits
- [ ] DAG patch indexing by touch-set for faster merge conflict detection

## 3. Priority 3: Future Optimizations (v0.2+)
- [ ] Shared Memory (SHM) for daemon status lookups (< 500ns)
- [ ] Reflink/CoW for instantaneous branching of large files
- [ ] Incremental BLAKE3 hashing for modified files
- [ ] Patch compaction (squash old patches into snapshots)
- [ ] Lazy DAG loading (don't load entire DAG into memory)
- [ ] Read-only SQLite connections for concurrent queries

## 4. Known Performance Risks
| Risk | Impact | Mitigation |
|------|--------|------------|
| Zstd level too high | Slow commits | Default to level 3, make configurable |
| SQLite contention | Slow concurrent operations | WAL mode, short transactions |
| Large touch-set intersection | Slow merge | Index touch-sets, early termination |
| Deep DAG traversal | Slow ancestor queries | Memoization, cached transitive closure |
