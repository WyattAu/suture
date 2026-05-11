As a team of retired HFT engineers, your technical requirements must prioritize **determinism, cache-locality, zero-copy data paths, and nanosecond-level auditing.**

The following is a comprehensive breakdown of the technical requirements for **Project Suture**, categorized by architectural layer.

---

> **Notation Guide**
>
> This document contains the original aspirational ("HFT-Spec") requirements alongside annotations reflecting the current implementation:
>
> - `[DEFERRED]` — Planned but not yet implemented; a simpler or alternative approach is used today.
> - `[NOT IMPLEMENTED]` — Not present in the codebase; may be revisited in a future phase.
>
> Aspirational items are intentionally preserved for long-term roadmap tracking.

### 1. Core Engine Requirements (`libsuture`)
The heart of the system must be a high-performance, thread-safe Rust library.
*   **Storage Model:**
    *   **CAS (Content Addressable Storage):** BLAKE3 hashing for SIMD-accelerated file integrity checks.
    *   **Metadata DB:** Embedded SQLite (via `rusqlite`) for relational queries, utilizing WAL (Write-Ahead Logging) mode for high-concurrency.
    *   **Serialization:** [DEFERRED] FlatBuffers for zero-copy deserialization of patch data; **Zstd** for delta-compression of binary project files. — Current: bincode + Zstd serialization. FlatBuffers deferred in favor of bincode's ergonomics; Zstd compression is implemented.
*   **Versioning Logic:**
    *   **Patch-DAG:** Implementation of a Directed Acyclic Graph where nodes are commutative patches, not full snapshots.
    *   **Algebraic Commutation:** Logic to verify if two patches $P_1$ and $P_2$ can be applied in any order ($P_1 \circ P_2 = P_2 \circ P_1$).
    *   **Conflict State:** Support for "First-Class Conflict" nodes in the DAG that preserve divergent states without corrupting the working tree.

### 2. Access Layer & VFS Requirements
The "Invisible" layer that handles how the OS and NLEs see the data.
*   **Virtualization:**
    *   [DEFERRED] **NFSv4/SMB3 Server:** A user-space implementation in Rust (e.g., `nfs-serve-rs`) to allow local loopback mounting on macOS/Windows/Linux/Docker. — Current: FUSE3 + WebDAV. NFSv4/SMB3 deferred; WebDAV provides cross-platform mounting (macOS Finder, Windows Explorer).
    *   [DEFERRED] **Windows ProjFS:** Integration with the Windows Projected File System API for native-speed file virtualization. — Current: FUSE3 on Linux, WebDAV cross-platform. ProjFS deferred; WebDAV covers Windows via Explorer integration.
    *   **FUSE Fallback:** `fuse3` support for legacy Linux environments.
*   **I/O Optimization:**
    *   **Reflink/CoW Support:** Atomic cloning using `ioctl` (FICLONERANGE) on XFS/Btrfs, APFS (macOS), and ReFS (Windows) for instantaneous branching.
    *   **Path Translation Engine:** A high-speed regex-based mapping engine to translate logical project paths to physical device paths at the syscall level.

### 3. Networking & Distributed Systems Requirements
The infrastructure for the "Hub" and multi-user synchronization.
*   **Transport:**
    *   [DEFERRED] **gRPC over QUIC:** Use the `quinn` crate for encrypted, multiplexed, low-latency communication that handles packet loss more gracefully than TCP (essential for remote editors). — Current: TCP (tonic) + Zstd compression. QUIC deferred; tonic over TCP is stable and Zstd delta-compression mitigates latency concerns.
*   **Consensus & Locking:**
    *   **Raft Implementation:** Use `raft-rs` or `kanidm-raft` for distributed leader election and atomic lock acquisition.
    *   **Lease Management:** TTL-based binary locking with heartbeat mechanisms to prevent "zombie locks" if a client crashes.
*   **Cloud Architecture (The Hub):**
    *   **S3 Backend:** Support for S3-compatible object storage for blob persistence.
    *   [DEFERRED] **PostgreSQL:** Distributed relational store for global project state and user permissions. — Current: SQLite with WAL mode. PostgreSQL deferred; SQLite suffices for current deployment scale (single-node and small clusters).
    *   [DEFERRED] **Redis:** For real-time "Liveness" tracking (who is currently in which sequence). — Current: SQLite-backed rate limiter. Redis deferred; in-process tracking via SQLite meets current needs.

### 4. Integration & SDK Requirements (The Drivers)
How Suture communicates with specific professional applications.
*   **The Bridge Layer:**
    *   **PyO3 Bindings:** To allow DaVinci Resolve's Python API to call `libsuture` directly without IPC overhead.
    *   **Node-API / C-ABI:** For integration with Adobe UXP (After Effects) and Excel Add-ins.
*   **The Driver Specification:**
    *   **Semantic Parsers:** High-speed XML/JSON parsers for `.otio` (Video), `.xlsx` (Excel), and `.docx` (Word).
    *   **Visual Diff Engine:** Logic to calculate "Visual Diffs" (filmstrip-style for video, grid-style for spreadsheets) to be rendered in the Web UI.

### 5. IPC & Local Performance Requirements
Requirements for communication between the NLE and the Suture Daemon.
*   **Zero-Copy IPC:** Implementation of **Shared Memory (SHM)** segments for high-frequency status updates (e.g., "Is this file currently locked?") to avoid context-switching.
*   **Atomic Operations:** Use of atomic primitives for lock-free progress tracking during large file pushes/pulls.
*   **Memory-Mapped I/O (mmap):** For rapid reading of large metadata databases.
*   [DEFERRED] **iceoryx-rs:** Zero-copy shared-memory communication for nanosecond IPC. — Current: memmap2 for SHM. iceoryx-rs deferred; memmap2 provides sufficient SHM performance for daemon↔client status queries.

### 6. Security & Compliance Requirements
*   **Cryptographic Identity:** Every commit/patch must be signed with **Ed25519** keys.
*   **TLS 1.3:** Mandatory for all traffic between the Daemon and the Hub.
*   **Audit Logging:** Immutable, append-only logs of every Raft transition and CAS write, exportable for SOC2/ISO compliance.
*   **Data Sovereignty:** Native support for "On-Premise Only" mode where the Hub runs within a disconnected local network (air-gapped).

### 7. User Interface (The Hub & Desktop)
*   **Web Frontend:** React or SvelteKit optimized for **WebAssembly (WASM)** to handle complex timeline rendering in the browser.
*   **Desktop App:** **Tauri** (Rust + WebView) for a tiny memory footprint, acting as the control center for VFS mounts and local repository status.
*   **CLI:** A "Git-style" CLI for power users, built with `clap` in Rust.

---

### Summary of the "HFT-Spec" Toolchain:
*   **Language:** Rust (Latest Stable)
*   **Async Runtime:** `tokio`
*   **Hashing:** `blake3`
*   **Database:** `sqlite` (Local), [DEFERRED] `postgresql` (Hub) — Current: SQLite with WAL mode
*   **Communication:** `tonic` (gRPC), [DEFERRED] `quinn` (QUIC) — Current: TCP (tonic) + Zstd compression
*   **IPC:** [DEFERRED] `iceoryx-rs` or custom SHM — Current: memmap2 for SHM
*   **Virtualization:** [DEFERRED] `nfs-serve-rs`, `fuse3`, `projfs` — Current: FUSE3 on Linux, WebDAV cross-platform

This stack ensures that **Suture** isn't just a "software tool," but an industrial-grade piece of infrastructure capable of handling the most demanding data environments in the world.
