# Suture WASM Plugin ABI v2

## Overview

ABI v2 extends the host-function ABI (introduced in v1) with `suture_diff` and
`suture_format_diff` exports, enabling plugins to provide semantic diff
capabilities in addition to merge.

## Version Negotiation

Plugins export `suture_abi_version() -> i32`. The host accepts versions
`1..=2`. Plugins declaring ABI v1 are fully supported but cannot use
`suture_diff` / `suture_format_diff` (the host returns a graceful error
if the caller requests them).

| Export           | Return value | Meaning                       |
|------------------|-------------:|-------------------------------|
| `suture_abi_version` | `2`       | Current ABI version           |
| `suture_abi_version` | `1`       | Legacy — merge only, no diff  |

## Host Imports (from `"env"`)

### Merge I/O (ABI v1+)

| Import                     | Signature                           | Description                                    |
|----------------------------|-------------------------------------|------------------------------------------------|
| `get_input_len`            | `() -> i32`                         | Length of JSON merge input buffer               |
| `get_input_byte`           | `(offset: i32) -> i32`              | Read byte at `offset` from merge input (-1=OOB)|
| `set_output_byte`          | `(offset: i32, byte: i32)`          | Write byte to merge output buffer              |
| `set_output_len`           | `(len: i32)`                        | Resize merge output buffer (max 16 MiB)        |

### Diff I/O (ABI v2)

| Import                     | Signature                           | Description                                      |
|----------------------------|-------------------------------------|--------------------------------------------------|
| `get_diff_input_len`       | `() -> i32`                         | Length of first diff input buffer (base content) |
| `get_diff_input_byte`      | `(offset: i32) -> i32`              | Read byte from first diff input (-1=OOB)         |
| `get_diff_input2_len`      | `() -> i32`                         | Length of second diff input buffer (new content)  |
| `get_diff_input2_byte`     | `(offset: i32) -> i32`              | Read byte from second diff input (-1=OOB)        |
| `set_diff_output_byte`     | `(offset: i32, byte: i32)`          | Write byte to diff output buffer                 |
| `set_diff_output_len`      | `(len: i32)`                        | Resize diff output buffer (max 16 MiB)            |

### Logging

| Import                     | Signature                           | Description                                      |
|----------------------------|-------------------------------------|--------------------------------------------------|
| `host_log`                 | `(level: i32, msg_ptr: i32, msg_len: i32)` | Log message (0=trace, 1=debug, 2=info, 3=warn, 4+=error) |

## Plugin Exports

### Required (all ABI versions)

| Export                     | Signature             | Description                                        |
|----------------------------|-----------------------|----------------------------------------------------|
| `suture_merge`             | `() -> i32`           | Perform 3-way merge (see below)                    |
| `suture_abi_version`       | `() -> i32`           | Return ABI version number                          |
| `suture_plugin_name`       | `() -> *const u8`     | Pointer to plugin name (not null-terminated)       |
| `suture_plugin_name_len`   | `() -> i32`           | Length of plugin name                              |
| `suture_plugin_version`    | `() -> *const u8`     | Pointer to plugin version string                   |
| `suture_plugin_version_len`| `() -> i32`           | Length of version string                           |
| `suture_extensions`        | `() -> *const u8`     | Comma-separated extensions (e.g. "json,yaml")     |
| `suture_extensions_len`    | `() -> i32`           | Length of extensions string                        |
| `suture_error_msg`         | `() -> *const u8`     | Error message (set before returning -1)             |
| `suture_error_msg_len`     | `() -> i32`           | Length of error message                            |

### ABI v2 Optional

| Export                     | Signature             | Description                                        |
|----------------------------|-----------------------|----------------------------------------------------|
| `suture_diff`              | `() -> i32`           | Perform semantic diff (see below)                  |
| `suture_format_diff`      | `() -> i32`           | Produce formatted diff string (see below)          |

## Data Flow Patterns

### Merge (`suture_merge`)

1. Host serialises `{"base":"...","ours":"...","theirs":"..."}` as JSON
2. Host writes JSON to merge input buffer, sets `input_len`
3. Host clears output buffer
4. Host calls `suture_merge()`
5. Plugin reads input via `get_input_len` / `get_input_byte`
6. Plugin writes result via `set_output_len` / `set_output_byte`
7. Plugin returns: `0` (success), `1` (conflict), or `-1` (error)
8. Host reads output buffer on success

### Diff (`suture_diff`)

1. Host writes base content to diff input 1 buffer, sets `diff_input1_len`
2. Host writes new content to diff input 2 buffer, sets `diff_input2_len`
3. Host clears diff output buffer
4. Host calls `suture_diff()`
5. Plugin reads inputs via `get_diff_input_*` host functions
6. Plugin writes diff output via `set_diff_output_len` / `set_diff_output_byte`
7. Plugin returns: `0` (success, diff in output), `1` (no changes), or `-1` (error)
8. Host reads diff output buffer on success

### Format Diff (`suture_format_diff`)

1. Same input setup as `suture_diff`
2. Host calls `suture_format_diff()`
3. Plugin returns: `0` (success) or `-1` (error)
4. Host reads formatted string from diff output buffer

## Return Codes

All callable exports (`suture_merge`, `suture_diff`, `suture_format_diff`)
use the same return code convention:

| Code | Meaning                                |
|------|----------------------------------------|
| `0`  | Success — output in buffer             |
| `1`  | Conflict / no changes (context-dependent) |
| `-1` | Error — check `suture_error_msg`       |
| Other| Reserved / error                      |

## Resource Limits

| Resource            | Limit     |
|---------------------|-----------|
| Plugin memory       | 16 MiB    |
| Output buffer size  | 16 MiB    |
| Default fuel budget | 1,000,000 fuel units |

## Backward Compatibility

- ABI v1 plugins (returning `1` from `suture_abi_version`) continue to work
- ABI v1 plugins without `suture_diff`/`suture_format_diff` exports: the host
  returns `Interface("suture_diff not exported (plugin uses ABI v1)")` gracefully
- New host imports (`get_diff_input_*`, `set_diff_output_*`) are always linked
  by the host, even for v1 plugins — they simply won't be called

## Migration from ABI v1 to v2

1. Update `suture_abi_version` to return `2`
2. Add `suture_diff` and/or `suture_format_diff` exports
3. Use `get_diff_input_len`/`get_diff_input_byte` and `get_diff_input2_len`/`get_diff_input2_byte`
   to read the two diff inputs
4. Write diff results to the diff output buffer via `set_diff_output_len`/`set_diff_output_byte`
