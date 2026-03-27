---
document_id: BP-METADATA-001
version: 1.0.0
status: APPROVED
ieee_1016_compliant: true
component_id: COMP-META-001
component_type: Module
interfaces: [IF-META-001]
depends_on:
  yellow_papers: []
  blue_papers: [BP-PATCH-DAG-001]
  external_libs: [rusqlite-0.39]
created: 2026-03-27
---

# BP-METADATA-001: SQLite Metadata Store

## BP-1: Design Overview

The Metadata Store is the centralized persistence layer for all Suture repository state.
It uses SQLite in WAL (Write-Ahead Logging) mode to store DAG topology, branch pointers,
working set state, repository configuration, and schema versioning. The metadata database
resides within the `.suture/` directory (REQ-META-005).

All consumers of the Metadata Store — the Patch DAG (BP-PATCH-DAG-001), the CAS
(BP-CAS-001), and the CLI (BP-CLI-001) — interact through the `MetadataStore` struct,
which provides a typed API over raw SQL.

---

## BP-2: Design Decomposition

### 2.1 Schema Management (`schema.rs`)

Manages database creation and versioned migrations:

- `create(db: &Connection) -> Result<()>`: Creates all tables with the initial schema.
- `migrate(db: &Connection, from: u32, to: u32) -> Result<()>`: Applies sequential
  migrations from version `from` to version `to`.
- `current_version(db: &Connection) -> Result<u32>`: Reads the current schema version.

### 2.2 MetadataStore (`store.rs`)

The primary public API:

```rust
pub struct MetadataStore {
    pool: Arc<SqlitePool>,
}
```

### 2.3 Query Modules

- `patches.rs`: CRUD for the `patches` table.
- `branches.rs`: CRUD for the `branches` table.
- `config.rs`: Key-value configuration management.
- `working_set.rs`: Working set (staged/unstaged changes) tracking.

---

## BP-3: Design Rationale

### SQLite over PostgreSQL/Custom

| Criterion | SQLite | PostgreSQL | Custom |
|-----------|--------|------------|--------|
| Deployment | Zero-config | Requires server | N/A |
| Latency | Sub-ms (in-process) | Network RTT | Variable |
| Concurrency | WAL mode (1 writer, N readers) | MVCC | Custom impl |
| Portability | Single file | Requires daemon | N/A |
| Dependencies | rusqlite crate | tokio-postgres | N/A |

SQLite is chosen for its zero-configuration deployment, in-process execution (eliminating
network latency), and single-file portability. WAL mode provides sufficient concurrency for
Suture's access pattern (many readers, one writer). REQ-EXCL-003 explicitly excludes
PostgreSQL from v0.1 scope.

### WAL Mode

Write-Ahead Logging allows concurrent readers while a single writer holds the lock. This
aligns with REQ-CORE-004 (concurrent reads) and REQ-CORE-005 (single-writer pipeline).

---

## BP-4: Traceability

| Requirement | Satisfied By | Verification Method |
|-------------|-------------|-------------------|
| REQ-META-001 (SQLite WAL) | BP-3, connection config | Integration test |
| REQ-META-002 (DAG, branches, working set, config) | BP-5, schema | Integration test |
| REQ-META-003 (schema migrations) | BP-2.1, schema.rs | Integration test |
| REQ-META-004 (sub-ms latency) | In-process SQLite | Benchmarks |
| REQ-META-005 (.suture/ directory) | BP-1, file location | Integration test |
| REQ-META-006 (config persistence) | BP-5, config table | Unit tests |

---

## BP-5: Interface Design

### IF-META-001: MetadataStore Public API

```rust
impl MetadataStore {
    /// Open (or create) the metadata database at the given path.
    /// Creates .suture/metadata.db with WAL mode enabled.
    pub fn open(path: &Path) -> Result<Self, MetaError>;

    /// Run pending schema migrations.
    pub fn ensure_schema(&self) -> Result<(), MetaError>;

    // --- Patches ---
    pub fn insert_patch(&self, patch: &Patch) -> Result<(), MetaError>;
    pub fn get_patch(&self, id: &PatchId) -> Result<Option<Patch>, MetaError>;
    pub fn list_patches(&self) -> Result<Vec<Patch>, MetaError>;
    pub fn delete_patch(&self, id: &PatchId) -> Result<(), MetaError>;

    // --- Edges ---
    pub fn insert_edge(&self, parent: &PatchId, child: &PatchId) -> Result<(), MetaError>;
    pub fn get_children(&self, parent: &PatchId) -> Result<Vec<PatchId>, MetaError>;
    pub fn get_parents(&self, child: &PatchId) -> Result<Vec<PatchId>, MetaError>;

    // --- Branches ---
    pub fn create_branch(&self, name: &str, target: &PatchId) -> Result<(), MetaError>;
    pub fn get_branch(&self, name: &str) -> Result<Option<Branch>, MetaError>;
    pub fn list_branches(&self) -> Result<Vec<Branch>, MetaError>;
    pub fn update_branch_target(&self, name: &str, target: &PatchId) -> Result<(), MetaError>;
    pub fn delete_branch(&self, name: &str) -> Result<(), MetaError>;
    pub fn rename_branch(&self, old: &str, new: &str) -> Result<(), MetaError>;

    // --- Config ---
    pub fn get_config(&self, key: &str) -> Result<Option<String>, MetaError>;
    pub fn set_config(&self, key: &str, value: &str) -> Result<(), MetaError>;
    pub fn list_config(&self) -> Result<Vec<(String, String)>, MetaError>;

    // --- Working Set ---
    pub fn stage(&self, path: &str, patch_id: &PatchId) -> Result<(), MetaError>;
    pub fn unstage(&self, path: &str) -> Result<(), MetaError>;
    pub fn get_working_set(&self) -> Result<Vec<WorkingSetEntry>, MetaError>;
    pub fn clear_working_set(&self) -> Result<(), MetaError>;

    // --- Transactions ---
    pub fn transaction<F, T>(&self, f: F) -> Result<T, MetaError>
    where
        F: FnOnce(&Transaction) -> Result<T, MetaError>;
}
```

---

## BP-6: Data Design

### 6.1 Complete SQL Schema

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE patches (
    id TEXT PRIMARY KEY,              -- BLAKE3 hash (hex-encoded)
    parent_ids TEXT NOT NULL,         -- JSON array of parent patch IDs
    operation_type TEXT NOT NULL,     -- OpType enum as string
    touch_set TEXT NOT NULL,          -- JSON array of address strings
    payload BLOB,                     -- FlatBuffers-encoded operation data
    timestamp TEXT NOT NULL,          -- ISO 8601
    author TEXT NOT NULL,
    signature BLOB                   -- Ed25519 signature (64 bytes)
);

CREATE TABLE edges (
    parent_id TEXT NOT NULL REFERENCES patches(id) ON DELETE CASCADE,
    child_id TEXT NOT NULL REFERENCES patches(id) ON DELETE CASCADE,
    PRIMARY KEY (parent_id, child_id)
);

CREATE INDEX idx_edges_child ON edges(child_id);
CREATE INDEX idx_edges_parent ON edges(parent_id);

CREATE TABLE branches (
    name TEXT PRIMARY KEY,
    target_patch_id TEXT NOT NULL REFERENCES patches(id),
    created_at TEXT NOT NULL
);

CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE working_set (
    path TEXT PRIMARY KEY,
    patch_id TEXT NOT NULL REFERENCES patches(id),
    status TEXT NOT NULL              -- 'added', 'modified', 'deleted'
);
```

### 6.2 Schema Migration Framework

```sql
-- Migration 1: Initial schema (version 1)
-- (see above)

-- Future migrations add columns/tables with ALTER TABLE/CREATE TABLE.
-- Each migration increments the schema_version table.
```

---

## BP-7: Component Design

```
suture-core/src/
  meta/
    mod.rs              -- MetadataStore struct, public API, MetaError
    schema.rs           -- Schema creation and migration logic
    patches.rs          -- Patch CRUD queries
    branches.rs         -- Branch CRUD queries
    config.rs           -- Key-value config queries
    working_set.rs      -- Working set staging queries
```

---

## BP-8: Deployment

- **Location**: `.suture/metadata.db`
- **Mode**: WAL (Write-Ahead Logging)
- **Initialization**: Created during `suture init`
- **Backup**: Single-file copy suffices (no running server to coordinate)

---

## BP-9: Formal Verification

The Metadata Store is a CRUD layer with no algebraic invariants of its own. Correctness
is ensured by:

1. **Schema constraints**: PRIMARY KEY, FOREIGN KEY, NOT NULL constraints prevent
   invalid data at the database level.
2. **Transaction atomicity**: Multi-step operations (e.g., add patch + edges) use
   SQLite transactions to ensure all-or-nothing semantics.
3. **WAL mode**: Ensures readers never see partially-written data.

---

## BP-11: Compliance Matrix

| Requirement | Section | Status | Verification |
|-------------|---------|--------|-------------|
| REQ-META-001 | BP-3, WAL mode | Satisfied | Integration test |
| REQ-META-002 | BP-5, BP-6 | Satisfied | Integration test |
| REQ-META-003 | BP-2.1, schema.rs | Satisfied | Migration tests |
| REQ-META-004 | BP-3, in-process | Satisfied | Benchmarks |
| REQ-META-005 | BP-8, .suture/ | Satisfied | Integration test |
| REQ-META-006 | BP-5, config | Satisfied | Unit tests |
| REQ-CORE-005 | BP-3, single writer | Satisfied | Integration test |

---

## BP-12: Quality Checklist

- [ ] All tables have appropriate PRIMARY KEY, FOREIGN KEY, and NOT NULL constraints.
- [ ] Schema migration tests: empty DB → v1, v1 → v2 (when v2 exists).
- [ ] Transaction atomicity test: multi-step write fails partially → all changes rolled back.
- [ ] WAL mode verified: concurrent readers during a write operation.
- [ ] Foreign key cascade: deleting a patch removes its edges.
- [ ] Config get/set round-trip test.
- [ ] Working set stage/unstage/clear lifecycle test.
- [ ] Branch CRUD lifecycle test.
- [ ] `cargo clippy` passes with zero warnings on the `meta` module.
- [ ] `cargo test` passes all metadata tests.

---

*End of BP-METADATA-001*
