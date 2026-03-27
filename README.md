# Suture: Universal Semantic Version Control System (USVCS)

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust: Stable](https://img.shields.io/badge/Rust-Stable-orange.svg)](https://www.rust-lang.org)
[![Build: Nix](https://img.shields.io/badge/Build-Nix_Flake-blueviolet.svg)](https://nixos.org)

**Suture** is a high-performance, deterministic version control engine designed for the next century of collaborative work. Engineered by a team of systems veterans with backgrounds in High-Frequency Trading (HFT), Suture transcends the limitations of snapshot-based systems like Git to provide **Semantic Patch Theory** for complex, non-textual data.

Whether it is a multi-terabyte DaVinci Resolve timeline, a mission-critical financial model in Excel, or a multi-layered VFX composition in After Effects, Suture ensures mathematical consistency, zero-copy efficiency, and sub-millisecond metadata operations.

---

## 1. The Thesis: Beyond the Binary Blob

Traditional Version Control Systems (VCS) treat non-textual data as opaque binary objects. This leads to:
*   **Merge Paralysis:** Concurrent edits result in total file conflicts, requiring manual, destructive overrides.
*   **Path Inflexibility:** Hard-coded absolute paths in project metadata cause broken links across disparate OS environments.
*   **Storage Bloat:** Minor changes result in redundant full-file snapshots.

**Suture resolves this by treating files as a sequence of commutative operations.**

---

## 2. Core Architectural Pillars

### 2.1. Semantic Patch Theory
Inspired by Pijul and Category Theory, Suture does not store "states" of a file. It stores **Patches**. 
*   **Commutativity:** If Operation A (moving a clip) and Operation B (grading a clip) do not depend on each other, they commute. $A \circ B = B \circ A$.
*   **Deterministic Merging:** Merging is a set-union of independent patches, mathematically eliminating "merge hell" for structured data.

### 2.2. The Invisible VFS (Virtual File System)
Suture utilizes a user-space **NFSv4/SMB3 loopback server** and **Windows ProjFS** to mount projects.
*   **OS Agnostic:** Works natively in Docker, WSL2, macOS, and Windows.
*   **Dynamic Relinking:** The VFS intercepts file I/O calls to translate logical project paths into physical local storage paths in real-time. Media is never "offline."
*   **Zero-Copy Branching:** Leverages Reflinks/Copy-on-Write (CoW) at the kernel level for near-instantaneous branching of massive datasets.

### 2.3. HFT-Grade Performance
*   **Language:** Pure, memory-safe **Rust**.
*   **Hashing:** **BLAKE3** for SIMD-accelerated, cryptographic-grade content addressing.
*   **Serialization:** **Flatbuffers** for zero-copy access to metadata without deserialization overhead.
*   **Concurrency:** Lock-free data structures and Shared Memory (SHM) IPC for nanosecond-level status lookups.

---

## 3. System Stack

| Layer | Technology |
| :--- | :--- |
| **Core Logic** | Rust (`libsuture`) |
| **Metadata DB** | Embedded SQLite (WAL Mode) |
| **Networking** | gRPC over QUIC (`tonic` + `quinn`) |
| **File Virtualization** | NFSv4 (Loopback), Windows ProjFS |
| **Serialization** | Flatbuffers, Zstd (Compression) |
| **Environment** | Nix Flakes, Fish Shell |

---

## 4. Repository Structure

Suture follows an **Open Core** model. This repository contains the foundational engine.

*   `crates/suture-core`: The central library handling Patch-DAG and CAS.
*   `crates/suture-daemon`: Background service managing VFS and state orchestration.
*   `crates/suture-cli`: Command-line interface for advanced automation.
*   `crates/suture-driver-otio`: The reference implementation for Video Editorial (OpenTimelineIO).

*For the Raft-based distributed coordination, enterprise security (SSO/RBAC), and visual web-hub, see **Suture Enterprise**.*

---

## 5. Getting Started

### Prerequisites
Suture requires **Nix** with Flakes enabled for a deterministic build environment.

```fish
# Clone the repository
git clone https://github.com/Suture-VCS/suture.git
cd suture

# Enter the deterministic shell
nix develop # or 'direnv allow' if installed
```

### Initializing a Project
```fish
# Initialize a new Suture repository
suture init my-project

# Mount the project VFS
suture mount my-project /mnt/suture/my-project

# Check status (HFT-optimized ticker)
suture status --live
```

---

## 6. The Semantic Driver SDK

Suture is extensible. By implementing the `SutureDriver` trait, you can add semantic versioning support for any structured format:

```rust
pub trait SutureDriver {
    fn serialize(path: &Path) -> Result<Patch, DriverError>;
    fn deserialize(patch: &Patch, target: &Path) -> Result<(), DriverError>;
    fn visualize(patch: &Patch) -> VisualDiff; // For the Suture Hub Web UI
}
```

Current and planned drivers:
*   [x] **Editorial:** OpenTimelineIO (.otio)
*   [ ] **Spreadsheets:** Excel (.xlsx / .csv)
*   [ ] **Documents:** Word (.docx)
*   [ ] **3D/CAD:** USD (Universal Scene Description)

---

## 7. Development Philosophy

As retired HFT engineers, we adhere to the following:
1.  **Determinism is non-negotiable:** Every operation must be idempotent.
2.  **Latency is a bug:** Metadata operations should never block the creative workflow.
3.  **Audit everything:** Every suture (merge) is cryptographically signed and immutable.

---

## 8. License

Project Suture is licensed under the **Apache License, Version 2.0**. See [LICENSE](LICENSE) for details.

---

**Suture: Joining the fragments of collaboration into a single, immutable truth.**