# Suture Hub

Suture Hub is a self-hosted collaboration server for Suture repositories. It provides a web UI, HTTP API, and gRPC access for push/pull, repository browsing, user management, and replication.

## Starting a Hub

```bash
suture-hub --db hub.db
# Web UI at http://localhost:50051
# API at http://localhost:50051/api/v2
```

Flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--addr` | `0.0.0.0:50051` | Bind address |
| `--db` | in-memory | SQLite database file path |
| `--replication-role` | `standalone` | `standalone`, `leader`, or `follower` |

Omit `--db` for in-memory storage (useful for testing). Use `--db hub.db` for persistent storage.

## Authentication

When a hub has no users or tokens, all operations are open. Once the first user is created, authentication is required.

### Create a User (Admin Only)

```bash
curl -X POST http://localhost:50051/auth/register \
  -H "Authorization: Bearer <admin-token>" \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "display_name": "Alice", "role": "member"}'
```

Roles: `admin` (full access), `member` (push/pull), `reader` (pull only).

### Generate an API Token

```bash
curl -X POST http://localhost:50051/auth/token \
  -H "Authorization: Bearer <admin-token>"
```

### CLI Login

```bash
suture remote add origin http://localhost:50051
suture remote login
```

### Ed25519 Key Authentication

Generate a keypair and register the public key with the hub. Pushes signed with the corresponding private key are verified automatically.

```bash
suture key generate
```

## Pushing and Pulling

### CLI

```bash
suture remote add origin http://localhost:50051
suture push
suture pull
suture clone http://localhost:50051/my-repo
```

### API

**Push:**

```bash
curl -X POST http://localhost:50051/push \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "repo_id": "my-repo",
    "patches": [...],
    "branches": [...],
    "blobs": [...]
  }'
```

**Pull:**

```bash
curl -X POST http://localhost:50051/pull \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "repo_id": "my-repo",
    "known_branches": [...]
  }'
```

### Protocol V2 (Delta Transfer)

Suture Hub supports delta-based transfer for efficient syncs when both sides share common blobs.

```bash
# Handshake to discover capabilities
curl -X POST http://localhost:50051/v2/handshake \
  -H "Content-Type: application/json" \
  -d '{"client_version": 2}'
```

Response includes `server_capabilities` with `supports_delta` and `supports_compression` flags.

## Web UI

Open `http://localhost:50051` in a browser. Features:

- **Repository list** -- browse all repositories
- **File tree** -- navigate files at any branch (`/repos/{id}/tree/{branch}`)
- **Branch browser** -- view branches and their targets
- **Patch history** -- paginated commit log with cursor-based pagination
- **Branch protection** -- prevent force-push on protected branches
- **Search** -- search repositories and patches by keyword

## Mirrors

Mirror a remote repository locally for redundancy or faster access.

### Setup a Mirror

```bash
curl -X POST http://localhost:50051/mirror/setup \
  -H "Content-Type: application/json" \
  -d '{
    "repo_name": "local-copy",
    "upstream_url": "http://upstream-hub:50051",
    "upstream_repo": "upstream-repo"
  }'
```

### Sync a Mirror

```bash
curl -X POST http://localhost:50051/mirror/sync \
  -H "Content-Type: application/json" \
  -d '{"mirror_id": 1}'
```

### Check Mirror Status

```bash
curl http://localhost:50051/mirror/status
```

### CLI Mirror

```bash
suture remote mirror http://upstream/repo upstream-name
```

## Replication

Suture Hub supports leader-follower replication for high availability.

**Leader** (`--replication-role leader`):
- Pushes replication log entries to followers every 30 seconds
- Accepts peer management requests

**Follower** (`--replication-role follower`):
- Accepts replication entries from the leader
- Read-only for replication sync endpoint

**Standalone** (default):
- No replication; works independently

### Manage Peers (Leader Only)

```bash
# Add a follower
curl -X POST http://localhost:50051/replication/peers \
  -H "Content-Type: application/json" \
  -d '{"peer_url": "http://follower:50051", "role": "follower"}'

# List peers
curl http://localhost:50051/replication/peers

# Remove a peer
curl -X DELETE http://localhost:50051/replication/peers/1

# Check replication status
curl http://localhost:50051/replication/status
```

## API Reference

### Repositories

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/repos` | List all repositories |
| POST | `/repos` | Create a repository |
| GET | `/repo/{id}` | Repository info (patch count, branches) |
| DELETE | `/repos/{id}` | Delete a repository |

### Branches

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/repos/{id}/branches` | List branches |
| POST | `/repos/{id}/branches` | Create a branch |
| DELETE | `/repos/{id}/branches/{name}` | Delete a branch |
| POST | `/repos/{id}/protect/{branch}` | Protect a branch |
| POST | `/repos/{id}/unprotect/{branch}` | Unprotect a branch |

### Content

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/repos/{id}/tree/{branch}` | File tree at branch |
| GET | `/repos/{id}/blobs/{hash}` | Get blob content (base64) |
| GET | `/repos/{id}/patches` | Paginated patch history |

### Sync

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/push` | Push patches (v1) |
| POST | `/pull` | Pull patches (v1) |
| POST | `/push/compressed` | Push with Zstd compression |
| POST | `/pull/compressed` | Pull with Zstd compression |
| POST | `/v2/push` | Push with delta transfer (v2) |
| POST | `/v2/pull` | Pull with delta transfer (v2) |

### Auth

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/auth/token` | Generate API token |
| POST | `/auth/verify` | Verify a token |
| POST | `/auth/login` | Login with username + token |
| POST | `/auth/register` | Create a user (admin only) |

### Users

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/users` | List users (admin only) |
| GET | `/users/{username}` | Get user info |
| PATCH | `/users/{username}/role` | Update user role (admin only) |
| DELETE | `/users/{username}` | Delete user (admin only) |

### Mirrors

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/mirror/setup` | Register a mirror |
| POST | `/mirror/sync` | Sync a mirror |
| GET | `/mirror/status` | Mirror status |
| DELETE | `/mirrors/{id}` | Delete a mirror |

### Replication

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/replication/peers` | Add a replication peer (leader) |
| GET | `/replication/peers` | List peers |
| DELETE | `/replication/peers/{id}` | Remove a peer |
| GET | `/replication/status` | Replication status |
| POST | `/replication/sync` | Accept replication entries (follower) |

### Other

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/search?q=` | Search repos and patches |
| GET | `/activity` | Activity log (paginated) |
| POST | `/handshake` | Protocol version handshake (v1) |
| POST | `/v2/handshake` | Protocol version handshake (v2) |

## gRPC Access

Suture Hub exposes a gRPC service alongside the HTTP API. See the `crates/suture-hub/grpc/` directory for service definitions.

## Rate Limits

Default limits (configurable):

| Operation | Limit |
|-----------|-------|
| Pushes | 100 per hour per IP |
| Pulls | 1000 per hour per IP |
| Token creation | 5 per minute per IP |

Rate-limited responses include a `Retry-After` header.
