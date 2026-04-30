# Suture API Reference v5.1.0

## Base URL

```
Platform: https://api.suture.dev
Hub:      https://hub.suture.dev
Self-hosted: configured via --addr flag
```

## Authentication

All protected endpoints accept a JWT Bearer token in the `Authorization` header:

```
Authorization: Bearer <jwt_token>
```

Tokens are obtained via `/auth/login` or OAuth callbacks. Token expiry: 7 days.

---

## Authentication Endpoints

### POST /auth/register

Create a new account.

**Auth:** None

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `email` | `string` | Yes | Valid email (max 254 chars, must contain `@`) |
| `password` | `string` | Yes | Min 8 characters |
| `display_name` | `string` | No | Display name (defaults to empty) |

```json
{
  "email": "user@example.com",
  "password": "securepassword",
  "display_name": "Jane Doe"
}
```

**Response:** `201 Created`

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "user_id": "uuid-v4",
    "email": "user@example.com",
    "display_name": "Jane Doe",
    "tier": "free",
    "created_at": "2026-05-01T00:00:00+00:00"
  }
}
```

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | Invalid email or password too short |
| 500 | Internal server error |

---

### POST /auth/login

Authenticate and receive a JWT.

**Auth:** None

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `email` | `string` | Yes | Registered email |
| `password` | `string` | Yes | Account password |

```json
{
  "email": "user@example.com",
  "password": "securepassword"
}
```

**Response:** `200 OK`

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "user_id": "uuid-v4",
    "email": "user@example.com",
    "display_name": "Jane Doe",
    "tier": "pro",
    "created_at": "2026-04-15T00:00:00+00:00"
  }
}
```

**Error codes:**

| Status | Error |
|--------|-------|
| 401 | Invalid email or password |
| 500 | Internal server error |

---

### POST /auth/logout

Revoke the current token.

**Auth:** Optional (JWT)

**Request body:** None

**Response:** `200 OK`

```json
{
  "logged_out": true
}
```

---

### GET /auth/me

Get the current authenticated user's info.

**Auth:** Optional (JWT — returns 401 if not authenticated)

**Response:** `200 OK`

```json
{
  "user_id": "uuid-v4",
  "email": "user@example.com",
  "display_name": "Jane Doe",
  "tier": "pro",
  "created_at": "2026-04-15T00:00:00+00:00"
}
```

**Error codes:**

| Status | Error |
|--------|-------|
| 401 | Missing or invalid token |
| 404 | User not found |

---

### POST /auth/github

GitHub OAuth login. This is a two-step flow:

**Step 1: Start OAuth**

`GET /auth/oauth/start?provider=github`

**Auth:** None

**Response:** `200 OK`

```json
{
  "url": "https://github.com/login/oauth/authorize?client_id=...&redirect_uri=...&scope=user:email&state=uuid-v4"
}
```

**Step 2: Callback**

`GET /auth/github/callback?code=<auth_code>&state=<csrf_state>`

**Auth:** None

The CSRF state token is validated (one-time use, 10-minute expiry, prevents CSRF attacks). On success, creates or links a user account.

**Response:** `200 OK`

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "user_id": "uuid-v4",
    "email": "user@example.com",
    "display_name": "jane",
    "tier": "free",
    "created_at": "2026-05-01T00:00:00+00:00"
  }
}
```

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | Missing/expired state parameter, CSRF detected |
| 503 | GitHub OAuth not configured |
| 502 | GitHub API error |

**Google OAuth** follows the same pattern with `provider=google`.

---

## Merge API

### POST /api/merge

Perform a three-way semantic merge.

**Auth:** Optional (JWT — required for usage tracking and billing limits)

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `driver` | `string` | Yes | Driver name or file extension (e.g., `json`, `yaml`, `docx`, `otio`) |
| `base` | `string` | Yes | Base version content |
| `ours` | `string` | Yes | Our version content |
| `theirs` | `string` | Yes | Their version content |

```json
{
  "driver": "json",
  "base": "{\"name\": \"Alice\", \"age\": 30}",
  "ours": "{\"name\": \"Alice\", \"age\": 31}",
  "theirs": "{\"name\": \"Alice\", \"role\": \"admin\"}"
}
```

**Response (clean merge):** `200 OK`

```json
{
  "result": "{\"name\": \"Alice\", \"age\": 31, \"role\": \"admin\"}",
  "driver": "json",
  "conflicts": false
}
```

**Response (conflict):** `200 OK`

```json
{
  "result": null,
  "driver": "json",
  "conflicts": true
}
```

**Supported drivers:** `json`, `yaml`/`yml`, `toml`, `xml`, `csv`, `sql`, `properties`, `html`/`htm`, `markdown`/`md`/`mdown`/`mkd`, `svg`, `docx`, `feed`/`rss`/`atom`, `ical`/`ics`/`ifb`, `image`/`png`/`jpg`/`jpeg`/`gif`/`bmp`/`webp`/`tiff`/`tif`/`ico`/`avif`, `otio`, `pdf`, `pptx`, `xlsx`

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | Unsupported driver or merge error |
| 403 | Merge limit reached for billing period |
| 500 | Internal server error |

**Rate limits:** Counts against monthly merge quota.

---

### GET /api/drivers

List all supported merge drivers.

**Auth:** None

**Response:** `200 OK`

```json
{
  "drivers": [
    {
      "name": "JSON",
      "extensions": [".json"]
    },
    {
      "name": "YAML",
      "extensions": [".yaml", ".yml"]
    },
    {
      "name": "DOCX",
      "extensions": [".docx"]
    }
  ]
}
```

Returns 18 driver entries (JSON, YAML, TOML, XML, CSV, SQL, Properties, HTML, Markdown, SVG, DOCX, Feed, iCal, Image, OpenTimelineIO, PDF, PPTX, XLSX).

---

### GET /api/usage

Get current usage and billing limits.

**Auth:** Optional (JWT)

**Response:** `200 OK`

```json
{
  "tier": "free",
  "merges_used": 47,
  "merges_limit": 100,
  "storage_bytes": 52428800,
  "storage_limit": 104857600,
  "repos_count": 3,
  "repos_limit": 5,
  "api_calls": 1200,
  "period": "2026-05",
  "utilization_percent": 47.0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `tier` | `string` | `free`, `pro`, or `enterprise` |
| `merges_used` | `integer` | Merges used this billing period |
| `merges_limit` | `integer` | Monthly merge limit (-1 = unlimited) |
| `storage_bytes` | `integer` | Storage used in bytes |
| `storage_limit` | `integer` | Storage limit in bytes |
| `repos_count` | `integer` | Number of repositories |
| `repos_limit` | `integer` | Repository limit (-1 = unlimited) |
| `api_calls` | `integer` | API calls this period |
| `period` | `string` | Billing period (YYYY-MM) |
| `utilization_percent` | `float` | Merge utilization percentage |

**Error codes:**

| Status | Error |
|--------|-------|
| 500 | Internal server error |

---

## Organization API

### POST /api/orgs

Create a new organization.

**Auth:** Optional (JWT)

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | Yes | 2-39 alphanumeric chars, hyphens and underscores allowed |
| `display_name` | `string` | No | Human-readable name (defaults to `name`) |

```json
{
  "name": "my-team",
  "display_name": "My Team"
}
```

**Response:** `201 Created`

```json
{
  "org_id": "uuid-v4",
  "name": "my-team",
  "display_name": "My Team",
  "tier": "free",
  "member_count": 1,
  "is_owner": true
}
```

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | Invalid org name |
| 409 | Organization name already taken |
| 500 | Internal server error |

---

### GET /api/orgs

List organizations the authenticated user belongs to.

**Auth:** Optional (JWT)

**Response:** `200 OK`

```json
[
  {
    "org_id": "uuid-v4",
    "name": "my-team",
    "display_name": "My Team",
    "tier": "pro",
    "member_count": 5,
    "is_owner": true
  }
]
```

---

### POST /api/orgs/{org_id}/invite

Invite a member to an organization (admin/owner only).

**Auth:** Optional (JWT — requires admin or owner role)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `org_id` | `string` | Organization UUID |

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `email` | `string` | Yes | Email of user to invite |
| `role` | `string` | Yes | One of: `owner`, `admin`, `member`, `viewer` |

```json
{
  "email": "newuser@example.com",
  "role": "member"
}
```

**Response:** `200 OK`

If the user already has an account:

```json
{
  "status": "added",
  "user_id": "uuid-v4",
  "email": "newuser@example.com"
}
```

If the user does not exist (invitation sent):

```json
{
  "status": "invited",
  "user_id": null,
  "email": "newuser@example.com"
}
```

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | Invalid role |
| 403 | Not an admin of this organization |
| 404 | Organization not found |
| 409 | User is already a member |
| 500 | Internal server error |

---

### GET /api/orgs/{org_id}/members

List organization members.

**Auth:** Optional (JWT — requires member role)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `org_id` | `string` | Organization UUID |

**Response:** `200 OK`

```json
[
  {
    "user_id": "uuid-v4",
    "role": "owner",
    "joined_at": "2026-04-01T00:00:00+00:00",
    "email": "admin@example.com",
    "display_name": "Admin User"
  },
  {
    "user_id": "uuid-v4",
    "role": "member",
    "joined_at": "2026-04-15T00:00:00+00:00",
    "email": "member@example.com",
    "display_name": "Team Member"
  }
]
```

**Error codes:**

| Status | Error |
|--------|-------|
| 403 | Not a member of this organization |
| 500 | Internal server error |

---

### PUT /api/orgs/{org_id}/members/{user_id}/role

Update a member's role (admin/owner only).

**Auth:** Optional (JWT — requires admin or owner role)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `org_id` | `string` | Organization UUID |
| `user_id` | `string` | Target user UUID |

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `role` | `string` | Yes | One of: `owner`, `admin`, `member`, `viewer` |

```json
{
  "role": "admin"
}
```

**Response:** `204 No Content`

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | Invalid role |
| 403 | Not an admin of this organization |
| 404 | Member not found in organization |
| 500 | Internal server error |

---

### DELETE /api/orgs/{org_id}/members/{user_id}

Remove a member from the organization (admin/owner only).

**Auth:** Optional (JWT — requires admin or owner role)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `org_id` | `string` | Organization UUID |
| `user_id` | `string` | Target user UUID |

**Request body:** None

**Response:** `204 No Content`

**Error codes:**

| Status | Error |
|--------|-------|
| 403 | Not an admin, or attempting to remove the last admin |
| 404 | Organization not found |
| 409 | Cannot remove the last admin |
| 500 | Internal server error |

---

### GET /api/invitations

List pending and sent invitations.

**Auth:** Optional (JWT — shows invitations where user is admin/owner or is the invited email)

**Response:** `200 OK`

```json
[
  {
    "invite_id": "inv_uuid-v4",
    "org_id": "uuid-v4",
    "org_name": "My Team",
    "email": "newuser@example.com",
    "role": "member",
    "invited_by": "uuid-v4",
    "created_at": "2026-05-01T00:00:00+00:00",
    "expires_at": "2026-05-08T00:00:00+00:00",
    "accepted_at": null
  }
]
```

---

### POST /api/invitations/{invite_id}/accept

Accept an organization invitation.

**Auth:** Optional (JWT — invite email must match authenticated user's email)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `invite_id` | `string` | Invitation ID (`inv_*`) |

**Request body:** None

**Response:** `204 No Content`

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | Email mismatch or invalid invitation |
| 404 | Invitation not found or expired |
| 500 | Internal server error |

---

## Billing API

### POST /billing/checkout

Create a Stripe checkout session for subscription purchase.

**Auth:** Optional (JWT)

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tier` | `string` | Yes | `pro` or `enterprise` |
| `success_url` | `string` | No | Redirect on success (defaults to `{base}/billing?success=true`) |
| `cancel_url` | `string` | No | Redirect on cancel (defaults to `{base}/billing?canceled=true`) |

```json
{
  "tier": "pro",
  "success_url": "https://app.example.com/billing?success=true"
}
```

**Response:** `200 OK`

```json
{
  "url": "https://checkout.stripe.com/c/pay/cs_live_..."
}
```

Redirect the user to this URL to complete payment.

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | Invalid tier |
| 500 | Failed to create Stripe customer or checkout session |
| 503 | Billing not configured (STRIPE_KEY not set) |

**Rate limits:** Standard tier-based limits apply.

---

### POST /billing/portal

Create a Stripe customer portal session for managing subscriptions.

**Auth:** Optional (JWT)

**Request body:** None

**Response:** `200 OK`

```json
{
  "url": "https://billing.stripe.com/p/session_..."
}
```

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | No Stripe customer on file |
| 500 | Failed to create portal session |
| 503 | Billing not configured |

---

### GET /billing/subscription

Get current subscription info.

**Auth:** Optional (JWT)

**Response:** `200 OK`

```json
{
  "tier": "pro",
  "status": "active",
  "current_period_end": null,
  "cancel_at_period_end": false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `tier` | `string` | Current tier: `free`, `pro`, or `enterprise` |
| `status` | `string` | `active`, `inactive`, `past_due`, `canceled`, `trialing` |
| `current_period_end` | `string or null` | End of current billing period |
| `cancel_at_period_end` | `boolean` | Whether subscription cancels at period end |

---

### POST /billing/webhook

Stripe webhook endpoint. Not called directly by users — Stripe sends events here.

**Auth:** None (HMAC-SHA256 signature verification via `Stripe-Signature` header)

**Handled events:**

| Event | Action |
|-------|--------|
| `checkout.session.completed` | Upgrade user tier, create subscription record |
| `customer.subscription.updated` | Update tier based on subscription status |
| `customer.subscription.deleted` | Downgrade to free tier |
| `invoice.payment_failed` | Set 7-day payment grace period |

**Security:**

- HMAC-SHA256 signature verification (when `STRIPE_WEBHOOK_SECRET` is set)
- Timestamp replay protection: rejects webhooks older than 300 seconds
- Logs warning if webhook secret is not configured

**Response:** `200 OK`

```json
{
  "received": true
}
```

---

## Analytics API

### GET /api/analytics

Get usage analytics. **Pro and Enterprise tiers only.**

**Auth:** Optional (JWT — requires Pro or Enterprise tier)

**Response:** `200 OK`

```json
{
  "total_merges": 4521,
  "merges_today": 12,
  "merges_this_week": 87,
  "merges_by_driver": {
    "json": 2100,
    "yaml": 1200,
    "docx": 500,
    "xlsx": 400,
    "toml": 321
  },
  "merges_by_day": [
    {"date": "2026-04-25", "count": 15},
    {"date": "2026-04-26", "count": 22},
    {"date": "2026-04-27", "count": 8}
  ],
  "conflicts_resolved": 3900,
  "conflicts_detected": 621,
  "avg_merge_time_ms": 3.42,
  "active_users_today": 1
}
```

| Field | Type | Description |
|-------|------|-------------|
| `total_merges` | `integer` | Lifetime merge count |
| `merges_today` | `integer` | Merges today |
| `merges_this_week` | `integer` | Merges in the last 7 days |
| `merges_by_driver` | `object` | Merge counts grouped by driver name |
| `merges_by_day` | `array` | Daily merge counts for the last 30 days |
| `conflicts_resolved` | `integer` | Clean merges |
| `conflicts_detected` | `integer` | Merges with conflicts |
| `avg_merge_time_ms` | `float` | Average merge time in milliseconds |
| `active_users_today` | `integer` | Active users today (org-wide) |

**Error codes:**

| Status | Error |
|--------|-------|
| 403 | Analytics available on Pro plan |
| 500 | Internal server error |

---

## Plugin API

### GET /api/plugins

List currently loaded WASM plugins.

**Auth:** None

**Response:** `200 OK`

```json
{
  "count": 2,
  "plugins": [
    {
      "name": "custom-merge-driver",
      "driver_name": "Custom Merge Driver",
      "version": "1.0.0",
      "extensions": [".custom"]
    }
  ]
}
```

---

### POST /api/plugins/upload

Upload a WASM plugin. **Enterprise tier only.**

**Auth:** Optional (JWT — requires Enterprise tier)

**Request body:** `multipart/form-data`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| File | `binary` | Yes | `.wasm` file |

**Response:** `201 Created`

```json
{
  "name": "custom-driver.wasm",
  "status": "loaded",
  "driver": "Custom Merge Driver"
}
```

**Error codes:**

| Status | Error |
|--------|-------|
| 400 | No file uploaded, invalid Wasm module, or failed to load |
| 403 | Plugin uploads require enterprise tier |
| 500 | Internal server error |

---

### POST /api/plugins/merge

Merge using a loaded WASM plugin.

**Auth:** Optional (JWT)

**Request body:** Same as `POST /api/merge`, but `driver` must match a loaded plugin name.

```json
{
  "driver": "plugin-custom-merge-driver",
  "base": "...",
  "ours": "...",
  "theirs": "..."
}
```

**Response:** Same format as `POST /api/merge`.

**Error codes:**

| Status | Error |
|--------|-------|
| 404 | Plugin not loaded |
| 500 | Plugin execution error |

---

## Rate Limiting

All endpoints are subject to rate limiting. Limits are applied per-user (authenticated) or per-IP (anonymous) using a 60-second sliding window.

| Tier | Requests/minute |
|------|----------------|
| Anonymous (per IP) | 10 |
| Free | 30 |
| Pro | 300 |
| Enterprise | 3,000 |

Rate limit information is returned in response headers:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests per window |
| `X-RateLimit-Remaining` | Remaining requests in current window |
| `X-RateLimit-Reset` | Seconds until window resets |
| `Retry-After` | Seconds to wait (only on 429 responses) |

**Response on limit exceeded:** `429 Too Many Requests`

```json
{
  "error": "rate limit exceeded",
  "retry_after_seconds": 42,
  "limit": 30
}
```

---

## Error Response Format

All errors follow a consistent format:

```json
{
  "error": "Human-readable error message"
}
```

Some errors include additional context:

```json
{
  "error": "merge limit reached for this billing period",
  "tier": "free",
  "upgrade_url": "/billing"
}
```

```json
{
  "error": "billing is not configured",
  "hint": "set STRIPE_KEY environment variable"
}
```

---

## Billing Tiers

| Feature | Free | Pro ($9/seat/mo) | Enterprise ($29/seat/mo) |
|---------|------|-------------------|--------------------------|
| Merges/month | 100 | 10,000 | Unlimited |
| Storage | 100 MB | 10 GB | 100 GB |
| Repositories | 5 | Unlimited | Unlimited |
| API rate limit | 30/min | 300/min | 3,000/min |
| Analytics | No | Yes | Yes |
| WASM plugins | No | No | Yes |
| Drivers | 5 | Unlimited | Unlimited |

---

## CORS

The platform exposes a permissive CORS policy (`CorsLayer::permissive()`). For production deployments, this should be configured to specific origins.

---

## Health Check

### GET /healthz

**Auth:** None

**Response:** `200 OK`

```
ok
```

Available on both platform and hub.
