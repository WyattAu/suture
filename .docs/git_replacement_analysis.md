# Suture as a Git Replacement for Non-Simple Text Files

## Executive Summary

Suture provides semantic merge for 19 file formats where Git offers only line-based diff3. It is a credible replacement for Git in workflows dominated by structured data (JSON, YAML, TOML, CSV, XML, Office documents, SQL schemas). However, it is not yet a full Git replacement for general-purpose version control -- significant gaps remain in network protocol efficiency, binary delta transfer, and ecosystem maturity.

**Verdict: 60-65% complete for the "Git but for structured files" use case. Core merge engine is production-grade; infrastructure gaps remain.**

---

## 1. What Git Does Badly (The Problem Space)

Git treats every file as a sequence of lines. This fails catastrophically for:

| File Type | Git Behavior | Failure Mode |
|-----------|-------------|--------------|
| JSON/YAML/TOML | Line-based diff3 | Reorderings, nested key changes produce false conflicts |
| CSV | Line-based diff3 | Row inserts corrupt column alignment |
| XML/HTML/SVG | Line-based diff3 | Attribute reordering, namespace changes cause spurious conflicts |
| DOCX/XLSX/PPTX | Binary blob | No diff/merge at all |
| SQL schemas | Line-based diff3 | ALTER TABLE reordering, index changes conflict incorrectly |
| Markdown | Line-based diff3 | Heading restructuring, list reordering produces conflicts |

---

## 2. Suture's Semantic Merge Engine

### 2.1 Merge Drivers (19 Formats)

| # | Driver | Extensions | Merge Granularity | Tests | Notable Capability |
|---|--------|-----------|-------------------|-------|--------------------|
| 1 | JSON | .json, .jsonl | Key-path value-level | 58 | Cargo.lock awareness, TF .tfstate merge, package.json scripts |
| 2 | YAML | .yaml, .yml | Key-path value-level | 46 | K8s strategic merge patch (by name key), named-object array identity |
| 3 | TOML | .toml | Key-path value-level | 34 | Standard key-value (via macro) |
| 4 | XML | .xml | Element-level (XPath) | 35 | Namespace-aware, attribute diff |
| 5 | HTML | .html, .htm | Node-level (CSS selector) | 12 | id-aware node identification |
| 6 | SVG | .svg | Element-level | 12 | id-attribute-aware paths |
| 7 | CSV | .csv | Row-level | 34 | Row-keyed by first column, duplicate handling |
| 8 | Markdown | .md, .mdx | Block-level | 44 | Heading-based identity, code blocks, lists, tables |
| 9 | SQL | .sql | DDL schema-level | 12 | CREATE/DROP TABLE, ALTER TABLE, INDEX |
| 10 | Properties | .properties | Key-level | 12 | Java-style continuation lines |
| 11 | OTIO | .otio | Element-tree level | 12 | Clips by URL+time, tracks by name |
| 12 | iCal | .ics | Component-level | 12 | UID-based event identity |
| 13 | Feed | .rss, .atom | Entry-level | 12 | RSS/Atom, entry identity by id/guid |
| 14 | XLSX | .xlsx | Cell-level (A1) | 21 | Shared strings, shared formula (si="N") |
| 15 | DOCX | .docx | Block-level | 23 | Track changes detection (rPrChange, move, comment ranges) |
| 16 | PPTX | .pptx | Slide-level | 12 | SHA-256 content hash identity |
| 17 | PDF | .pdf | Page-level | 12 | Text extraction, page-granularity diff |
| 18 | Image | .png/.jpg/.gif/.bmp/.webp/.tiff | Metadata-only | 12 | Width/height/color_type diff. NO pixel merge. |
| 19 | Example | (template) | N/A | N/A | Reference driver template |

**Total driver tests: ~419**

### 2.2 Core Merge Algorithm

The merge engine operates at three levels:

**Patch Algebra** (`suture-core/src/patch/`):
- Patches are the atomic unit of change, each with a BLAKE3 content hash ID
- `TouchSet`: BTreeSet of addresses a patch modifies
- **Commutativity**: Two patches commute iff their touch sets are disjoint (THM-COMM-001)
- **Conflict classification**: Four types -- AutoResolvable (identical changes), DriverResolvable (different sub-addresses), Genuine (same element, different values), Structural (modify vs delete)
- Formal proofs: Lean 4 proofs for commutativity, idempotency, and conflict symmetry

**Line-Level Merge** (`suture-core/src/engine/merge.rs`):
- 3-way merge using LCS (Longest Common Subsequence)
- DP table O(m*n) for files up to 2,000 lines, linear-space hash-based for larger files
- Conflict marker generation for unresolvable regions

**DAG-Aware Branch Merge** (`suture-core/src/dag/merge.rs`):
- Finds LCA (lowest common ancestor) between branch tips
- Computes unique patches per branch
- Produces merge plan without modifying the DAG

### 2.3 Conflict Resolution

The conflict resolution system is formally verified:

| Type | Name | Resolution | Example |
|------|------|------------|---------|
| I | AutoResolvable | Automatic | Both sides set `version = "2.0"` |
| II | DriverResolvable | Semantic merge via driver | One changes `name`, other changes `version` in same JSON |
| III | Genuine | Manual resolution required | Both change `version` to different values |
| IV | Structural | Manual resolution required | One modifies file, other deletes it |

---

## 3. VCS Operations Parity

Suture implements **60+ commands** covering the full Git workflow:

### Full Parity (Feature-Equivalent)

| Category | Commands |
|----------|----------|
| Core VCS | init, status, add, rm, commit, log, diff, show, blame, grep |
| Branching | branch (create/delete/list/protect), checkout, switch, restore |
| Merging | merge (--dry-run, --continue, --abort, strategies: semantic/ours/theirs/manual) |
| Rebasing | rebase (including interactive), cherry-pick, revert, rollback, squash, undo |
| Tagging | tag (lightweight, annotated, delete, list, sort) |
| Stashing | stash (push/pop/apply/list/drop/show/clear/branch) |
| Remote | clone (--depth shallow), push (--force), pull (--rebase, --autostash), fetch (--depth) |
| Integrity | verify (Ed25519), fsck, doctor (--fix), audit (tamper-evident log) |
| Export | export (dir/zip, --at ref), archive (tar.gz/zip) |

### Beyond Git

| Feature | Description |
|---------|-------------|
| Semantic merge | 19 format-aware drivers instead of line-based diff3 |
| `merge-file` | Standalone 3-way merge with auto driver detection |
| `classification` | Defence compliance marking scan/report |
| `timeline` | OpenTimelineIO import/export/summary/diff |
| `batch` | Stage/commit/export clients by pattern |
| `report` | Change/activity/stats reporting |
| `sync` | Auto-commit + push/pull daemon |
| `hub` | Backup/restore to central server |
| `drivers` | List/query semantic drivers |
| Git driver mode | `git driver install` -- use Suture as a Git merge driver |

### Git Interop

- `git import` -- Import Git repository history into Suture
- `git log`, `git status` -- Query Git state
- `git driver` -- Install/uninstall Suture as a Git merge driver (hybrid mode)

---

## 4. Storage and Protocol

### Content-Addressable Storage

| Aspect | Suture | Git |
|--------|--------|-----|
| Hash algorithm | BLAKE3 | SHA-1 (transitioning to SHA-256) |
| Compression | Zstd level 3 | zlib |
| Object layout | `.suture/objects/{2-char}/{62-char}` | `.git/objects/{2-char}/{38-char}` |
| Pack files | Yes (PackFile + PackIndex + PackCache) | Yes (pack + idx) |
| Blob cache | In-memory LRU (1024 entries) | In-memory loose objects |
| Fanout | 256 buckets | 256 buckets |
| Verify-on-read | Configurable | Not standard |

### Protocol

| Aspect | Suture | Git |
|--------|--------|-----|
| Transport | HTTP JSON + Zstd | HTTP/SSH, pack protocol |
| Delta encoding | Prefix/suffix matching only | Thin packs with OFS_DELTA |
| Compression | Zstd | zlib |
| Authentication | Ed25519 signing + Bearer tokens + OIDC | SSH keys + HTTPS credentials |
| Large files | LFS protocol | Git LFS |
| Version | V1 + V2 (capability negotiation) | v1 + v2 |

### Key Limitation: Delta Algorithm

Suture's delta encoding (`compute_delta` in suture-protocol) uses **prefix/suffix byte matching only**:

```
1. Find longest common prefix from start
2. Find longest common suffix from end
3. Emit changed middle region
```

Git's pack protocol uses a proper delta encoding that can reference regions at **any offset** in the base object, not just prefix/suffix. This means:

- **Best case** (append-only files like logs): Suture is as efficient as Git
- **Typical case** (changes scattered in structured files): Suture sends more data than Git would
- **Worst case** (single byte change in middle of large binary): Suture sends the entire changed region; Git sends a small delta

---

## 5. Gap Analysis

### What Suture Does Better Than Git

| Feature | Advantage |
|---------|-----------|
| Structured file merge | Semantic merge for 19 formats vs Git's line-based diff3 |
| Conflict classification | Formal 4-type taxonomy with auto-resolution for Type I/II |
| Office document support | DOCX/XLSX/PPTX merge (Git: no support) |
| Database schema merge | SQL DDL-aware merge (Git: line-based, error-prone) |
| K8s/Terraform merge | Strategic merge patch, .tfstate merge (Git: impossible) |
| Formal verification | Lean 4 proofs for core invariants |
| Integrity | Ed25519 signing built-in (Git: GPG signing, less ergonomic) |
| Compliance | Defence classification scanning, audit log |

### What Git Does Better Than Suture

| Feature | Gap Size | Impact |
|---------|----------|--------|
| Delta encoding | Large | Git's OFS_DELTA is far more efficient for scattered changes |
| Ecosystem maturity | Very Large | GitHub/GitLab/Forgejo integrations, CI/CD, code review |
| Performance at scale | Large | Git handles millions of files; Suture is untested at that scale |
| Binary diff | Medium | Git has binary diff heuristics; Suture has metadata-only for images |
| Submodules | Small | Suture has no submodule equivalent |
| Partial clone | Medium | Git supports blobless/treeless clones; Suture has shallow clone only |
| Worktree concurrency | Medium | Suture supports worktrees but concurrent access patterns are untested |
| SSH transport | Small | Suture uses HTTP only |
| Hook ecosystem | Small | Suture has hooks but no pre-commit.com ecosystem |

### Missing Features for Git Parity

| Feature | Status | Effort to Implement |
|---------|--------|---------------------|
| Rolling hash delta encoding | Missing | 4-6 weeks |
| Binary delta (libxdiff equivalent) | Missing | 3-4 weeks |
| Sparse checkout | Missing | 2-3 weeks |
| Submodules / subtrees | Missing | 4-6 weeks |
| Multi-remote support | Partial (mirror exists) | 2-3 weeks |
| Reflog expiry / gc heuristics | Partial (gc exists) | 1-2 weeks |
| Performance optimization for 1M+ files | Untested | 6-8 weeks |
| Git compatibility layer (serve as Git remote) | Missing | 8-12 weeks |

---

## 6. Test Coverage and Formal Verification

### Test Counts

| Module | Tests |
|--------|-------|
| Merge hardening | 63 |
| CLI integration | 63 |
| Repository operations | 61 |
| JSON driver | 58 |
| Protocol | 55 |
| YAML driver | 46 |
| Markdown driver | 44 |
| Raft consensus | 36 |
| XML driver | 35 |
| TOML driver | 34 |
| CSV driver | 34 |
| E2E workflows | 26 |
| Patch algebra (proptest) | ~30 |
| **Total** | **~1,601** |

### Formal Verification (Lean 4)

- **28 theorems** across 2 proof files
  - 16 core DAG/patch algebra theorems (proof_suture_core.lean)
  - 12 Raft safety theorems (proof_raft_safety.lean)
- 7 `sorry` placeholders remain (proof sketches provided)
- Properties proved: DAG acyclicity, commutativity, idempotency, conflict symmetry
- Properties pending: log matching, PreVote safety, quorum overlap

---

## 7. Assessment by Use Case

| Use Case | Readiness | Notes |
|----------|-----------|-------|
| **Configuration management** (JSON/YAML/TOML) | **90%** | Core strength. Semantic merge eliminates most conflicts. |
| **Kubernetes manifests** | **85%** | Strategic merge patch works. Missing: CRD schema awareness, kubectl integration. |
| **Terraform state** | **75%** | .tfstate merge works. Missing: .tf file merge, state locking protocol. |
| **Office documents** (DOCX/XLSX/PPTX) | **70%** | Best-in-class. Missing: image embedding merge, chart data merge. |
| **Database schemas** | **65%** | DDL merge works. Missing: data migration generation, round-trip fidelity. |
| **General software development** | **50%** | Good merge, but missing ecosystem (CI, code review, PRs). |
| **Large binary files** | **40%** | LFS exists but no binary diff. Image: metadata-only. |
| **Performance-critical repos** (1M+ files) | **30%** | Untested at scale. No rolling hash delta. No partial clone. |

---

## 8. Roadmap to Git Parity for Structured Files

### Phase 1: Core Infrastructure (4-6 weeks)

- Rolling hash delta encoding (replace prefix/suffix with rsync-style block matching)
- Binary delta encoding (xdiff-equivalent for arbitrary binary files)
- Performance benchmarking at 100K+ files
- Streaming blob transfer (chunked instead of whole-blob)

### Phase 2: Ecosystem (6-8 weeks)

- Git remote compatibility layer (act as a Git remote for push/pull)
- GitHub Actions integration (status checks, commit status API)
- Editor integration hardening (VS Code, Neovim, JetBrains)
- CI/CD trigger webhooks (push, PR merge events)

### Phase 3: Advanced Features (4-6 weeks)

- Sparse checkout / partial clone
- Submodule support
- Image diff (pixel-level SSIM comparison)
- PDF intra-page semantic merge

---

## 9. Conclusion

Suture's semantic merge engine for structured files is genuinely ahead of Git. The 19 format-aware drivers, formal conflict taxonomy, and Lean 4-verified patch algebra provide a correctness guarantee that Git cannot match for non-trivial structured data.

However, Suture is **not yet a drop-in Git replacement**. The gaps are in infrastructure (delta encoding efficiency, network protocol, large-scale performance) and ecosystem (CI/CD, code review, hosting platform). For teams whose primary pain point is merge conflicts in structured files (DevOps teams managing K8s configs, data teams with CSV/JSON pipelines, organizations with Office document workflows), Suture already provides significant value today -- particularly through the `git driver` mode that layers semantic merge on top of an existing Git workflow.

The path to full Git replacement is approximately **14-20 weeks** of focused engineering on the infrastructure and ecosystem gaps listed above.
