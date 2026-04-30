# Suture 5.1.0: Semantic Merge for Every Format

After 5 weeks of development, 228 commits, and a growing open-source community, we're shipping Suture 5.1.0 — a complete semantic merge platform.

Suture parses structured files into their native representations, compares them semantically, and produces correct merges without spurious conflicts. What started as a Rust library for merging JSON has become a full stack: a git merge driver, an interactive demo, a hosted platform with billing and analytics, editor extensions, language bindings, and CI/CD integration — supporting 17 file formats out of the box.

## What's new in 5.1.0

This release turns Suture from a CLI tool into a platform. Here are the four biggest additions:

- **Git merge driver (5-second install).** One curl command and git automatically uses Suture for JSON, YAML, TOML, XML, and CSV files. No configuration editing, no build steps — it downloads the right binary for your platform and wires up `.gitattributes`. Includes templates for Node.js, Rust, Python, Kubernetes, Java, CI/CD, web, and Go projects.

- **Interactive demo.** A fully client-side 3-way merge demo at suture.dev/#/merge. No backend, no signup. Load a base, ours, and theirs — or edit them in place — and see the semantic merge happen live. Supports JSON, YAML, TOML, and CSV with conflict highlighting and shareable URLs.

- **Platform with billing.** The Suture hub is now deployable with Stripe integration: free, pro ($9/mo), and enterprise ($29/mo) tiers. Usage analytics with 30-day charts, team management with invite/remove/role, and a customer portal for self-service plan changes. Webhook events are HMAC-SHA256 signed with 5-minute timestamp freshness.

- **GitHub Action + CI/CD integration.** `suture/merge-action` runs semantic merge as part of any CI pipeline. Reads files at arbitrary git refs, calls the merge API, and falls back to standard git merge for non-structured files. Also includes portable bash scripts for GitLab CI, CircleCI, and Jenkins.

## The problem

Git's three-way merge operates on lines. It diffs `HEAD` against the common ancestor and `branch` against the common ancestor, then tries to apply both sets of changes. When two people edit different keys in the same JSON object, the line-based diff reports a conflict — even though the changes are structurally non-overlapping.

This affects every team that collaborates on structured files: Kubernetes manifests, CI configs, localization files, OpenAPI specs, database migrations, i18n resource bundles, and more. The result is wasted time manually resolving conflicts that a structural merge would handle automatically.

Suture solves this by understanding file formats. It parses files into trees, maps, and arrays — the actual data structures — and merges at that level. Non-overlapping changes merge cleanly. True conflicts (same key changed to different values) are surfaced with clear markers.

## How it works

Suture's merge pipeline has three stages. **Parse:** each file format driver converts raw bytes into an intermediate tree representation using the `SutureDriver` trait. **Compare:** the tree diff engine walks both branches against the common ancestor to identify additions, deletions, and modifications at the node level. **Merge:** the merge engine applies both change sets, resolving non-overlapping edits automatically and flagging true conflicts for manual resolution.

## Install

**As a git merge driver (recommended):**

```bash
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash
```

**Other methods:**

| Method | Command |
|--------|---------|
| Cargo | `cargo install suture-merge-driver` |
| Homebrew | `brew install WyattAu/suture-merge-driver/suture-merge-driver` |
| npm | `npm install -g suture-merge-driver` |
| PyPI | `pip install suture-merge-driver` |
| Binary | [GitHub Releases](https://github.com/WyattAu/suture/releases) |

## Supported formats

JSON, YAML, TOML, XML, CSV, SQL, HTML, Markdown, SVG, Properties/INI, DOCX, XLSX, PPTX, PDF, Image metadata, RSS/Atom feeds, iCalendar, and OpenTimelineIO timelines.

## What's next

Three areas we're focused on for the next release cycle:

- **WASM plugins.** The plugin system is built on wasmtime v28 with fuel-based timeouts and memory limits. We're working on a registry where anyone can publish format drivers without touching the core codebase.

- **Real-time collaboration.** The hub already has WebSocket infrastructure from the Raft consensus work. We're investigating operational transform on top of the semantic merge layer for concurrent editing.

- **Community drivers.** We want Suture to support every format that matters. If you work with a structured format that isn't on the list, the `SutureDriver` trait is four methods — `parse`, `diff`, `format_diff`, and `merge` — and we'll help you ship it.

## Try it

- **Interactive demo:** [suture.dev/#/merge](https://suture.dev/#/merge)
- **API docs:** [suture.dev/#/api](https://suture.dev/#/api)
- **Source code:** [github.com/WyattAu/suture](https://github.com/WyattAu/suture)
- **Install the git merge driver:** `curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash`
