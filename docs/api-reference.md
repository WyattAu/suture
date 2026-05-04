# Suture Platform API Reference

## Base URL

```
Platform:  https://api.suture.dev
Hub:       https://hub.suture.dev
Self-hosted: configured via --addr flag
```

## Authentication

All protected endpoints accept a JWT Bearer token in the `Authorization` header:

```
Authorization: Bearer <jwt_token>
```

Tokens are obtained via `/auth/login` or OAuth callbacks. Token expiry: 7 days.

All protected endpoints are rate-limited.

---

## Endpoints

### Health

#### `GET /healthz`

Health check endpoint.

**Auth:** None

**Response:** `200 OK`

```json
"ok"
```

---

### Auth

#### `POST /auth/register`

Register a new account.

**Auth:** None

**Request:**

```json
{
  "email": "user@example.com",
  "password": "at-least-8-chars",
  "display_name": "Alice"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `email` | string | yes | Valid email (max 254 chars) |
| `password` | string | yes | Min 8 characters |
| `display_name` | string | no | Display name |

**Response:** `201 Created`

```json
{
  "token": "eyJ...",
  "user": {
    "user_id": "uuid",
    "email": "user@example.com",
    "display_name": "Alice",
    "tier": "free",
    "created_at": "2026-01-01T00:00:00+00:00"
  }
}
```

**Error Codes:**

| Status | Description |
|--------|-------------|
| 400 | Invalid email or password too short |
| 409 | Email already registered |

#### `POST /auth/login`

Authenticate and receive a JWT.

**Auth:** None

**Request:**

```json
{
  "email": "user@example.com",
  "password": "secret"
}
```

**Response:** `200 OK`

```json
{
  "token": "eyJ...",
  "user": {
    "user_id": "uuid",
    "email": "user@example.com",
    "display_name": "Alice",
    "tier": "free",
    "created_at": "2026-01-01T00:00:00+00:00"
  }
}
```

**Error Codes:**

| Status | Description |
|--------|-------------|
| 401 | Invalid email or password |

#### `GET /auth/oauth/start?provider=<provider>`

Start an OAuth flow.

**Auth:** None

**Query Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `provider` | string | `google` or `github` |

**Response:** `200 OK`

```json
{
  "url": "https://accounts.google.com/o/oauth2/v2/auth?..."
}
```

#### `GET /auth/google/callback`

Google OAuth callback. Redirects with JWT on success.

**Auth:** None

#### `GET /auth/github/callback`

GitHub OAuth callback. Redirects with JWT on success.

**Auth:** None

#### `GET /auth/me`

Get the current authenticated user.

**Auth:** Bearer token required

**Response:** `200 OK`

```json
{
  "user_id": "uuid",
  "email": "user@example.com",
  "display_name": "Alice",
  "tier": "free",
  "created_at": "2026-01-01T00:00:00+00:00"
}
```

**Error Codes:**

| Status | Description |
|--------|-------------|
| 401 | Missing or invalid token |
| 404 | User not found |

#### `POST /auth/logout`

Revoke the current JWT.

**Auth:** Bearer token required

**Response:** `200 OK`

```json
{
  "logged_out": true
}
```

---

### Merge

#### `GET /api/drivers`

List all available merge drivers.

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
    }
  ]
}
```

#### `POST /api/merge`

Perform a semantic three-way merge.

**Auth:** Bearer token required. Subject to billing limits.

**Request:**

```json
{
  "driver": "json",
  "base": "{\"name\": \"base\", \"version\": 1}",
  "ours": "{\"name\": \"ours\", \"version\": 2}",
  "theirs": "{\"name\": \"theirs\", \"version\": 1}"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `driver` | string | yes | Driver name (`json`, `yaml`, `toml`, `xml`, `csv`, `sql`, `html`, `markdown`, `svg`, `docx`, `xlsx`, `pptx`, `pdf`, `feed`, `ical`, `image`, `otio`, `properties`) |
| `base` | string | yes | Ancestor (common base) content |
| `ours` | string | yes | Current branch content |
| `theirs` | string | yes | Incoming branch content |

**Response:** `200 OK`

```json
{
  "result": "{\"name\":\"ours\",\"version\":2}",
  "driver": "json",
  "conflicts": false
}
```

When conflicts cannot be automatically resolved, `conflicts` is `true` and
`result` is `null`.

**Error Codes:**

| Status | Description |
|--------|-------------|
| 400 | Invalid driver name or malformed input |
| 403 | Merge limit reached for billing period |

---

### Usage

#### `GET /api/usage`

Get current usage and billing limits.

**Auth:** Bearer token required

**Response:** `200 OK`

```json
{
  "tier": "free",
  "merges_used": 42,
  "merges_limit": 100,
  "storage_bytes": 5242880,
  "storage_limit": 104857600,
  "repos_count": 2,
  "repos_limit": 5,
  "api_calls": 150,
  "period": "2026-01",
  "utilization_percent": 42.0
}
```

Tier limits (`merges_limit`, `repos_limit`) are `-1` for unlimited (Pro/Enterprise).

---

### Analytics

#### `GET /api/analytics`

Get merge analytics. **Pro tier or higher required.**

**Auth:** Bearer token required (Pro+)

**Response:** `200 OK`

```json
{
  "total_merges": 1234,
  "merges_today": 15,
  "merges_this_week": 87,
  "merges_by_driver": {
    "json": 500,
    "yaml": 300,
    "toml": 200
  },
  "merges_by_day": [
    {"date": "2026-01-28", "count": 12},
    {"date": "2026-01-29", "count": 15}
  ],
  "conflicts_resolved": 1100,
  "conflicts_detected": 134,
  "avg_merge_time_ms": 2.5,
  "active_users_today": 3
}
```

**Error Codes:**

| Status | Description |
|--------|-------------|
| 403 | Analytics available on Pro plan |

---

### Organizations

#### `POST /api/orgs`

Create an organization.

**Auth:** Bearer token required

**Request:**

```json
{
  "name": "my-org",
  "display_name": "My Organization"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | 2-39 alphanumeric chars, hyphens/underscores |
| `display_name` | string | no | Human-readable name |

**Response:** `201 Created`

```json
{
  "org_id": "uuid",
  "name": "my-org",
  "display_name": "My Organization",
  "tier": "free",
  "member_count": 1,
  "is_owner": true
}
```

**Error Codes:**

| Status | Description |
|--------|-------------|
| 400 | Invalid org name |
| 409 | Organization name already taken |

#### `GET /api/orgs`

List organizations the current user belongs to.

**Auth:** Bearer token required

**Response:** `200 OK` — array of `OrgInfo` objects.

#### `POST /api/orgs/{org_id}/invite`

Invite a member or directly add an existing user.

**Auth:** Bearer token required (org admin/owner)

**Request:**

```json
{
  "email": "newuser@example.com",
  "role": "member"
}
```

Valid roles: `owner`, `admin`, `member`, `viewer`.

**Response:** `200 OK`

```json
{
  "status": "added",
  "user_id": "uuid",
  "email": "newuser@example.com"
}
```

If the email is not registered, `status` is `"invited"` and `user_id` is `null`.

**Error Codes:**

| Status | Description |
|--------|-------------|
| 403 | Not an org admin |
| 409 | User is already a member |

#### `GET /api/orgs/{org_id}/members`

List organization members.

**Auth:** Bearer token required (org member)

**Response:** `200 OK` — array of `MemberInfo` objects:

```json
[
  {
    "user_id": "uuid",
    "role": "owner",
    "joined_at": "2026-01-01T00:00:00+00:00",
    "email": "owner@example.com",
    "display_name": "Owner"
  }
]
```

#### `DELETE /api/orgs/{org_id}/members/{user_id}`

Remove a member from an organization.

**Auth:** Bearer token required (org admin/owner)

**Response:** `204 No Content`

**Error Codes:**

| Status | Description |
|--------|-------------|
| 403 | Not an org admin, or attempting to remove the last admin |
| 404 | Member not found |

#### `PUT /api/orgs/{org_id}/members/{user_id}/role`

Update a member's role.

**Auth:** Bearer token required (org admin/owner)

**Request:**

```json
{
  "role": "admin"
}
```

**Response:** `204 No Content`

#### `GET /api/invitations`

List pending invitations (sent by user or received by user).

**Auth:** Bearer token required

**Response:** `200 OK` — array of `InvitationInfo` objects.

#### `POST /api/invitations/{invite_id}/accept`

Accept an invitation.

**Auth:** Bearer token required (email must match invitation)

**Response:** `204 No Content`

**Error Codes:**

| Status | Description |
|--------|-------------|
| 400 | Email does not match invitation |
| 404 | Invitation not found or expired |

---

### Plugins

#### `GET /api/plugins`

List loaded WASM plugins.

**Auth:** None

**Response:** `200 OK`

```json
{
  "plugins": [
    {
      "name": "custom-driver",
      "version": "1.0.0"
    }
  ],
  "count": 1
}
```

#### `POST /api/plugins/upload`

Upload a WASM plugin. **Enterprise tier required.**

**Auth:** Bearer token required (Enterprise)

**Request:** `multipart/form-data` with a `.wasm` file field.

**Response:** `201 Created`

```json
{
  "name": "custom-driver.wasm",
  "status": "loaded",
  "driver": "plugin-custom-driver"
}
```

**Error Codes:**

| Status | Description |
|--------|-------------|
| 400 | Invalid Wasm module or malformed filename |
| 403 | Plugin uploads require enterprise tier |

#### `POST /api/plugins/merge`

Merge using a custom plugin driver.

**Auth:** Bearer token required

**Request:**

```json
{
  "driver": "plugin-custom-driver",
  "base": "...",
  "ours": "...",
  "theirs": "..."
}
```

**Response:** Same shape as `POST /api/merge`.

**Error Codes:**

| Status | Description |
|--------|-------------|
| 404 | Plugin not loaded |

---

### Billing

#### `POST /billing/checkout`

Create a Stripe Checkout session for upgrading.

**Auth:** Bearer token required

**Request:**

```json
{
  "tier": "pro",
  "success_url": "https://example.com/success",
  "cancel_url": "https://example.com/cancel"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tier` | string | yes | `pro` or `enterprise` |
| `success_url` | string | no | Redirect after payment |
| `cancel_url` | string | no | Redirect on cancel |

**Response:** `200 OK`

```json
{
  "url": "https://checkout.stripe.com/c/pay/cs_test_..."
}
```

**Error Codes:**

| Status | Description |
|--------|-------------|
| 400 | Invalid tier |
| 503 | Billing not configured |

#### `GET /billing/subscription`

Get current subscription status.

**Auth:** Bearer token required

**Response:** `200 OK`

```json
{
  "tier": "pro",
  "status": "active",
  "current_period_end": null,
  "cancel_at_period_end": false
}
```

#### `POST /billing/portal`

Create a Stripe Customer Portal session for managing subscriptions.

**Auth:** Bearer token required

**Response:** `200 OK`

```json
{
  "url": "https://billing.stripe.com/p/session_..."
}
```

**Error Codes:**

| Status | Description |
|--------|-------------|
| 400 | No Stripe customer on file |
| 503 | Billing not configured |

#### `POST /billing/webhook`

Receive Stripe webhook events. Called by Stripe, not by clients.

**Auth:** Stripe-Signature header verification (HMAC-SHA256)

Handles: `checkout.session.completed`, `customer.subscription.updated`,
`customer.subscription.deleted`, `invoice.payment_failed`.

**Response:** `200 OK`

```json
{
  "received": true
}
```

---

### Admin

#### `GET /admin/users`

List all users. **Admin role required.**

**Auth:** Bearer token required (admin)

**Response:** `200 OK` — array of `UserInfo` objects.

**Error Codes:**

| Status | Description |
|--------|-------------|
| 403 | Admin only |

---

## Error Response Format

All errors return a consistent JSON body:

```json
{
  "error": "Human-readable error message"
}
```

## Rate Limiting

All protected endpoints are rate-limited. Responses include standard
`RateLimit-*` headers when limits are approached or exceeded.

## Tier Limits

| Feature | Free | Pro | Enterprise |
|---------|------|-----|------------|
| Merges/month | 100 | 10,000 | Unlimited |
| Storage | 100 MB | 10 GB | 100 GB |
| Repos | 5 | Unlimited | Unlimited |
| Core drivers | 5 | All | All |
| Analytics | — | Yes | Yes |
| WASM plugins | — | — | Yes |
| SSO/Audit/SLA | — | — | Yes |
| Price | $0 | $9/user/mo | $29/user/mo |
