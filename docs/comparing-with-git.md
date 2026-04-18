# Suture vs. Git: When to Use What

Git is one of the most successful tools in software history. Suture is not trying to replace it — Suture solves a different problem.

## At a Glance

| | Git | Suture |
|---|---|---|
| **Best for** | Source code, text files | Structured files: JSON, YAML, DOCX, XLSX, PPTX, CSV, OTIO |
| **Merge model** | Three-way line merge | Patch DAG with semantic drivers |
| **Conflict detection** | Line overlap | Logical address overlap |
| **Binary files** | Opaque blobs (conflicts) | Semantic merge when format is supported |
| **Ecosystem** | 20 years of tooling, CI/CD, platforms | Early, growing |
| **Maturity** | Production since 2005 | v1.0 RC |

## Where Git Wins

**Source code.** Git was built for text files and it's excellent at it. Line-based diff and merge is the right model for `.py`, `.rs`, `.js`, `.c`, and every other programming language.

**Ecosystem.** GitHub, GitLab, Bitbucket, CI/CD pipelines, code review tools, editor integrations — the ecosystem is unmatched. Every platform, every IDE, every build system speaks Git.

**Maturity.** Twenty years of edge cases, performance optimizations, and battle-tested reliability. Git handles million-file repos, massive monorepos, and every workflow pattern imaginable.

**Scripting and automation.** `git log`, `git diff`, `git hook` — the tooling for automation is deep and well-documented.

## Where Suture Wins

**Structured data.** JSON, YAML, TOML, CSV, XML — Suture understands the structure and merges at the key/row/element level. Two people editing different keys in the same JSON file never conflict.

**Office documents.** DOCX (paragraph-level), XLSX (cell-level), PPTX (slide-level). Git treats these as opaque ZIP archives. Suture opens them up and merges the actual content.

**Video timelines.** OTIO timeline merge at the clip/track level. No other VCS does this.

**Conflict precision.** Suture only flags conflicts when logical addresses overlap — not when changes happen to be on the same line. This eliminates the vast majority of false conflicts in structured files.

**Filesystem mount.** FUSE and WebDAV mounts let any application save directly into a Suture repo. Git requires explicit `git add` and `git commit`.

## Where They Complement

**Git merge driver.** Use Suture as a merge driver inside your existing Git repos:

```bash
git config merge.suture.name "suture"
git config merge.suture.driver "suture merge-file --driver %s %O %A %B -o %A"
```

This gives you semantic merging for JSON, YAML, TOML, etc. while keeping Git for everything else.

**Git remote helper** (planned). In the future, Suture could act as a Git remote, allowing `git push` and `git pull` to synchronize with a Suture Hub.

**Hybrid workflows.** Use Git for source code and CI/CD. Use Suture for configs, documents, and media. The two can coexist in the same organization.

## When to Use What

| Scenario | Tool |
|----------|------|
| Writing Rust/Python/JS code | Git |
| Managing Kubernetes YAML across environments | Suture (or Suture as Git merge driver) |
| Collaborating on a Word document | Suture |
| Tracking spreadsheet changes across a team | Suture |
| Version-controlling a video editing timeline | Suture |
| CI/CD pipeline with GitHub Actions | Git |
| Data science experiment with YAML configs + CSV results | Suture |
| Open source project on GitHub | Git (with Suture merge driver for configs) |
| Internal team sharing Excel budget workbooks | Suture |

## Bottom Line

Git is the right tool for source code. Suture is the right tool for everything else — the structured files, documents, and media assets that make up the non-code part of modern projects. They're complementary, not competing.

## See Also

- [Why Suture?](why-suture.md) — the problem with binary version control
- [Comparison with other VCS](comparison.md) — Pijul, Darcs, Mercurial
- [Git merge driver setup](git_merge_driver.md) — use Suture inside Git
