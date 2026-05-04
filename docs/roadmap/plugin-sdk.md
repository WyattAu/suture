# Suture Plugin SDK

## Overview

Suture supports WASM-based plugins for extending merge behavior and adding
partner integrations. Plugins run in a sandboxed WebAssembly runtime with
memory limits and fuel-based timeouts.

## Plugin API

### ABI Version 1

Every plugin must export:

```rust
// Plugin identity
pub fn plugin_version() -> i32;       // Must return 1
pub fn plugin_name() -> *const u8;    // Null-terminated string
pub fn plugin_extensions() -> i32;    // Pointer to extension list
```

### Merge Extension

```rust
// Called during semantic merge
// Inputs: base_ptr, base_len, ours_ptr, ours_len, theirs_ptr, theirs_len
// Output: result_ptr (JSON: {merged: string, conflicts: [{path, ours, theirs}]})
pub fn merge(
    base_ptr: *const u8, base_len: i32,
    ours_ptr: *const u8, ours_len: i32,
    theirs_ptr: *const u8, theirs_len: i32
) -> i32;
```

### Host Imports

Plugins can call host functions:

```
suture.alloc(size: i32) -> i32  // Allocate WASM memory, returns pointer
suture.log(ptr: *const u8)       // Log message to host
suture.get_config(key_ptr: *const u8) -> i32  // Read plugin config
```

### Memory Limits
- Max input size: 16MB per file
- Max WASM memory: 64MB
- Fuel limit: 1,000,000 instructions

## Creating a Plugin

### 1. Write the Plugin (Rust)

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn plugin_version() -> i32 { 1 }

#[wasm_bindgen]
pub fn plugin_name() -> *const u8 {
    // Return name as null-terminated string
}

#[wasm_bindgen]
pub fn merge(base_ptr: *const u8, base_len: i32,
             ours_ptr: *const u8, ours_len: i32,
             theirs_ptr: *const u8, theirs_len: i32) -> i32 {
    // Read inputs from WASM memory
    // Perform merge
    // Write result to WASM memory
    // Return pointer to result
}
```

### 2. Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

### 3. Test

```bash
suture plugins test ./target/wasm32-unknown-unknown/release/my_plugin.wasm
```

### 4. Upload

```bash
suture plugins upload ./target/wasm32-unknown-unknown/release/my_plugin.wasm
# Or via API:
curl -X POST https://platform.suture.dev/api/plugins/upload \
  -H "Authorization: Bearer $TOKEN" \
  -F "plugin=@my_plugin.wasm"
```

## Plugin Marketplace

### Listing
Plugins can be listed and searched via the platform:
- `GET /api/plugins` — list all plugins
- `GET /api/plugins/{name}` — get plugin details
- `POST /api/plugins/merge` — merge files using a plugin

### Security
- All plugins run in a sandboxed WASM runtime
- Memory access is bounds-checked
- Execution time is fuel-limited
- Plugins cannot access the filesystem or network
