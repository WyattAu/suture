# Publishing Suture to crates.io

## Prerequisites

1. **Rust API token**: Create one at https://crates.io/settings/tokens with `publish-update` permission
2. **Login**: `cargo login <token>`
3. **Verify ownership**: You must be the owner of all crate names on crates.io

## Publishing Order

Crates must be published in dependency order. The publish script handles this automatically.

### Core library chain (no external dependencies beyond workspace)
```
suture-common (v5.1.0)
└── suture-core (v5.1.0)       ← depends on suture-common
    ├── suture-protocol (v5.1.0) ← depends on suture-common, suture-core
    ├── suture-driver (v5.1.0)  ← depends on suture-common, suture-core
    │   ├── suture-ooxml (v5.1.0) ← depends on suture-driver
    │   ├── suture-driver-json (v5.1.0)
    │   ├── suture-driver-yaml (v5.1.0)
    │   ├── suture-driver-toml (v5.1.0)
    │   ├── suture-driver-csv (v5.1.0)
    │   ├── suture-driver-xml (v5.1.0)
    │   ├── suture-driver-markdown (v5.1.0)
    │   ├── suture-driver-docx (v5.1.0) ← depends on suture-driver, suture-ooxml
    │   ├── suture-driver-xlsx (v5.1.0) ← depends on suture-driver, suture-ooxml
    │   ├── suture-driver-pptx (v5.1.0) ← depends on suture-driver, suture-ooxml
    │   ├── suture-driver-sql (v5.1.0)
    │   ├── suture-driver-pdf (v5.1.0)
    │   ├── suture-driver-image (v5.1.0)
    │   ├── suture-driver-otio (v5.1.0) ← depends on suture-driver, suture-core, suture-common
    │   └── suture-driver-example (v5.1.0)
    ├── suture-tui (v5.1.0)     ← depends on suture-core, suture-common, suture-driver, drivers
    ├── suture-lsp (v5.1.0)     ← depends on suture-core, suture-common
    ├── suture-raft (v0.1.0)    ← depends on async-trait, serde, tokio, tracing, thiserror
    ├── suture-s3 (v0.1.0)      ← depends on suture-common, reqwest, etc.
    ├── suture-vfs (v0.1.0)     ← depends on suture-core, suture-protocol, fuse3, axum
    └── suture-daemon (v5.1.0)  ← depends on suture-core, suture-common, suture-protocol
```

### Application crates
```
suture-hub (v5.1.0)             ← depends on suture-core, suture-common, suture-protocol
suture-cli (v5.3.1)             ← depends on suture-core, suture-common, suture-protocol, suture-driver, drivers, suture-tui
```

### Not published (require special toolchains or are tooling-only)
- `suture-py` — Requires Python dev headers (PyO3), excluded from workspace
- `suture-node` — Requires Node.js/npm (napi-rs), `publish = false`
- `suture-e2e` — Integration tests only, `publish = false`
- `suture-fuzz` — Fuzz testing only, `publish = false`
- `suture-bench` — Benchmarks only, `publish = false`

## Quick Publish (automated)

```bash
# Dry-run first (recommended)
./scripts/publish.sh

# Real publish
./scripts/publish.sh --real
```

## Quick Publish (manual, all crates in order)

```bash
# Login first
cargo login

# Core chain
cargo publish -p suture-common
cargo publish -p suture-core
cargo publish -p suture-protocol
cargo publish -p suture-driver
cargo publish -p suture-ooxml
cargo publish -p suture-driver-json
cargo publish -p suture-driver-yaml
cargo publish -p suture-driver-toml
cargo publish -p suture-driver-csv
cargo publish -p suture-driver-xml
cargo publish -p suture-driver-markdown
cargo publish -p suture-driver-docx
cargo publish -p suture-driver-xlsx
cargo publish -p suture-driver-pptx
cargo publish -p suture-driver-sql
cargo publish -p suture-driver-pdf
cargo publish -p suture-driver-image
cargo publish -p suture-driver-otio
cargo publish -p suture-driver-example

# Application crates
cargo publish -p suture-hub
cargo publish -p suture-daemon
cargo publish -p suture-tui
cargo publish -p suture-lsp
cargo publish -p suture-vfs
cargo publish -p suture-raft
cargo publish -p suture-s3
cargo publish -p suture-cli
```

## Pre-publish Checklist

Before publishing, verify each crate has:
- [x] `description` field in Cargo.toml
- [x] `license` field (Apache-2.0)
- [x] `repository` field (GitHub URL)
- [x] `readme` field pointing to project README
- [x] `categories` array
- [x] `keywords` array
- [ ] No `path` dependencies pointing to local crates (only version deps)
- [ ] `cargo publish --dry-run -p <crate>` succeeds

> **Note**: Local `path` dependencies are normal during development. Before
> publishing, verify that the `version` field on each `path` dependency matches
> what is (or will be) published on crates.io. `cargo publish --dry-run`
> validates this automatically.

## Install Verification

After publishing `suture-cli`, verify the install works:

```bash
./scripts/verify-install.sh
```

## Post-publish

After publishing `suture-cli`, users can install with:
```bash
cargo install suture-cli
```

Then update:
- README.md install instructions
- Homebrew formula (`Formula/suture.rb` and `packaging/homebrew/suture.rb`)
- AUR PKGBUILD (`packaging/aur/PKGBUILD`)
- GitHub release notes
