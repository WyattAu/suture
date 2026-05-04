# Enterprise Features Roadmap

## 1. SSO/SAML Integration

### Design
- Support SAML 2.0 and OpenID Connect (OIDC) for enterprise authentication
- Integrate with Okta, Azure AD, Google Workspace, OneLogin
- Store identity provider configuration in platform database
- Add `saml_config` and `oidc_config` tables

### API Endpoints
- `POST /auth/saml/metadata` — generate SP metadata XML
- `POST /auth/saml/acs` — SAML assertion consumer service
- `POST /auth/oidc/callback` — OIDC callback
- `GET /admin/auth/providers` — list configured providers
- `POST /admin/auth/providers` — add identity provider
- `DELETE /admin/auth/providers/{id}` — remove provider

### Implementation Notes
- Use `openssl` crate for SAML XML signature verification
- Use `openidconnect` crate for OIDC
- Map external identities to internal `users` table via `external_id` column
- Preserve existing JWT auth as fallback

## 2. Audit Logging

### Design
- Log all significant platform events to an immutable audit log
- Support export to SIEM systems (Splunk, Datadog, Elasticsearch)
- Retention policy: 90 days minimum

### Events to Log
- Authentication (login, logout, token refresh, SSO)
- Authorization (role changes, permission grants)
- Data access (repo read, file download, merge API call)
- Data modification (repo create/delete, branch protection change)
- Admin actions (user create/delete, org settings change)
- Billing (subscription change, webhook received)

### Schema
```sql
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY,
    timestamp TEXT NOT NULL,
    actor_id TEXT,
    actor_email TEXT,
    action TEXT NOT NULL,
    resource_type TEXT,
    resource_id TEXT,
    ip_address TEXT,
    user_agent TEXT,
    details TEXT, -- JSON
    org_id TEXT
);
CREATE INDEX idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_log_actor ON audit_log(actor_id);
CREATE INDEX idx_audit_log_action ON audit_log(action);
```

### API Endpoints
- `GET /admin/audit-logs?from=&to=&actor=&action=&limit=&offset=`
- `GET /admin/audit-logs/export?format=json|csv`

## 3. SLA Guarantees

### Design
- Track API response times and uptime
- Provide SLA dashboard in admin panel
- Alert on SLA breaches

### Targets (Enterprise)
- API availability: 99.95%
- API p99 latency: < 500ms
- Merge API p99 latency: < 5s (for files < 1MB)
- Support response time: < 4 hours

### Implementation
- Add middleware to track response times
- Store metrics in time-series table
- Expose `GET /admin/sla/metrics`
- Cron job to check SLA compliance

## 4. Advanced Rate Limiting

### Design (per-organization rate limits)
- Free: 100 merge API calls/day
- Pro: 10,000 merge API calls/day
- Enterprise: unlimited (within SLA)

### Implementation
- Redis-backed sliding window counter
- Per-org tracking
- `X-RateLimit-Remaining` header
- `Retry-After` header on 429
