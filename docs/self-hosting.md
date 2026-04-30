# Self-Hosting Suture Hub

## Prerequisites

- **Rust 1.85+** (for building from source) or **Docker** (for containerized deployment)
- **1 GB RAM** minimum, **10 GB disk**
- **SQLite** (bundled, zero config) or **PostgreSQL** (optional, for larger deployments)

## Quick Start (Docker)

```bash
docker compose up -d
# Hub available at http://localhost:8080
```

## Quick Start (Binary)

```bash
# Install
cargo install suture-hub

# Run
suture-hub --addr 0.0.0.0:8080 --data-dir ./hub-data

# Or with custom config
suture-hub --addr 0.0.0.0:8080 --data-dir /var/lib/suture --lfs-dir /var/lib/suture/lfs
```

## Configuration

### CLI Flags

| Flag | Default | Description |
|---|---|---|
| `--addr` | `0.0.0.0:8080` | Listen address and port |
| `--data-dir` | `./hub-data` | Root directory for all hub data |
| `--lfs-dir` | `<data-dir>/lfs` | Directory for LFS object storage |
| `--max-repo-size` | `100 MB` | Maximum repository size |
| `--max-batch-size` | `50` | Maximum operations per batch request |

### Environment Variables

| Variable | Equivalent Flag |
|---|---|
| `SUTURE_ADDR` | `--addr` |
| `SUTURE_DATA_DIR` | `--data-dir` |
| `SUTURE_LFS_DIR` | `--lfs-dir` |

Environment variables take precedence over defaults but are overridden by CLI flags.

## Storage

### SQLite (default)

Zero configuration, good for deployments with fewer than 100 users. Uses WAL mode by default for improved concurrent access.

### PostgreSQL (optional)

For larger deployments requiring higher write throughput. Configure via `SUTURE_DATABASE_URL`.

### Data Directory Structure

```
hub-data/
├── hub.db           # Main database
├── repos/           # Repository data
│   └── {repo_id}/
│       ├── objects/ # Patch/content-addressable storage
│       └── refs/    # Branch/tag references
└── lfs/             # Large file storage
    └── objects/
```

## LFS Support

- **Enabled by default** — no additional configuration required
- **Max file size:** configurable via `--max-repo-size` (default 5 GB)
- **Storage backend:** local filesystem
- **S3 backend:** planned

## Backup

```bash
# Stop the hub
systemctl stop suture-hub

# Backup database
sqlite3 hub-data/hub.db ".backup hub-backup-$(date +%Y%m%d).db"

# Backup repos
tar czf hub-repos-backup-$(date +%Y%m%d).tar.gz hub-data/repos/

# Start the hub
systemctl start suture-hub
```

For zero-downtime backups with SQLite, use the online backup API or WAL checkpointing.

## Reverse Proxy

### Caddy

```Caddyfile
hub.suture.example.com {
    reverse_proxy localhost:8080
}
```

TLS is provisioned automatically.

### Nginx

```nginx
server {
    listen 443 ssl;
    server_name hub.suture.example.com;

    ssl_certificate     /etc/ssl/certs/hub.suture.example.com.pem;
    ssl_certificate_key /etc/ssl/private/hub.suture.example.com.key;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        client_max_body_size 5G;
    }
}
```

## Performance

| Metric | Value |
|---|---|
| Expected throughput | ~1000 pushes/hour on 2 CPU, 4 GB RAM |
| Scaling strategy | Vertical (more RAM/CPU) or horizontal (load balancer + shared storage) |
| Connection pooling | SQLite WAL mode (default) |

## Monitoring

- **Health check:** `GET /`
- **Metrics:** planned (Prometheus endpoint)
- **Logs:** set `RUST_LOG=debug` for verbose output

```bash
RUST_LOG=suture_hub=debug suture-hub --addr 0.0.0.0:8080
```

## Security

- **Authentication:** token-based
- **Rate limiting:** per-IP
- **CORS:** configurable
- **TLS:** always use in production

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `database is locked` | High concurrent write load | Increase SQLite `busy_timeout`; consider PostgreSQL |
| `permission denied` | Insufficient filesystem permissions | `chown -R <user>:<group> <data-dir>` |
| `address already in use` | Port conflict | Change `--addr` or stop the conflicting process |
| High memory usage | Large repos or many concurrent connections | Increase `--max-repo-size` limits or add more RAM |

## Helm Chart

A Helm chart for Kubernetes deployment is available at `deploy/helm/suture-hub/`.

```bash
helm install suture-hub deploy/helm/suture-hub
```

See [Helm Chart README](../deploy/helm/suture-hub/README.md) for full values reference.
