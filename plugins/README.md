# Plugin Marketplace

The Suture plugin marketplace is an optional centralized registry for discovering and distributing WASM-based merge plugins. Plugins extend Suture's conflict resolution with format-specific merge strategies.

## Registry Format

The registry is defined in `registry.toml`. Each plugin entry contains:

```toml
[[plugin]]
id = "my-plugin"            # Unique identifier (slug format)
name = "My Plugin"          # Human-readable name
version = "0.1.0"           # Semver version
author = "Author Name"
description = "What it does"
format = ["json", "yaml"]   # Supported file formats (extensions)
abi_version = 2             # WASM ABI version (currently v2)
license = "MIT"
repository = "https://..."
homepage = "https://..."
wasm_url = "https://..."    # URL to compiled .wasm binary
checksum_sha256 = ""         # SHA-256 of the wasm binary
wasm_size_bytes = 0         # Size in bytes (metadata)
min_suture_version = "0.1.0"
max_suture_version = ""     # Empty = no upper bound
tags = ["json", "merge"]
verified = true             # Registry-maintained verification flag
```

## Submitting a Plugin

1. Build your plugin as a WASM module targeting the Suture ABI v2 spec.
2. Host the `.wasm` binary at a stable, publicly accessible URL.
3. Open a pull request against this repository adding your `[[plugin]]` entry to `registry.toml`.
4. Include the SHA-256 checksum of your WASM binary in `checksum_sha256`.

## Installing a Plugin

```sh
suture plugin install <id>
```

This fetches the plugin from the registry, verifies its checksum, and installs it locally.

## ABI Versioning

All plugins must target the current WASM ABI version (v2). The ABI defines the interface between Suture's host runtime and guest WASM modules, including:

- Memory layout and export/import signatures
- Merge function protocol (base, ours, theirs inputs; merged output)
- Conflict reporting and resolution callbacks
- Error handling conventions

See the ABI v2 spec in `.specs/` for full details.
