# Suture Merge Driver v5.0.0

Semantic merge for 20+ structured file formats. Drop-in Git merge driver that resolves conflicts in JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, OTIO, SQL, PDF, images, iCalendar, and RSS/Atom feeds at the structural level instead of the line level.

## What's New in v5.0.0

This is a major release with unified versioning, new formats, binary document conflict handling, and improved CI/CD integration.

**Unified versioning.** All 37 crates in the workspace are now at v5.0.0, providing coherent versioning across the entire Suture ecosystem. Previous releases had inconsistent crate versions (e.g., suture-cli at v1.0.0 while suture-merge was at v0.2).

**New supported formats.** iCalendar (`.ics`) with event-level merge and RSS/Atom (`.rss`, `.atom`) with feed and entry-aware merge. The total format count is now 20+.

**Binary document conflict UX.** DOCX, XLSX, and PPTX files no longer get corrupted by conflict markers. When a conflict occurs, the driver preserves the "ours" version and generates a `.suture_conflicts/report.md` file with details about what conflicted and how to resolve it.

**Improved CI/CD integration.** The GitHub Action now supports the `fail-on-conflict` and `file-patterns` inputs for finer control over merge behavior in automated workflows.

**Merge strategies.** New `SUTURE_MERGE_STRATEGY` environment variable supports `semantic` (default), `ours`, and `theirs` strategies for conflict resolution control.

**Performance.** Semantic merge of a 10-field JSON file completes in under 9 microseconds. A 100-field JSON file merges in under 130 microseconds. These are release-build benchmarks on Linux x86_64.

**1,419 tests.** Zero failures across 37 crates, including 389 hardening tests with adversarial inputs, 80 validation tests with real-world fixtures, and 71 binary document end-to-end tests covering the full VCS lifecycle for DOCX, XLSX, and PPTX.

## Installation

### Prebuilt binaries (recommended)

Download from GitHub Releases for your platform:

| Platform | Architecture | File |
|----------|-------------|------|
| Linux | x86_64 | `suture-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | aarch64 | `suture-aarch64-unknown-linux-gnu.tar.gz` |
| macOS | x86_64 | `suture-x86_64-apple-darwin.tar.gz` |
| macOS | aarch64 | `suture-aarch64-apple-darwin.tar.gz` |
| Windows | x86_64 | `suture-x86_64-pc-windows-msvc.zip` |

Verify the download with the SHA256 checksum file included in the release assets.

### npm

```bash
npm install -g suture-merge-driver
```

Requires Node.js >= 18. Downloads the appropriate platform binary automatically.

### pip

```bash
pip install suture-merge-driver
```

Requires Python >= 3.8. Downloads the appropriate platform binary automatically.

### cargo

```bash
cargo install suture-cli
```

Installs the full Suture CLI, which includes the merge driver and all semantic formats. Requires a Rust toolchain.

### Homebrew

```bash
brew install wyattau/tap/suture
```

Available for macOS and Linux (x86_64 and aarch64).

### AUR (Arch Linux)

```bash
paru -S suture-git
```

### From source

```bash
git clone https://github.com/WyattAu/suture.git
cd suture
cargo build --release --bin suture
# Binary at target/release/suture
```

## Quick Configuration

After installation, configure Git to use Suture as a merge driver:

```bash
git config merge.suture.name "Suture semantic merge"
git config merge.suture.driver "suture-merge-driver %O %A %B %P"
git config merge.suture.recursive binary
echo "*.json merge=suture" >> .gitattributes
echo "*.yaml merge=suture" >> .gitattributes
echo "*.yml merge=suture" >> .gitattributes
echo "*.toml merge=suture" >> .gitattributes
```

Add more file patterns as needed. Commit `.gitattributes` to share the configuration with your team.

If you installed the full CLI (`suture-cli`), you can run `suture git driver install` to configure all 20+ formats at once.

## Migration from v0.2

The npm and pip packages now use the same package name (`suture-merge-driver`) and command name (`suture-merge-driver`). If you were using a previous version:

1. Uninstall the old package: `npm uninstall -g suture-merge-driver` or `pip uninstall suture-merge-driver`.
2. Install the new version using the commands above.
3. Update your git config if you were using a different driver command name. The new command accepts the same arguments (`%O %A %B %P`).
4. If you were using the `suture merge-file` CLI command directly, the interface is unchanged. All flags (`--driver`, `-o`, `--label-ours`, `--label-theirs`) work the same way.
5. If you had `merge.suture.recursive` set, verify it is still set to `binary` for DOCX/XLSX/PPTX support.

The `.gitattributes` format has not changed. Existing `.gitattributes` files with `merge=suture` patterns will work without modification.

## Supported Formats

| Format    | Extensions                                          | Merge Granularity               |
|-----------|-----------------------------------------------------|---------------------------------|
| JSON      | `.json`, `.jsonl`                                   | Field-level (RFC 6901 paths)    |
| YAML      | `.yaml`, `.yml`                                     | Key-level                       |
| TOML      | `.toml`                                             | Table and key-aware             |
| CSV       | `.csv`, `.tsv`                                      | Row-level with header detection |
| XML       | `.xml`, `.xsl`, `.svg`                              | Element/attribute-aware         |
| Markdown  | `.md`, `.markdown`                                  | Section-aware                   |
| HTML      | `.html`                                             | DOM-aware                       |
| DOCX      | `.docx`, `.docm`                                    | Paragraph-level                 |
| XLSX      | `.xlsx`, `.xlsm`                                    | Cell-level                      |
| PPTX      | `.pptx`, `.pptm`                                    | Slide-level                     |
| OTIO      | `.otio`                                             | Clip-level (video timelines)    |
| SQL       | `.sql`                                              | DDL schema diff                 |
| PDF       | `.pdf`                                              | Page-level text diff            |
| Image     | `.png`, `.jpg`, `.gif`, `.bmp`, `.webp`, `.tiff`, `.ico`, `.avif` | Metadata diff  |
| iCalendar | `.ics`                                              | Event-level merge               |
| RSS/Atom  | `.rss`, `.atom`                                     | Feed and entry-aware            |

Files without a matching driver fall back to Git's standard line-based merge.

## Known Limitations

- **PPTX and XLSX single-line XML.** Some Office applications serialize XML parts as single-line documents. Semantic merge may not detect changes as precisely in these cases, since the XML parser works at the element level and the driver may fall back to line-based merge.
- **DOCX positional diff.** The DOCX driver merges at the paragraph level. It does not track intra-paragraph changes (e.g., a single word change within a paragraph). If both sides edit the same paragraph, the paragraph-level merge reports a conflict even if the changes are to different sentences.
- **Large binary documents.** Documents over 50 MB may take several seconds to merge due to ZIP extraction and XML parsing. This is a known performance constraint and will be addressed in a future release.
- **Concurrent merges.** The merge driver is invoked once per file per merge. It does not handle multiple merges of the same file in parallel. This is a Git limitation, not a driver limitation.
- **Format detection.** Format is detected by file extension only. Files with non-standard extensions or no extension are not recognized and fall back to line-based merge. You can work around this by adding explicit patterns in `.gitattributes` (e.g., `Makefile merge=suture` for TOML-formatted Makefiles, though this is not recommended).
- **JSON comments.** JSON5 and JSONC (JSON with comments) are not supported. Only standard JSON (RFC 8259) is parsed semantically.

## Documentation

- [Quick Start Guide](../docs/quickstart.md) -- 60-second getting started
- [Merge Driver Guide](../docs/merge-driver-guide.md) -- comprehensive driver configuration, troubleshooting, CI/CD integration, and enterprise deployment
- [CLI Reference](../docs/cli-reference.md) -- full command documentation
- [Semantic Merge Explained](../docs/semantic-merge.md) -- how the merge algorithm works
- [Performance Baseline](../docs/performance.md) -- benchmark results

## Contributors

Thank you to everyone who contributed to this release:

- **@WyattAu** -- project lead, core engine, CLI, semantic drivers, OOXML infrastructure
- **Suture contributors** -- bug reports, feature requests, and testing

See the full contributor list at https://github.com/WyattAu/suture/graphs/contributors.

## License

Apache License 2.0. See [LICENSE](https://github.com/WyattAu/suture/blob/main/LICENSE).
