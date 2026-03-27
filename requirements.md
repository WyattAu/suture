As a team of retired HFT engineers, your technical requirements must prioritize **determinism, cache-locality, zero-copy data paths, and nanosecond-level auditing.**

The following is a comprehensive breakdown of the technical requirements for **Project Suture**, categorized by architectural layer.

---

### 1. Core Engine Requirements (`libsuture`)
The heart of the system must be a high-performance, thread-safe Rust library.
*   **Storage Model:**
    *   **CAS (Content Addressable Storage):** BLAKE3 hashing for SIMD-accelerated file integrity checks.
    *   **Metadata DB:** Embedded SQLite (via `rusqlite`) for relational queries, utilizing WAL (Write-Ahead Logging) mode for high-concurrency.
    *   **Serialization:** **Flatbuffers** for zero-copy deserialization of patch data; **Zstd** for delta-compression of binary project files.
*   **Versioning Logic:**
    *   **Patch-DAG:** Implementation of a Directed Acyclic Graph where nodes are commutative patches, not full snapshots.
    *   **Algebraic Commutation:** Logic to verify if two patches $P_1$ and $P_2$ can be applied in any order ($P_1 \circ P_2 = P_2 \circ P_1$).
    *   **Conflict State:** Support for "First-Class Conflict" nodes in the DAG that preserve divergent states without corrupting the working tree.

### 2. Access Layer & VFS Requirements
The "Invisible" layer that handles how the OS and NLEs see the data.
*   **Virtualization:**
    *   **NFSv4/SMB3 Server:** A user-space implementation in Rust (e.g., `nfs-serve-rs`) to allow local loopback mounting on macOS/Windows/Linux/Docker.
    *   **Windows ProjFS:** Integration with the Windows Projected File System API for native-speed file virtualization.
    *   **FUSE Fallback:** `fuse3` support for legacy Linux environments.
*   **I/O Optimization:**
    *   **Reflink/CoW Support:** Atomic cloning using `ioctl` (FICLONERANGE) on XFS/Btrfs, APFS (macOS), and ReFS (Windows) for instantaneous branching.
    *   **Path Translation Engine:** A high-speed regex-based mapping engine to translate logical project paths to physical device paths at the syscall level.

### 3. Networking & Distributed Systems Requirements
The infrastructure for the "Hub" and multi-user synchronization.
*   **Transport:**
    *   **gRPC over QUIC:** Use the `quinn` crate for encrypted, multiplexed, low-latency communication that handles packet loss more gracefully than TCP (essential for remote editors).
*   **Consensus & Locking:**
    *   **Raft Implementation:** Use `raft-rs` or `kanidm-raft` for distributed leader election and atomic lock acquisition.
    *   **Lease Management:** TTL-based binary locking with heartbeat mechanisms to prevent "zombie locks" if a client crashes.
*   **Cloud Architecture (The Hub):**
    *   **S3 Backend:** Support for S3-compatible object storage for blob persistence.
    *   **PostgreSQL:** Distributed relational store for global project state and user permissions.
    *   **Redis:** For real-time "Liveness" tracking (who is currently in which sequence).

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
*   **Database:** `sqlite` (Local), `postgresql` (Hub)
*   **Communication:** `tonic` (gRPC), `quinn` (QUIC)
*   **IPC:** `iceoryx-rs` or custom SHM
*   **Virtualization:** `nfs-serve-rs`, `fuse3`, `projfs`

This stack ensures that **Suture** isn't just a "software tool," but an industrial-grade piece of infrastructure capable of handling the most demanding data environments in the world.