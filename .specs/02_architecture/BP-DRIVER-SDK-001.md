---
document_id: BP-DRIVER-SDK-001
version: 1.0.0
status: APPROVED
ieee_1016_compliant: true
component_id: COMP-DRIVER-001
component_type: SDK
interfaces: [IF-DRIVER-001]
depends_on:
  yellow_papers: [YP-ALGEBRA-PATCH-001]
  blue_papers: [BP-PATCH-ALGEBRA-001]
created: 2026-03-27
---

# BP-DRIVER-SDK-001: SutureDriver SDK

## BP-1: Design Overview

The SutureDriver SDK defines the interface through which format-specific plugins interact
with Suture's patch algebra engine. Each driver is responsible for translating between a
file format (e.g., OpenTimelineIO `.otio` files, CSV spreadsheets, USD scene graphs) and
Suture's generic `Patch` representation.

Drivers are loaded dynamically based on file extension mapping configured in the repository
(REQ-DRIVER-006). The SDK provides a trait definition, a driver registry, and error types
that all driver implementations must conform to.

The SDK is delivered as part of the `suture-core` library crate and can be consumed by
both Rust-native drivers and Python-based drivers via PyO3 bindings (REQ-DRIVER-007).

---

## BP-2: Design Decomposition

### 2.1 Core Trait (`trait.rs`)

The `SutureDriver` trait is the central abstraction:

```rust
pub trait SutureDriver: Send + Sync {
    fn name(&self) -> &str;

    fn supported_extensions(&self) -> &[&str];

    fn serialize(&self, path: &Path) -> Result<Vec<Patch>, DriverError>;

    fn deserialize(&self, patches: &[Patch], target: &Path) -> Result<(), DriverError>;

    fn touch_set(&self, patch: &Patch) -> Result<Vec<String>, DriverError>;

    fn visual_diff(&self, base: &Patch, other: &Patch) -> Result<VisualDiff, DriverError>;
}
```

### 2.2 Driver Registry (`registry.rs`)

Manages driver discovery and selection by file extension:

```rust
pub struct DriverRegistry {
    drivers: HashMap<String, Box<dyn SutureDriver>>, // extension → driver
}

impl DriverRegistry {
    pub fn register(&mut self, extension: &str, driver: Box<dyn SutureDriver>);
    pub fn get(&self, extension: &str) -> Result<&dyn SutureDriver, DriverError>;
    pub fn list(&self) -> Vec<(&str, &dyn SutureDriver)>;
}
```

### 2.3 Error Types (`error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("unsupported file extension: {0}")]
    UnsupportedExtension(String),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("driver not found for extension: {0}")]
    DriverNotFound(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

### 2.4 Visual Diff Types (`visual_diff.rs`)

```rust
pub struct VisualDiff {
    pub hunks: Vec<DiffHunk>,
    pub summary: DiffSummary,
}

pub struct DiffHunk {
    pub address: String,         // Semantic address (e.g., "timeline.clip.3.start_time")
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub hunk_type: DiffHunkType,
}

pub enum DiffHunkType {
    Added,
    Removed,
    Modified,
    Moved,
}

pub struct DiffSummary {
    pub additions: usize,
    pub removals: usize,
    pub modifications: usize,
    pub moves: usize,
}
```

---

## BP-3: Design Rationale

### 3.1 Trait-Based Plugin Architecture

The `SutureDriver` trait is chosen over a procedural macro or code-generation approach
because:
1. **Language flexibility**: Drivers can be written in Rust (native) or Python (via PyO3).
2. **Dynamic loading**: File extension mapping is configurable per-repository.
3. **Testability**: Mock drivers can be injected for testing the core engine.

### 3.2 Touch Set Computation

The `touch_set()` method allows drivers to provide format-specific semantic addresses
(e.g., `"timeline.track_0.clip_5.start_time"` for OTIO). This is more meaningful than
byte offsets for conflict detection and visual diffs.

---

## BP-4: Traceability

| Requirement | Satisfied By | Verification Method |
|-------------|-------------|-------------------|
| REQ-DRIVER-001 (SutureDriver trait) | BP-2.1, trait definition | Compile-time |
| REQ-DRIVER-002 (serialize) | BP-2.1, serialize() | Unit tests |
| REQ-DRIVER-003 (deserialize) | BP-2.1, deserialize() | Unit tests |
| REQ-DRIVER-004 (visualize) | BP-2.1, visual_diff() | Unit tests |
| REQ-DRIVER-005 (OTIO reference driver) | Separate crate | Integration tests |
| REQ-DRIVER-006 (per-extension config) | BP-2.2, DriverRegistry | Unit tests |
| REQ-DRIVER-007 (PyO3 bindings) | PyO3 bridge module | Integration tests |

---

## BP-5: Interface Design

### IF-DRIVER-001: SutureDriver Trait

```rust
/// Format-specific driver for translating between file formats and Suture patches.
///
/// Implementations must be Send + Sync for concurrent use across threads.
pub trait SutureDriver: Send + Sync {
    /// Human-readable driver name (e.g., "OpenTimelineIO", "CSV", "USD").
    fn name(&self) -> &str;

    /// File extensions this driver handles (e.g., [".otio", ".csv"]).
    fn supported_extensions(&self) -> &[&str];

    /// Parse a file and produce a list of patches representing its semantic content.
    ///
    /// Precondition: `path` exists and is a supported file type.
    /// Postcondition: Each returned Patch has a valid, non-empty touch set.
    /// Postcondition: The concatenation of all patches, when applied to an empty state,
    ///   reconstructs the file content.
    fn serialize(&self, path: &Path) -> Result<Vec<Patch>, DriverError>;

    /// Apply patches to produce a file at the target path.
    ///
    /// Precondition: `patches` is a well-formed patch set.
    /// Postcondition: The file at `target` represents the state after applying all patches.
    fn deserialize(&self, patches: &[Patch], target: &Path) -> Result<(), DriverError>;

    /// Extract the semantic touch set from a patch.
    ///
    /// Postcondition: Returns the set of addresses modified by this patch.
    fn touch_set(&self, patch: &Patch) -> Result<Vec<String>, DriverError>;

    /// Produce a visual diff between two patches for UI rendering.
    ///
    /// Precondition: Both patches have compatible operation types.
    /// Postcondition: Returns a structured diff suitable for terminal or GUI rendering.
    fn visual_diff(&self, base: &Patch, other: &Patch) -> Result<VisualDiff, DriverError>;
}
```

---

## BP-6: Data Design

Drivers operate on the `Patch` type defined in BP-PATCH-ALGEBRA-001. No additional
persistent data is required. The driver registry mapping (extension → driver name) is
stored in the `config` table of the metadata store:

```sql
INSERT INTO config (key, value) VALUES ('driver.otio', 'suture-driver-otio');
INSERT INTO config (key, value) VALUES ('driver.csv', 'suture-driver-csv');
```

---

## BP-7: Component Design

```
suture-core/src/
  driver/
    mod.rs              -- SutureDriver trait, DriverRegistry, re-exports
    trait.rs            -- Trait definition
    registry.rs         -- Extension-based driver lookup
    error.rs            -- DriverError enum
    visual_diff.rs      -- VisualDiff, DiffHunk, DiffSummary
```

---

## BP-8: Deployment

The SDK is part of `suture-core`. Reference driver implementations are separate crates:

```
suture-driver-otio/    -- OpenTimelineIO driver (REQ-DRIVER-005)
suture-driver-csv/     -- CSV/spreadsheet driver (future)
suture-driver-usd/     -- Universal Scene Description driver (future)
```

---

## BP-9: Formal Verification

The driver SDK has no algebraic invariants of its own. Correctness is ensured by:
1. **Trait contract enforcement**: The Rust type system ensures all implementations
   provide the required methods.
2. **Round-trip invariant**: For every driver, `deserialize(serialize(file))` must produce
   a file that `serialize()` maps to an equivalent patch set. This is tested per-driver.
3. **Touch set consistency**: The touch set returned by `touch_set()` must match the
   addresses actually modified by the patch's operation.

---

## BP-11: Compliance Matrix

| Requirement | Section | Status | Verification |
|-------------|---------|--------|-------------|
| REQ-DRIVER-001 | BP-2.1 | Satisfied | Compile-time |
| REQ-DRIVER-002 | BP-2.1 | Satisfied | Per-driver tests |
| REQ-DRIVER-003 | BP-2.1 | Satisfied | Per-driver tests |
| REQ-DRIVER-004 | BP-2.1 | Satisfied | Per-driver tests |
| REQ-DRIVER-005 | BP-8 | Deferred | Separate crate |
| REQ-DRIVER-006 | BP-2.2 | Satisfied | Unit tests |
| REQ-DRIVER-007 | PyO3 module | Deferred | Integration tests |

---

## BP-12: Quality Checklist

- [ ] SutureDriver trait is Send + Sync.
- [ ] DriverRegistry correctly maps extensions to drivers.
- [ ] Error types are comprehensive and include remediation hints.
- [ ] VisualDiff output is structured for terminal and GUI rendering.
- [ ] Mock driver implementation available for core engine testing.
- [ ] `cargo clippy` passes with zero warnings on the `driver` module.
- [ ] `cargo test` passes all driver SDK tests.

---

*End of BP-DRIVER-SDK-001*
