# Suture as an rsync and Syncthing Replacement

## Executive Summary

Suture is a **patch-based version control system** with a synchronization layer, not a file synchronization tool. Its architecture is fundamentally different from rsync (block-level file transfer) and Syncthing (peer-to-peer continuous sync). While Suture provides file watching, auto-sync, delta encoding, and a sync daemon, it lacks the core algorithms that make rsync and Syncthing efficient for their use cases: rolling-hash block matching, peer-to-peer discovery, NAT traversal, and resume-on-failure transfer.

**Verdict: 25-30% complete for the "rsync/Syncthing replacement" use case. The version control layer adds value that rsync/Syncthing cannot provide, but the sync infrastructure needs substantial work.**

---

## 1. What rsync Does (The Benchmark)

rsync's core algorithm (Andrew Tridgell, 1996):

1. **Sender computes checksums** for fixed-size blocks of the source file:
   - Weak checksum: Adler32-based rolling hash (fast, collision-prone)
   - Strong checksum: MD4 (later MD5) for confirmation
2. **Receiver has the target file.** It finds matching blocks at **any offset** using the rolling hash.
3. **Protocol exchanges only**: block checksums (small) + non-matching data (the actual delta).
4. Result: transfers only the changed portions of a file, regardless of where those changes occur.

**Key properties:**
- Block-level deduplication (detects moved/inserted blocks)
- Works over SSH (secure transport)
- Bandwidth limiting (`--bwlimit`)
- Partial transfer resume (`--partial`, `--append-verify`)
- Hardlink/symlink preservation
- Exclude patterns
- Checksum verification (`-c`)
- Device files, permissions, ownership preservation
- Delta transfer works regardless of change position in file

---

## 2. What Syncthing Does (The Benchmark)

Syncthing's core algorithm:

1. **Block protocol**: Files split into 128KB blocks, each with a BLAKE2b hash.
2. **Block exchange**: Devices share block hashes. Only missing blocks are transferred.
3. **Weak hash detection**: Bekker hash detects shifted content within blocks.
4. **Peer-to-peer discovery**: Global Discovery Server, local mDNS, relays.
5. **NAT traversal**: BEP (Block Exchange Protocol) over QUIC/TCP with hole punching.
6. **Continuous sync**: File system watcher + scanner with configurable intervals.
7. **Conflict files**: `.sync-conflict-` files preserve both versions.
8. **File versioning**: `.stversions` archive with trash can, simple, staggered, or external versioning.
9. **Folder sharing**: Share specific folders with specific devices.
10. **Relaying**: When direct connection fails, relay servers proxy traffic.

---

## 3. Suture's Sync Architecture

### 3.1 Sync Daemon (`suture-daemon`)

The daemon provides three modes:

| Mode | Components | Description |
|------|-----------|-------------|
| Full | Watch + Sync + SHM | File watching, auto-commit, auto-sync, shared memory status |
| Watch-only | Watch + Auto-commit | File changes detected and committed, no remote sync |
| Sync-only | Auto-sync | Periodic push/pull without file watching |

**File Watching:**
- Uses `notify` crate with recursive watching
- 500ms configurable debounce window
- Ignores `.suture/` directory changes
- Batches rapid changes into single commits

**Auto-Sync Cycle (default: 60 seconds):**
1. Pull: `PullRequest` with known branch tips to `{remote}/pull/compressed`
2. Receive patches, branches, blobs (Zstd-compressed)
3. Store blobs in CAS, patches in metadata, update DAG and branches
4. Call `sync_working_tree()` to update working directory
5. Push: Collect patches since last push, gather referenced blobs, compress, send

**SHM Status:**
- 176-byte `#[repr(C)]` struct via `memmap2`
- Cross-process readable by CLI (`suture sync status`)
- Reports: repo_count, total_patches, total_blobs, head_branch, is_mounted, last_commit_ts, last_sync_ts, pid

### 3.2 Delta Encoding (`suture-protocol`)

**Algorithm: Prefix/Suffix Matching**

```
compute_delta(base: &[u8], target: &[u8]):
  1. prefix_len = count matching bytes from start
  2. suffix_len = count matching bytes from end (excluding prefix overlap)
  3. changed = target[prefix_len .. len - suffix_len]
  4. If changed.len() < target.len():
       emit DELTA(0x01, prefix_len, suffix_len, target_len, changed_bytes)
     Else:
       emit FULL_BLOB(0x00, target_bytes)
```

**Performance characteristics:**

| Change Pattern | Efficiency | Example |
|---------------|------------|---------|
| Append-only | Excellent | Log files, CSV with new rows at end |
| Prepend-only | Excellent | Stack traces with new frames |
| Small change at edges | Good | Header changes, footer additions |
| Change in the middle | **Poor** | JSON key reordering, CSV row insert |
| Scattered changes | **Poor** | Multi-site edits in a document |
| Complete rewrite | Full transfer | File format conversion |

**This is the critical gap.** rsync's rolling hash finds matching blocks at any offset. Suture's prefix/suffix only finds matches at the edges. For structured files where changes are scattered (the common case for Suture's target use case), this provides minimal benefit.

### 3.3 Protocol V2

V2 adds capability negotiation and delta-aware transfers:

- `ClientCapabilities` / `ServerCapabilities` (supports_delta, supports_compression, max_blob_size)
- `known_blob_hashes`: Client tells server which blobs it already has; server skips sending them
- `BlobDelta`: Delta packets referencing base blobs

This provides **whole-blob deduplication** (skip blobs the client already has) but not **intra-blob deduplication** (skip unchanged portions within a blob).

### 3.4 Compression

All blob data is Zstd-compressed at level 3 (~500 MB/s throughput). This provides:
- Typically 2-5x size reduction for text-based formats
- Minimal benefit for already-compressed formats (XLSX, DOCX, images)
- Applied during transport and storage

---

## 4. Feature-by-Feature Comparison

### 4.1 vs rsync

| Feature | rsync | Suture | Gap |
|---------|-------|--------|-----|
| **Block-level deduplication** | Rolling checksum + MD5 | None | **Critical** |
| **Delta transfer** | Any-offset matching | Prefix/suffix only | **Critical** |
| **Bandwidth limiting** | `--bwlimit` | None | Medium |
| **Resume on failure** | `--partial`, `--append-verify` | None (restarts) | **High** |
| **Checksum verification** | `-c` post-transfer | BLAKE3 verify-on-read (configurable) | Small |
| **Compression** | `-z` (zlib) | Zstd level 3 (better) | Suture ahead |
| **Exclude patterns** | `--exclude` globs | `.sutureignore` globs | Parity |
| **Hardlink preservation** | `-H` | None | Medium |
| **Symlink handling** | `-l` | Minimal (worktree symlinks) | Medium |
| **Permissions/ownership** | `-a`, `-p`, `-o`, `-g` | None | Medium |
| **Device files** | `--devices` | None | Low (not target use case) |
| **Delete propagation** | `--delete` | Patch-based (deletes are patches) | Different model |
| **Incremental transfer** | Block-level | Whole-blob (with V2 skip-if-present) | **Critical** |
| **SSH transport** | Yes | HTTP + TLS (reqwest) | Medium |
| **Streaming** | Yes | No (entire blobs in JSON body) | **High** |
| **Multi-host** | Yes (one-to-many) | Hub-based (many-to-one) | Different model |
| **Dry run** | `--dry-run` | `--dry-run` on merge | Partial |

### 4.2 vs Syncthing

| Feature | Syncthing | Suture | Gap |
|---------|-----------|--------|-----|
| **Block protocol** | 128KB blocks + BLAKE2b | Whole blobs | **Critical** |
| **Rolling/weak hash** | Bekker hash for shifted content | None | **Critical** |
| **Peer-to-peer** | BEP protocol, device-to-device | Client-server (Hub) | **High** |
| **Device discovery** | Global Discovery, local mDNS | Manual URL configuration | **High** |
| **NAT traversal** | UPnP, hole punching, relays | None (requires direct connectivity) | **High** |
| **Folder sharing** | Per-device, per-folder | Per-repo on Hub | Medium |
| **Continuous sync** | File watcher + scanner | File watcher + interval sync | Small |
| **Conflict files** | `.sync-conflict-*` preservation | ConflictNode in DAG | Different model (better) |
| **File versioning** | `.stversions` archive | Stash, reflog, full patch history | Suture ahead |
| **Ignore patterns** | `.stignore` | `.sutureignore` | Parity |
| **Send/receive only** | Per-folder direction | Branch protection | Different model |
| **Database encryption** | At-rest encryption | TLS transport only | Medium |
| **Web UI** | Full management dashboard | Basic Hub SPA | Medium |
| **Auto-upgrade** | Yes | None | Low |
| **Multi-device** | Unlimited peers | Hub-mediated | Different model |
| **Offline queue** | Syncs on reconnect | Silently logs warning | **High** |

---

## 5. What Suture Does Better Than rsync/Syncthing

| Feature | Advantage |
|---------|-----------|
| **Version history** | Full DAG-based history with branch/merge. rsync/Syncthing have no version concept. |
| **Semantic merge** | 19 format-aware drivers resolve conflicts that rsync/Syncthing would lose data on. |
| **Audit trail** | Every change is a signed, content-hashed patch with author and timestamp. |
| **Branching** | Branch, merge, rebase, cherry-pick -- impossible in rsync/Syncthing. |
| **Integrity** | Ed25519 signing of push operations. Tamper-evident audit log. |
| **Search** | Query patch history by author, message, time range. |
| **Selective sync** | Sync specific branches, not entire repos. |
| **Compliance** | Defence classification scanning, audit log, webhook notifications. |

---

## 6. Architecture Mismatch

The fundamental difference:

```
rsync/Syncthing:  File A (local) <--delta--> File A (remote)
Suture:           Patch history (local) <--patches+blobs--> Patch history (Hub)
```

- **rsync/Syncthing** synchronize file state. They compare files, find differences, and transfer only what changed.
- **Suture** synchronizes patch history. It transfers patches (operations) and blobs (file content snapshots). The "delta" is between blob versions, not between files.

This means:
- Suture **preserves history** (rsync/Syncthing do not)
- Suture **supports branching and merging** (rsync/Syncthing do not)
- Suture is **less bandwidth-efficient** for simple file sync (whole blobs vs blocks)
- Suture is **more bandwidth-efficient** when the receiver already has similar patches (V2 known_blob_hashes)

---

## 7. Sync Failure Modes

### Current Behavior

| Scenario | Suture Behavior | rsync/Syncthing Behavior |
|----------|-----------------|--------------------------|
| Network interruption during transfer | Entire push/pull fails, must restart | rsync: resume with --partial. Syncthing: resume at block boundary |
| Concurrent edits by two users | Both commits succeed; conflict detected on merge | rsync: last-write-wins (data loss). Syncthing: conflict file created |
| Remote server unreachable | Log warning, skip cycle, retry next interval | Syncthing: queue changes, sync on reconnect |
| Disk full during pull | Partial state (some blobs stored) | rsync: --partial leaves partial file. Syncthing: atomic rename |
| Merge conflict during auto-sync | `sync_working_tree()` may fail silently | N/A (no merge concept) |

### Missing Reliability Features

1. **No resume capability**: No checkpoint/progress for interrupted transfers
2. **No offline queue**: Changes are not queued for later sync
3. **No atomic pull**: If blob storage succeeds but DAG update fails, state is inconsistent
4. **No conflict auto-resolution during sync**: Merge is manual
5. **No retry with backoff**: Single attempt per sync cycle

---

## 8. Large File Handling

| Feature | Status | Details |
|---------|--------|---------|
| LFS protocol | Implemented | Hub supports batch upload/download of large objects |
| S3 multipart upload | Implemented | 8MB threshold, 5MB parts, 3 retries with backoff |
| BLAKE3 streaming hash | Implemented | 64KB chunks, no full-file memory load |
| CAS blob cache | Implemented | In-memory LRU (1024 entries) |
| Pack files | Implemented | Repack loose blobs into pack files |
| Chunked blob transfer | **Missing** | Entire blobs sent in single JSON body |
| Binary delta | **Missing** | No intra-blob delta for large files |
| Streaming FUSE reads | **Missing** | All files loaded into memory on mount |
| Client-side LFS pointers | **Missing** | LFS is server-side only |

---

## 9. Network Topology

### Current: Star Topology

```
  Client A ----\
  Client B -----+----> Hub (SQLite/S3)
  Client C ----/
```

All sync goes through a central Hub. No peer-to-peer capability.

### rsync: Point-to-Point

```
  Source -----> Destination
```

One-way sync. No central server needed.

### Syncthing: Mesh Topology

```
  Device A <--> Device B
     ^             ^
     v             v
  Device C <--> Device D
```

Every device can sync with every other device. No central server.

### Implication

Suture's star topology is appropriate for team workflows (everyone syncs to a central hub) but inappropriate for personal file sync (device-to-device) or backup scenarios (one-way to storage).

---

## 10. Roadmap to rsync/Syncthing Parity

### Phase 1: Efficient Transfer (4-6 weeks)

- Implement rolling hash (Adler32/Rabin-Karp) for block-level delta encoding
- Replace prefix/suffix delta with block-matching delta
- Add streaming blob transfer (chunked encoding, not whole-blob-in-JSON)
- Benchmark against rsync on real-world structured files

### Phase 2: Reliability (3-4 weeks)

- Transfer resume: checkpoint progress, resume from last checkpoint
- Offline queue: buffer changes when remote is unreachable, auto-sync on reconnect
- Atomic pull: transaction-like semantics (all-or-nothing for patch+blob application)
- Conflict auto-resolution during auto-sync (semantic merge on pull)

### Phase 3: Network (4-6 weeks)

- Bandwidth limiting (token bucket in transport layer)
- Peer-to-peer transport option (QUIC or TCP with hole punching)
- Device discovery (mDNS for local, optional global discovery)
- NAT traversal (UPnP + relay servers)

### Phase 4: File System Integration (3-4 weeks)

- Streaming FUSE reads (lazy blob retrieval from Hub)
- File permission/ownership preservation
- Hardlink/symlink preservation during sync
- Partial file transfer for large files

**Total estimated effort: 14-20 weeks for basic rsync parity. 20-30 weeks for Syncthing parity.**

---

## 11. Recommended Approach

Suture should NOT attempt to be a general-purpose rsync/Syncthing replacement. Instead, it should focus on its unique advantage: **semantic version control for structured files**.

The recommended sync strategy:

1. **Keep the current patch-sync architecture** -- it provides history, branching, merge
2. **Add rolling-hash delta encoding** -- this is the single highest-ROI improvement
3. **Add transfer resume** -- second highest ROI for reliability
4. **Add offline queue** -- third highest ROI for usability
5. **Do NOT pursue peer-to-peer** -- star topology is correct for the team workflow use case
6. **Do NOT pursue NAT traversal** -- delegate to VPN/overlay networks
7. **Position as "Git + rsync for structured data"** -- not as a standalone rsync competitor

This gives Suture a unique position: **the only tool that provides semantic merge, version history, AND efficient sync for structured files.**

---

## 12. Assessment by Use Case

| Use Case | rsync | Syncthing | Suture | Recommendation |
|----------|-------|-----------|--------|----------------|
| **Backup personal files** | Best | Good | Poor | Use rsync |
| **Sync team config files** | Poor (no merge) | Poor (no merge) | **Best** | Use Suture |
| **Sync K8s manifests** | Poor (no merge) | Poor (no merge) | **Best** | Use Suture |
| **Sync Office documents** | Last-write-wins | Conflict files | **Best** (semantic merge) | Use Suture |
| **Mirror directory tree** | Best | Good | Medium | Use rsync |
| **Continuous file sync** | Poor (cron-based) | **Best** | Medium | Use Syncthing |
| **Multi-device personal sync** | Manual | **Best** | Poor (needs Hub) | Use Syncthing |
| **Database schema evolution** | N/A | N/A | **Best** | Use Suture |
| **Large media files** | **Best** (block-level) | Good | Poor (no binary delta) | Use rsync |

---

## 13. Conclusion

Suture's sync capabilities are adequate for its primary mission: **version-controlled semantic merge for structured files within a team**. The auto-sync daemon provides a reasonable experience for small teams with reliable network connectivity to a Hub.

However, Suture is **not a competitive file synchronization tool** in the rsync/Syncthing sense. The delta encoding algorithm is too simplistic (prefix/suffix vs rolling hash), there is no resume capability, no offline queue, no bandwidth limiting, and no peer-to-peer support.

The path to competitiveness is clear: implement rolling-hash delta encoding and transfer resume (6-10 weeks), and Suture becomes a compelling alternative for teams that need both version control and file synchronization for structured data. Attempting to match rsync/Syncthing feature-for-feature is not recommended -- the architecture should leverage Suture's unique semantic merge advantage rather than competing on raw transfer efficiency.
