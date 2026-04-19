# WASM Feasibility Report: Suture Semantic Merge Engine

## 1. Dependency Analysis

### suture-driver (trait crate)

| Dependency | WASM Status | Notes |
|---|---|---|
| `suture-core` | **BLOCKER** | Transitive — see below |
| `suture-common` | Friendly | `blake3`, `serde`, `thiserror` — all pure Rust |
| `serde` / `serde_json` | Friendly | Pure Rust |
| `thiserror` | Friendly | Pure Rust |
| `wasmtime` (optional) | **BLOCKER** | WASM runtime — cannot be compiled *to* WASM |

### suture-core (version control engine)

| Dependency | WASM Status | Notes |
|---|---|---|
| `blake3` | Friendly | Explicit WASM SIMD support |
| `zstd` | Friendly | Pure Rust implementation |
| `rusqlite` | **BLOCKER** | SQLite C bindings / filesystem |
| `ed25519-dalek` | Friendly | Has `wasm32` support |
| `rand` | Friendly | Has `wasm32` support |
| `dirs` | **BLOCKER** | Queries OS filesystem paths |
| `rayon` | **BLOCKER** | Requires threads (`std::thread`) |
| `tempfile` | **BLOCKER** | Filesystem temp directories |

### Driver crates (suture-driver-{json,yaml,toml,csv,xml,markdown})

| Crate | External Deps | WASM Status |
|---|---|---|
| `suture-driver-json` | `serde_json` | Friendly |
| `suture-driver-yaml` | `serde_yaml` (0.9 / `unsafe-libyaml`) | Friendly (pure Rust YAML) |
| `suture-driver-toml` | `toml` (0.8) | Friendly |
| `suture-driver-csv` | `csv` (1.3) | Friendly |
| `suture-driver-xml` | `roxmltree` (0.20) | Friendly |
| `suture-driver-markdown` | *(none)* | Friendly |

**All six driver crates contain zero filesystem, network, or threading calls.** Their merge logic operates entirely on `&str` inputs and produces `String` / `Option<String>` outputs.

## 2. Minimal WASM Library

### What can compile to WASM today

The semantic merge logic in every driver crate (`diff`, `merge`, `format_diff`) is pure computation on strings. None of the driver implementations use `suture-core` APIs at all — they only import three types from `suture-driver`:

```rust
use suture_driver::{DriverError, SemanticChange, SutureDriver};
```

The `SutureDriver` trait, `SemanticChange` enum, and `DriverError` type are defined in `suture-driver/src/lib.rs` and `suture-driver/src/error.rs`. They have **no dependency on `suture-core` internals** — the dependency on `suture-core` in `suture-driver/Cargo.toml` is used only by the `plugin` module (WASM plugin loading via wasmtime) and `registry` module.

### What cannot compile to WASM

- `suture-core` (SQLite, threads, filesystem)
- `suture-driver/plugin.rs` (wasmtime, filesystem plugin discovery)
- `suture-driver/registry.rs` (filesystem plugin discovery)

## 3. Recommendation

### Architecture: Extract a `suture-merge` crate

Create a new crate `suture-merge` that re-exports only the WASM-friendly subset:

```
suture-merge/
  Cargo.toml          # depends only on serde, thiserror
  src/
    lib.rs            # re-exports trait + types + all 6 drivers
    traits.rs         # SutureDriver trait + SemanticChange + DriverError
    json.rs           # JsonDriver (copied or symlinked from suture-driver-json)
    yaml.rs           # YamlDriver
    toml.rs           # TomlDriver
    csv.rs            # CsvDriver
    xml.rs            # XmlDriver
    markdown.rs       # MarkdownDriver
```

**Alternative (less duplication):** Extract the `SutureDriver` trait, `SemanticChange`, and `DriverError` into a zero-dependency `suture-driver-traits` crate. Then:

- `suture-driver` depends on `suture-driver-traits` + `suture-core` (for plugin/registry)
- `suture-driver-json` etc. depend on `suture-driver-traits` instead of `suture-driver`
- `suture-merge` depends on `suture-driver-traits` + all `suture-driver-*` crates

This eliminates code duplication and keeps the existing crate structure intact.

### WASM API surface

```rust
#[wasm_bindgen]
pub fn merge_json(base: &str, ours: &str, theirs: &str) -> JsValue { ... }

#[wasm_bindgen]
pub fn merge_yaml(base: &str, ours: &str, theirs: &str) -> JsValue { ... }

#[wasm_bindgen]
pub fn merge_toml(base: &str, ours: &str, theirs: &str) -> JsValue { ... }

#[wasm_bindgen]
pub fn merge_csv(base: &str, ours: &str, theirs: &str) -> JsValue { ... }

#[wasm_bindgen]
pub fn merge_xml(base: &str, ours: &str, theirs: &str) -> JsValue { ... }

#[wasm_bindgen]
pub fn merge_markdown(base: &str, ours: &str, theirs: &str) -> JsValue { ... }
```

Return type: `{ merged: string | null, is_clean: boolean }` (JSON-encoded `Option<String>`).

### What would need to change

1. **Extract trait types** into `suture-driver-traits` (or inline into `suture-merge`)
2. **Update driver crates** to depend on `suture-driver-traits` instead of `suture-driver` (or keep as-is and have `suture-merge` re-declare the types)
3. **Add `wasm-bindgen` wrapper** in `suture-merge`
4. **Add `[target.'cfg(target_arch = "wasm32")'.dependencies]`** if any conditional deps are needed
5. **No changes needed to driver merge logic** — it is already WASM-compatible

## 4. Estimated Effort

| Task | Effort |
|---|---|
| Create `suture-driver-traits` crate, move trait + types | 1-2 hours |
| Update 6 driver crates to use `suture-driver-traits` | 1 hour |
| Create `suture-merge` crate with `wasm-bindgen` API | 2-3 hours |
| Add `wasm-pack` build pipeline + npm package generation | 1-2 hours |
| Test WASM build + JS/TS integration tests | 2-3 hours |
| **Total** | **~1 day** |

### Key question answer

> Could we create a `suture-merge-wasm` crate that provides just the semantic merge function (parse + three-way merge + serialize) as a WASM module, with no filesystem or network dependencies?

**Yes.** The merge logic in all six drivers is already pure string-in/string-out computation with zero platform dependencies. The only obstacle is the transitive `suture-driver -> suture-core` dependency chain, which is trivially severed by extracting the `SutureDriver` trait into its own crate.
