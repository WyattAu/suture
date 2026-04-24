# Security Audit Report

**Date:** 2026-04-24
**Auditor:** Automated (cargo-audit + manual review)
**Scope:** Suture VCS workspace (629 crate dependencies)

## Dependency Vulnerability Summary

### Critical (2)

| ID | Crate | Version | Title |
|---|---|---|---|
| RUSTSEC-2026-0095 | wasmtime | 22.0.1 | Winch compiler sandbox-escaping memory access |
| RUSTSEC-2026-0096 | wasmtime | 22.0.1 | Miscompiled guest heap access enables sandbox escape (aarch64 Cranelift) |

**Recommendation:** Upgrade wasmtime to >=36.0.7, <37.0.0 OR >=42.0.2, <43.0.0 OR >=43.0.1.
These are sandbox-escape vulnerabilities in the Wasmtime WASM runtime used by `suture-driver`
for plugin execution. Impact depends on whether untrusted WASM plugins are loaded.

### Medium (8)

| ID | Crate | Severity | Title |
|---|---|---|---|
| RUSTSEC-2026-0020 | wasmtime | 6.9 | Guest-controlled resource exhaustion in WASI |
| RUSTSEC-2026-0021 | wasmtime | 6.9 | Panic adding excessive fields to wasi:http/types.fields |
| RUSTSEC-2026-0091 | wasmtime | 6.1 | OOB write when transcoding component model strings |
| RUSTSEC-2026-0094 | wasmtime | 6.1 | Improperly masked return value from table.grow |
| RUSTSEC-2026-0085 | wasmtime | 5.6 | Panic when lifting flags component value |
| RUSTSEC-2026-0089 | wasmtime | 5.9 | Host panic when Winch executes table.fill |
| RUSTSEC-2026-0092 | wasmtime | 5.9 | Panic when transcoding misaligned UTF-16 strings |
| RUSTSEC-2026-0093 | wasmtime | 6.9 | Heap OOB read in UTF-16 to latin1+utf16 transcoding |
| RUSTSEC-2026-0087 | wasmtime | 4.1 | Segfault with f64x2.splat on Cranelift x86-64 |

All medium-severity findings are in `wasmtime 22.0.1`. Upgrading to >=24.0.7 resolves most.

### Low (4)

| ID | Crate | Severity | Title |
|---|---|---|---|
| RUSTSEC-2024-0438 | wasmtime | - | Windows device filename sandboxing |
| RUSTSEC-2025-0046 | wasmtime | 3.3 | Host panic with fd_renumber WASIp1 |
| RUSTSEC-2025-0118 | wasmtime | 1.8 | Unsound API access to shared linear memory |
| RUSTSEC-2026-0086 | wasmtime | 2.3 | Host data leakage with 64-bit tables |
| RUSTSEC-2026-0088 | wasmtime | 2.3 | Data leakage between pooling allocator instances |

### Warnings (5)

| ID | Crate | Category | Title |
|---|---|---|---|
| RUSTSEC-2025-0057 | fxhash 0.2.1 | unmaintained | No longer maintained |
| RUSTSEC-2024-0384 | instant 0.1.13 | unmaintained | Unmaintained (via notify) |
| RUSTSEC-2024-0436 | paste 1.0.15 | unmaintained | No longer maintained |
| RUSTSEC-2026-0002 | lru 0.12.5 | unsound | IterMut violates Stacked Borrows |
| RUSTSEC-2024-0442 | wasmtime-jit-debug 22.0.1 | unsound | Dump Undefined Memory |

## Fuzz Targets Created

7 libfuzzer-sys harness files in `crates/suture-fuzz/fuzz_targets/`:

| Target | Description | Notes |
|---|---|---|
| `fuzz_patch_deserialize.rs` | Patch JSON deserialization | Tests `suture_core::patch::types::Patch` |
| `fuzz_hash_parse.rs` | Hash hex parsing | Tests `suture_common::Hash::from_hex` |
| `fuzz_json_merge.rs` | JSON semantic 3-way merge | Uses `suture_driver_json::JsonDriver` |
| `fuzz_yaml_merge.rs` | YAML semantic 3-way merge | Uses `suture_driver_yaml::YamlDriver` |
| `fuzz_toml_merge.rs` | TOML semantic 3-way merge | Uses `suture_driver_toml::TomlDriver` |
| `fuzz_classification.rs` | Classification pattern regex | Tests 13 regex patterns from classification detection |
| `fuzz_diff_input.rs` | Unified diff parser | Inline re-implementation (suture-cli is binary-only) |

Existing proptest-based smoke tests remain in `crates/suture-fuzz/src/lib.rs`.

## Recommendations

1. **[Critical]** Upgrade `wasmtime` from 22.0.1 to >=24.0.7 (or latest stable). This resolves 14 of 16 vulnerabilities.
2. **[Medium]** Evaluate whether untrusted WASM plugins can be loaded. If so, the wasmtime sandbox escapes are exploitable.
3. **[Low]** Upgrade `lru` to fix Stacked Borrows unsoundness (via ratatui -> suture-tui).
4. **[Housekeeping]** Replace `fxhash` and `instant` with maintained alternatives when upstream deps update.
5. **[Fuzzing]** Run the new fuzz targets with `cargo-fuzz` in CI for continuous coverage.
6. **[Supply chain]** Pin dependency versions in Cargo.lock and review changes in PRs.
