# Suture WASM Plugin ABI

## Overview

WASM plugins are `.wasm` modules that implement a set of required exports to
integrate with the Suture driver system. Plugins are loaded at runtime and
provide format-specific semantic diff and merge capabilities.

## ABI Version

The current ABI version is **1**. All plugins must export `plugin_version`
returning this value. Future versions will be backwards-compatible where
possible.

## Required Exports

| Export | Type | Description |
|--------|------|-------------|
| `plugin_name` | `function() -> i32` | Returns pointer to null-terminated plugin name string |
| `plugin_extensions` | `function() -> i32` | Returns pointer to comma-separated list of file extensions |
| `plugin_version` | `function() -> i32` | Returns ABI version (currently 1) |

## Required Functions for Merge

| Export | Type | Description |
|--------|------|-------------|
| `merge` | `function(base_ptr: i32, base_len: i32, ours_ptr: i32, ours_len: i32, theirs_ptr: i32, theirs_len: i32) -> (i32, i32)` | 3-way merge returning (result_ptr, result_len) |

## Required Functions for Diff

| Export | Type | Description |
|--------|------|-------------|
| `diff` | `function(base_ptr: i32, base_len: i32, new_ptr: i32, new_len: i32) -> (i32, i32)` | Semantic diff returning (changes_ptr, changes_len) |

## Memory Layout

- Strings are null-terminated UTF-8 encoded in WASM linear memory
- Return values are `(pointer, length)` pairs pointing into WASM linear memory
- The host provides memory allocation via `alloc(size: i32) -> i32` import

## Error Handling

- Return `(0, 0)` to indicate "no result / fall back to line-based" handling
- Return `(ptr, len)` with JSON-encoded error for failures

## Host Imports

| Import | Signature | Description |
|--------|-----------|-------------|
| `suture.alloc` | `(size: i32) -> i32` | Allocate bytes in WASM memory, returns pointer |
| `suture.memory` | `(ptr: i32, len: i32)` | Read memory region (provided by WASM memory export) |
