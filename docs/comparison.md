# Suture vs Other Version Control Systems

## At a Glance

| Feature | Suture | Git | Pijul | Darcs | Mercurial |
|---------|--------|-----|-------|-------|-----------|
| Merge model | Patch DAG + semantic drivers | Three-way line merge | Patch theory | Patch theory | Three-way merge |
| Semantic merge | JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX | No | No | No | No |
| Office documents | DOCX/XLSX/PPTX | Binary blob | Binary blob | Binary blob | Binary blob |
| Conflict detection | Logical address overlap | Line overlap | Line overlap | Line overlap | Line overlap |
| Binary size | BLAKE3 + Zstd | SHA1 + zlib | BLAKE3 + zstd | SHA256 + zlib | SHA1 + zlib |
| Language | Rust | C | Rust | Haskell | Rust/Python |
| Maturity | v1.0 RC | Production (2005) | Alpha | Mature (2005) | Mature (2005) |
| License | Apache 2.0 | GPL v2 | GPL v2+ | GPL v2 | GPL v2+ |
| GitHub integration | Git merge driver | Native | No | No | Via Hg-Git |
| Distributed | Yes (hub replication) | Yes | Yes | Yes | Yes |
| LSP support | Yes (blame, diagnostics) | No | No | No | No |

## Detailed Comparison

### Suture vs Git

**Where Suture wins:**
- Semantic merge for 10+ structured file formats
- No false conflicts on JSON/YAML/TOML configs
- Office document merging (DOCX, XLSX, PPTX)
- Logical address-based conflict detection (two patches to different JSON keys never conflict)

**Where Git wins:**
- 20 years of maturity and tooling
- Every editor, CI system, and platform integrates with Git
- Enormous ecosystem (GitHub, GitLab, Bitbucket)
- Performance on large binary repositories (with Git LFS)

**Where they're equal:**
- Distributed architecture
- Branching model
- Speed on source code files (both use BLAKE3/SHA1 hashing)

**Can you use both?** Yes. Suture provides a [Git merge driver](git_merge_driver.md) that gives you semantic merging inside existing Git workflows.

### Suture vs Pijul

Both use patch-based models, but Suture adds semantic awareness:

- **Pijul** treats files as lines of text. Two patches to different lines commute.
- **Suture** treats JSON as key-value pairs. Two patches to different keys commute — even if they're on the same line.

### Suture vs Darcs

Darcs pioneered patch theory but hasn't gained traction:
- Darcs is written in Haskell (smaller contributor base)
- Suture is written in Rust (growing ecosystem, memory safety)
- Suture adds semantic drivers that Darcs lacks

## When to Use Suture

### Use Suture when:
- Your team frequently hits merge conflicts on config files
- You collaborate on Office documents via version control
- You want semantic conflict detection for structured data
- You're willing to try a new tool for a specific pain point

### Stick with Git when:
- You're working on source code (C, Rust, Python, etc.) — Git is excellent for this
- You need GitHub/GitLab integration
- Your team isn't experiencing merge conflict pain
- You need the full Git ecosystem (submodules, worktrees, bisect, etc.)

### Use both when:
- You want semantic merging for specific file types but need Git for everything else
- Use Suture's Git merge driver for JSON/YAML/TOML files, Git for everything else
