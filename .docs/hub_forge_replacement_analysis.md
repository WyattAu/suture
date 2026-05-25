# Suture Hub as a Forgejo/GitLab/Gitea Replacement

## Executive Summary

Suture Hub is a well-engineered **patch synchronization server** with solid foundations in storage, replication, authentication, webhooks, and monitoring. It provides approximately **30-35% of the feature surface** needed to replace Forgejo for Suture repository hosting. The core patch-sync protocol is production-grade. The major missing capabilities -- issue tracking, pull requests/code review, CI/CD, wiki, organizations/teams, fork networks, and a richer API -- represent the bulk of what makes a modern code forge.

**Verdict: 30-35% complete. Core infrastructure is solid; collaborative features are absent.**

---

## 1. What Exists (Production-Grade)

### 1.1 Repository Management

| Operation | Endpoint | Status |
|-----------|----------|--------|
| Create repo | `POST /repos` | Implemented |
| Delete repo | `DELETE /repos/{repo_id}` | Implemented (cascade: patches, branches, blobs, protection) |
| List repos | `GET /repos` | Implemented |
| Repo info | `GET /repo/{repo_id}` | Implemented (patch_count, branches) |
| Create branch | `POST /repos/{repo_id}/branches` | Implemented |
| Delete branch | `DELETE /repos/{repo_id}/branches/{branch}` | Implemented |
| Branch protection | `POST /repos/{repo_id}/protect/{branch}` | Implemented |
| Tree view | `GET /repos/{repo_id}/tree/{branch}` | Implemented (reconstructed from patches) |
| Blob view | `GET /repos/{repo_id}/blobs/{hash}` | Implemented |
| Patch list | `GET /repos/{repo_id}/patches` | Implemented |
| Batch push | `POST /repos/{repo_id}/patches/batch` | Implemented |

### 1.2 Patch Sync Protocol

| Version | Features | Status |
|---------|----------|--------|
| V1 | Push, pull, handshake, compressed transport | Implemented |
| V2 | Delta encoding, capability negotiation, known_blob_hashes | Implemented |
| Compressed | Zstd compression wrapper for V1 | Implemented |
| gRPC | 14 RPC methods defined in proto | Proto only, no server wiring |

**Sync reliability:**
- Non-fast-forward push rejection (unless force=true)
- Per-repo push locking (prevents concurrent push conflicts)
- Ed25519 signature verification on push
- Incremental sync (patches since last_pushed)
- V2 skip-if-present optimization (known_blob_hashes)

### 1.3 Authentication & Authorization

| Mechanism | Status | Details |
|-----------|--------|---------|
| Bearer token auth | Implemented | SHA256-hashed tokens with scopes (read/write/admin) |
| Ed25519 key signing | Implemented | Push operations carry 64-byte Ed25519 signatures |
| RBAC | Implemented | 3 roles: Admin (rank 3), Member (rank 2), Reader (rank 1) |
| Token scopes | Implemented | read, write, admin -- checked via `TokenScope::contains_scope()` |
| OIDC/SSO client | Implemented | Google, Okta, Azure AD, Auth0, Keycloak support |
| Rate limiting | Implemented | Per-IP: pushes 100/hr, pulls 1000/hr, token creates 5/min |
| No-auth mode | Implemented | Development/air-gapped deployment |
| Audit logging | Implemented | All mutating requests logged with actor, action, resource, status |

### 1.4 Storage

| Backend | Status | Details |
|---------|--------|---------|
| SQLite (default) | Implemented | WAL mode, NORMAL sync, 64KB cache, 256MB mmap |
| S3 (optional) | Implemented | Via `s3-backend` feature, multipart upload for >8MB blobs |
| BlobBackend trait | Implemented | `store_blob`, `get_blob`, `has_blob`, `delete_blob`, `list_blobs` |

### 1.5 Webhooks

| Feature | Status | Details |
|---------|--------|---------|
| Create/List/Delete | Implemented | Per-repo webhooks |
| Event types | Implemented | push, branch.create, branch.delete |
| HMAC signing | Implemented | `X-Suture-Signature: sha256=...` |
| Retry with backoff | Implemented | 5 retries, 1s base, 5min max, +/-25% jitter |
| Delivery tracking | Implemented | Succeeded/Pending/Failed/Aborted states |
| Custom headers | Implemented | Configurable per-webhook |

### 1.6 Monitoring

| Feature | Endpoint | Status |
|---------|----------|--------|
| Health check | `/healthz` | Implemented (DB + blob + Raft check) |
| Readiness probe | `/readyz` | Implemented (DB writable check) |
| Liveness probe | `/livez` | Implemented (always 200) |
| Prometheus metrics | `/metrics` | Implemented (9 metrics with labels) |
| Request duration | Histogram | Implemented (10ms, 100ms, 1s, +Inf buckets) |
| Path normalization | Middleware | Implemented (dynamic segments replaced with `:id`) |

**Prometheus metrics:**
- `suture_build_info` (version, compiler)
- `suture_process_start_time_seconds`
- `suture_repos_total`
- `suture_patches_total`
- `suture_blobs_total`
- `suture_blobs_size_bytes`
- `suture_active_users_total`
- `suture_requests_total` (method, path, status labels)
- `suture_request_duration_seconds` (histogram)

### 1.7 Replication & Mirroring

| Feature | Status | Details |
|---------|--------|---------|
| Mirror setup | Implemented | Pull from upstream Suture Hub |
| Mirror sync | Implemented | On-demand sync |
| Mirror status | Implemented | Query sync state |
| Peer management | Implemented | Add/remove/list peers |
| Replication sync | Implemented | Push to peers |
| Raft consensus | Implemented (feature flag) | Leader election, log replication, snapshots |

### 1.8 Backup/Restore

| Feature | Status | Details |
|---------|--------|---------|
| Full backup | Implemented | SQLite online backup + manifest.json |
| Restore | Implemented | Validate manifest, restore repos/patches/branches/blobs |
| Integrity check | Implemented | Verifies restored repo count matches manifest |

### 1.9 Web UI

The Hub serves a **single-page application** (~2,466 lines of HTML/CSS/JS):

| View | Lines | Features |
|------|-------|----------|
| Dashboard | ~150 | Stat cards (repos, patches, users), recent activity feed |
| Repositories | ~100 | Table with name, patch count, branch count. Create/delete. |
| Repo detail | ~150 | Branches (with protection toggles), patches table, create/delete branch |
| File tree | ~80 | Directory browsing with breadcrumbs, file links |
| Blob viewer | ~50 | File content display (binary detection), hash and size |
| Patches list | ~80 | Paginated table with ID, type badge, path, author, message, timestamp |
| Search | ~40 | Search repos and patches by query string |
| Users | ~80 | Table with username, role dropdown, delete button, create form |
| Mirrors | ~60 | List mirrors, sync buttons, add mirror form |
| Replication | ~60 | Peer management, replication status |
| Settings | ~40 | Hub configuration display |
| Login | ~30 | Token-based login form |
| CSS | 1,262 | GitHub-dark theme, responsive (768px breakpoint), toast notifications, skeleton loading |

**UI architecture:** Vanilla JavaScript, hash-based routing (`#/repos`, `#/repo/{id}`), no build step. Served via Axum `include_str!` for HTML, filesystem for CSS/JS.

---

## 2. What Is Missing (Feature Gap Analysis)

### 2.1 Issue Tracking -- MISSING

Forgejo provides: issues, labels, milestones, assignments, due dates, comments, reactions, attachments, time tracking, dependencies, pinning, project boards.

Suture Hub: **Nothing.** No issues table, no issue types, no handlers. The database schema has 16 tables; none relate to issues.

**Estimated effort to implement: 8-12 weeks.**

Components needed:
- Database schema (issues, labels, milestones, comments, reactions, issue_dependencies, issue_assignees)
- ~30 REST endpoints (CRUD for issues, comments, labels, milestones, assignments)
- Search and filtering (full-text search, label filters, assignee filters)
- Email notifications on issue events
- Web UI for issue management
- API pagination

### 2.2 Pull Requests / Code Review -- MISSING

Forgejo provides: PR lifecycle (create, review, approve/request-changes, comment, merge), merge strategies (merge, squash, rebase), CI status checks, branch protection rules requiring reviews, line comments, review suggestions.

Suture Hub: **Nothing.** No PR types, handlers, routes, or database tables.

**Estimated effort to implement: 12-16 weeks.**

Components needed:
- Database schema (pull_requests, reviews, review_comments, review_approvals)
- ~40 REST endpoints (create PR, list PRs, review, approve, merge, comment, diff)
- Diff generation endpoint (patch-based diff between branches)
- Merge trigger (apply merge plan on approval)
- Branch protection integration (require N approvals)
- Line-level commenting on diffs
- Web UI for PR review (diff view, comment threads)
- Email notifications on PR events
- CI status integration

### 2.3 CI/CD -- MISSING

Forgejo provides: Forgejo Actions (GitHub Actions compatible), runners, workflows, artifacts, status checks, secrets management.

Suture Hub: **Nothing.** No pipeline, runner, build status, or CI/CD concepts.

**Estimated effort to implement: 16-24 weeks.**

Components needed:
- Workflow definition format (YAML-based, compatible with GitHub Actions?)
- Runner registration and management
- Job scheduling and execution
- Log streaming
- Artifact storage
- Secret management (encrypted at rest)
- Status checks API (commit/patch status)
- Web UI for pipeline visualization
- Webhook integration (trigger on push, PR)

### 2.4 Wiki -- MISSING

Forgejo provides: Git-backed wiki per repository, Markdown rendering, page history, search.

Suture Hub: **Nothing.**

**Estimated effort to implement: 3-4 weeks.**

Components needed:
- Database schema (wiki_pages, wiki_history)
- ~10 REST endpoints (CRUD for pages, history, search)
- Markdown rendering
- Web UI for wiki editing
- Version history display

### 2.5 Organizations and Teams -- MISSING

Forgejo provides: organizations, teams, team membership, per-repo team access, org-level settings.

Suture Hub: **Flat user list.** No organizations, teams, groups, or namespaces.

**Estimated effort to implement: 4-6 weeks.**

Components needed:
- Database schema (organizations, teams, team_members, team_repo_access)
- ~20 REST endpoints (org CRUD, team CRUD, membership, access control)
- Namespace support (org/repo instead of just repo_id)
- Per-repo access control (team can read/write/admin specific repos)
- Web UI for org/team management

### 2.6 Fork Networks -- MISSING

Forgejo provides: fork repos, fork networks, sync from parent, PR from fork.

Suture Hub: **No fork concept.** Repos are independent.

**Estimated effort to implement: 3-4 weeks.**

Components needed:
- Database schema (fork_relationships)
- Fork endpoint (create repo from existing repo's patch history)
- Fork network display
- Cross-repo merge (PR from fork to parent)

### 2.7 Release Management -- MISSING

Forgejo provides: releases, tags, release assets, release notes.

Suture Hub: Tags exist in the CLI. No Hub release management.

**Estimated effort to implement: 2-3 weeks.**

### 2.8 Package Registry -- MISSING

Forgejo provides: Container registry, npm, PyPI, Maven, Cargo, etc.

Suture Hub: **Nothing.**

**Estimated effort to implement: 12-16 weeks** (significant scope).

### 2.9 Email Notifications -- MISSING

Forgejo provides: Email on issue, PR, review, push events. Configurable per-user preferences.

Suture Hub: Webhooks only. No email notifications.

**Estimated effort to implement: 3-4 weeks.**

### 2.10 Code Search -- MISSING

Forgejo provides: Full-text code search (bleve/elasticsearch), regex search, syntax-aware search.

Suture Hub: Basic LIKE queries on repo IDs and patch author/message. No blob content search.

**Estimated effort to implement: 4-6 weeks.**

### 2.11 Advanced Auth Features -- MISSING

| Feature | Forgejo | Suture Hub | Effort |
|---------|---------|------------|--------|
| SSH key authentication | Yes | No | 2-3 weeks |
| Two-factor authentication (2FA) | Yes | No | 2-3 weeks |
| LDAP authentication | Yes | No | 2-3 weeks |
| OAuth2 server (for integrations) | Yes | No (only client) | 3-4 weeks |
| Password-based login | Yes | No (token-only) | 1-2 weeks |
| Email verification | Yes | No | 1-2 weeks |

### 2.12 Repository Features -- MISSING

| Feature | Forgejo | Suture Hub |
|---------|---------|------------|
| Visibility (public/private) | Yes | No |
| Description/website/topics | Yes | No |
| Default branch setting | Yes | No (hardcoded "main") |
| Archiving | Yes | No |
| Transfer | Yes | No |
| Repo size display | Yes | No |
| README rendering | Yes | No |
| Language detection | Yes | No |
| License detection | Yes | No |

---

## 3. API Completeness

| Metric | Suture Hub | Forgejo |
|--------|-----------|---------|
| REST endpoints | ~40 unique routes | ~500+ endpoints |
| API versioning | V1 (`/api/v1/`) | V1 |
| gRPC methods | 14 defined (not wired) | N/A |
| GraphQL | No | No |
| Pagination | Basic (limit/offset on some) | Comprehensive |
| Error codes | `HubErrorCode` enum | Structured error responses |
| Rate limiting | Per-IP (fixed limits) | Per-user (configurable) |
| CORS | Not configured | Configurable |

### Endpoint Count by Category

| Category | Suture Hub | Forgejo (approximate) |
|----------|-----------|----------------------|
| Repository operations | 14 | 80+ |
| User management | 5 | 30+ |
| Authentication | 5 | 15+ |
| Issues | 0 | 60+ |
| Pull requests | 0 | 50+ |
| Releases | 0 | 20+ |
| Organizations | 0 | 25+ |
| Teams | 0 | 15+ |
| CI/CD | 0 | 30+ |
| Wiki | 0 | 10+ |
| Search | 1 | 10+ |
| Settings/Admin | 5 | 40+ |
| Webhooks | 3 | 10+ |
| LFS | 3 | 5+ |
| Sync/Protocol | 10 | N/A (Git protocol) |
| Replication | 5 | N/A |
| SSO | 5 | 5+ |
| **Total** | **~40** | **~500+** |

---

## 4. Raft Cluster Readiness

The Raft implementation (2,100+ lines) is impressive but **not production-ready for multi-node**:

| Component | Status | Details |
|-----------|--------|---------|
| Leader election | Implemented | PreVote extension, jittered timeouts |
| Log replication | Implemented | AppendEntries with consistency checks |
| Snapshots | Implemented | InstallSnapshot for log compaction |
| Membership changes | Implemented | Joint consensus |
| Read index | Implemented | Linearizable reads via quorum heartbeat |
| TCP transport | Implemented | 4-byte length prefix + JSON |
| Hub integration | Implemented | 8 HubCommand types (CreateRepo, StoreBlob, etc.) |
| **Peer message forwarding** | **Stub** | "In a real multi-node deployment, these messages would be sent to peers via HTTP. For now, we log state transitions." |
| **Snapshot recovery** | **Missing** | Snapshots created but no state machine recovery from snapshot |
| **Log persistence in Hub** | **Missing** | `persist` feature exists in raft crate but not wired into Hub |
| **Linearizable reads in Hub** | **Missing** | raft node supports read_index but Hub doesn't use it |

**Assessment:** Single-node Raft works. Multi-node requires:
1. Wire TCP transport to peer message forwarding (2-3 weeks)
2. Add log persistence (1-2 weeks)
3. Add snapshot recovery (1-2 weeks)
4. Add linearizable reads to Hub handlers (1 week)
5. Integration testing with 3+ nodes (2-3 weeks)

**Total: 7-11 weeks for production multi-node.**

---

## 5. Security Posture

| Aspect | Status | Details |
|--------|--------|---------|
| TLS transport | Implemented | rustls via reqwest |
| Token hashing | Implemented | SHA256 |
| Push signing | Implemented | Ed25519 |
| Rate limiting | Implemented | Per-IP, configurable |
| Audit logging | Implemented | All mutating requests |
| Webhook HMAC | Implemented | HMAC-SHA256 |
| SQL injection | Mitigated | Parameterized queries (rusqlite) |
| CORS | Not configured | Potential issue for browser integrations |
| CSRF | Not configured | Not needed for API-only (no cookies) |
| Secret storage | Basic | Tokens in SQLite, no encryption at rest |
| Input validation | Partial | No explicit validation layer |
| Dependency auditing | Weekly cargo audit | CI-integrated |

---

## 6. What Suture Hub Does Better Than Forgejo

| Feature | Advantage |
|---------|-----------|
| **Semantic merge server-side** | Hub can merge structured files using 19 format-aware drivers. Forgejo relies on Git (line-based). |
| **Raft consensus** | Built-in consensus for multi-node replication. Forgejo has no equivalent. |
| **Patch-based protocol** | More expressive than Git's object model for structured data. |
| **Ed25519 push signing** | Built-in, more ergonomic than GPG signing. |
| **S3 blob storage** | Native S3 backend. Forgejo requires external configuration. |
| **Prometheus metrics** | Comprehensive built-in metrics. Forgejo requires configuration. |
| **Audit logging** | Built-in audit trail. Forgejo requires configuration. |
| **Protocol V2** | Capability negotiation, delta encoding, skip-if-present. More modern than Git pack protocol. |

---

## 7. Recommended Strategy

### Option A: Full Forgejo Parity (Not Recommended)

**Effort: 12-18 months.** This would require implementing issues, PRs, CI/CD, wiki, orgs, teams, forks, releases, packages, email, code search, and expanding the API from 40 to 500+ endpoints. This duplicates existing open-source work and does not leverage Suture's unique advantages.

### Option B: Integration Layer (Recommended)

**Effort: 3-4 months.** Position Suture Hub as a **specialized companion to Forgejo/GitLab**, not a replacement:

1. **Git bridge**: Implement a Git remote compatibility layer so Forgejo can push/pull from Suture Hub (4-6 weeks)
2. **Webhook integration**: Forgejo webhooks trigger Suture semantic merges (2 weeks, mostly exists)
3. **Merge-as-a-service**: Expose the 19 semantic merge drivers as an API that Forgejo calls for merge conflicts (2-3 weeks)
4. **Federated identity**: OIDC integration with Forgejo as identity provider (1-2 weeks)
5. **SSO passthrough**: Allow Forgejo users to authenticate to Suture Hub via Forgejo OIDC (1-2 weeks)

### Option C: Minimal Forgejo Replacement for Suture-Only Teams (Pragmatic)

**Effort: 6-8 months.** For teams that use only Suture (no Git), provide the minimum viable forge:

| Priority | Feature | Effort |
|----------|---------|--------|
| P0 | Pull requests (simplified: branch diff + approval + merge) | 8-10 weeks |
| P0 | Code search (blob content search) | 4-6 weeks |
| P1 | Issue tracking (basic: issues, labels, comments) | 6-8 weeks |
| P1 | Email notifications | 3-4 weeks |
| P1 | Repository visibility (public/private) | 1-2 weeks |
| P2 | Organizations and teams | 4-6 weeks |
| P2 | Fork networks | 3-4 weeks |
| P2 | Wiki (basic Markdown) | 3-4 weeks |
| P3 | CI/CD (webhook-based, not built-in runners) | 2-3 weeks |
| P3 | Release management | 2-3 weeks |

---

## 8. Assessment by Team Size

| Team Size | Suture Hub Today | What's Missing | Recommendation |
|-----------|-----------------|----------------|----------------|
| **1-3 people** | Usable today | PRs, issues | Use Hub + external issue tracker |
| **4-10 people** | Marginal | PRs, issues, email, code search | Implement PRs + issues first |
| **10-50 people** | Insufficient | PRs, issues, orgs, teams, CI, wiki | Implement Option C (6-8 months) |
| **50+ people** | Insufficient | Full forge feature set | Use Forgejo/GitLab + Suture as merge service |

---

## 9. Conclusion

Suture Hub's core infrastructure -- patch sync, authentication, storage, replication, monitoring, and webhooks -- is solid and production-grade. The codebase is well-organized (~7,400 lines in server.rs alone) with comprehensive test coverage (96 tests).

The gap is not in engineering quality but in feature breadth. A code forge requires issue tracking, code review, CI/CD, and collaboration features that are absent. These are not trivial additions -- they represent the majority of Forgejo's codebase.

**The recommended path is Option B (Integration Layer):** position Suture Hub as a specialized semantic merge server that complements existing forges rather than competing with them. This leverages Suture's unique advantage (19 format-aware merge drivers) without reimplementing the commodity features (issues, PRs, CI/CD) that Forgejo already provides.

For teams committed to a Suture-only workflow, Option C (Minimal Forge Replacement) provides a realistic 6-8 month path to a usable team platform centered on semantic version control.
