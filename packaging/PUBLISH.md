# Publishing Suture to crates.io

## Prerequisites

1. **Rust API token**: Create one at https://crates.io/settings/tokens with `publish-update` permission
2. **Login**: `cargo login <token>`
3. **Verify ownership**: You must be the owner of all crate names on crates.io

## Publishing Order

Crates must be published in dependency order. The publish script handles this automatically.

### Core library chain (no external dependencies beyond workspace)
```
suture-common (v1.1.0)
└── suture-core (v1.1.0)       ← depends on suture-common
    ├── suture-protocol (v1.1.0) ← depends on suture-common, suture-core
    ├── suture-driver (v1.1.0)  ← depends on suture-common, suture-core
    │   ├── suture-ooxml (v1.1.0) ← depends on suture-common
    │   ├── suture-driver-json (v1.1.0)
    │   ├── suture-driver-yaml (v1.1.0)
    │   ├── suture-driver-toml (v1.1.0)
    │   ├── suture-driver-csv (v1.1.0)
    │   ├── suture-driver-xml (v1.1.0)
    │   ├── suture-driver-markdown (v1.1.0)
    │   ├── suture-driver-docx (v1.1.0)
    │   ├── suture-driver-xlsx (v1.1.0)
    │   ├── suture-driver-pptx (v1.1.0)
    │   ├── suture-driver-sql (v1.1.0)
    │   ├── suture-driver-pdf (v1.1.0)
    │   ├── suture-driver-image (v1.1.0)
    │   ├── suture-driver-otio (v1.1.0)
    │   └── suture-driver-example (v1.1.0)
    ├── suture-tui (v1.1.0)     ← depends on suture-core, suture-driver
    ├── suture-lsp (v1.1.0)     ← depends on suture-core
    ├── suture-raft (v0.1.0)    ← depends on async-trait, serde, tokio, tracing, thiserror
    ├── suture-s3 (v0.1.0)      ← depends on suture-common, reqwest, etc.
    └── suture-daemon (v1.1.0)  ← depends on suture-core, suture-protocol, suture-hub
```

### Application crates
```
suture-hub (v1.1.0)             ← depends on suture-core, suture-protocol, suture-driver
suture-cli (v2.5.0)             ← depends on suture-core, suture-protocol, suture-driver, suture-hub
```

### Not published (require special toolchains)
- `suture-py` — Requires Python dev headers (PyO3)
- `suture-node` — Requires Node.js/npm (napi-rs)
- `suture-e2e` — Integration tests only
- `suture-fuzz` — Fuzz testing only
- `suture-bench` — Benchmarks only

## Quick Publish (all crates in order)

```bash
# Login first
cargo login

# Publish core chain
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

# Publish application crates
cargo publish -p suture-hub
cargo publish -p suture-daemon
cargo publish -p suture-tui
cargo publish -p suture-lsp
cargo publish -p suture-raft
cargo publish -p suture-s3
cargo publish -p suture-cli
```

## Pre-publish Checklist

Before publishing, verify each crate has:
- [ ] `description` field in Cargo.toml
- [ ] `license` field (Apache-2.0)
- [ ] `repository` field (GitHub URL)
- [ ] `readme` field pointing to README.md
- [ ] `keywords` array
- [ ] `categories` array
- [ ] No `path` dependencies pointing to local crates (only version deps)
- [ ] `cargo publish --dry-run -p <crate>` succeeds

## Post-publish

After publishing `suture-cli`, users can install with:
```bash
cargo install suture-cli
```

Then update:
- README.md install instructions
- Homebrew formula URL
- AUR PKGBUILD source URL
- GitHub release notes
