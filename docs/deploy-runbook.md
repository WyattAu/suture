# Suture 5.1.0 Deployment Runbook

Step-by-step instructions for deploying the Suture platform, publishing packages, and launching v5.1.0. Every command, every click, no ambiguity.

---

## Section 1: Prerequisites

| Requirement | Details |
|-------------|---------|
| GitHub account | Push access to `WyattAu/suture` |
| Stripe account | Free tier — [stripe.com](https://stripe.com) |
| Fly.io account | Free tier — [fly.io](https://fly.io), credit card required for verification |
| Node.js 20+ | Required for GitHub Action build and VS Code extension packaging |
| Rust toolchain | Stable, matching `rust-toolchain.toml` |
| npm account | For publishing `suture-merge-driver` |
| PyPI account | For publishing `suture-merge-driver` |
| crates.io account | For publishing Rust crates |
| `flyctl` CLI | Installed in Section 2 |
| `vsce` CLI | Installed in Section 6 |

---

## Section 2: Platform Deployment

```bash
# 1. Install flyctl
curl -L https://fly.io/install.sh | sh

# 2. Authenticate (opens browser)
fly auth login

# 3. Create the app (run from repo root, where fly.toml lives)
fly apps create suture-platform

# 4. Set required secrets
fly secrets set JWT_SECRET=$(openssl rand -hex 32)
fly secrets set DATABASE_URL=/data/platform.db
```

### Optional: Stripe integration

```bash
# Set these after completing Section 4 (Stripe Configuration)
fly secrets set STRIPE_SECRET_KEY=sk_test_...
fly secrets set STRIPE_WEBHOOK_SECRET=whsec_...
fly secrets set STRIPE_PRICE_PRO=price_...
fly secrets set STRIPE_PRICE_ENTERPRISE=price_...
```

### Optional: GitHub OAuth

```bash
# Set these after creating an OAuth App in GitHub Developer Settings
# https://github.com/settings/developers → New OAuth App
fly secrets set GITHUB_CLIENT_ID=...
fly secrets set GITHUB_CLIENT_SECRET=...
fly secrets set GITHUB_REDIRECT_URL=https://suture-platform.fly.dev/auth/github/callback
```

### Deploy

```bash
# Deploy from repo root (uses fly.toml + Dockerfile)
fly deploy

# Verify
fly status
curl https://suture-platform.fly.dev/healthz
# Expected: {"status":"ok"}
```

---

## Section 3: Custom Domain

```bash
# Add your domain
fly certs add suture.dev

# Check certificate status (may take a few minutes)
fly certs list

# DNS configuration: add these records at your registrar
#   CNAME  *.suture.dev  →  suture-platform.fly.dev
#   CNAME  suture.dev    →  suture-platform.fly.dev

# Verify DNS propagation
dig suture.dev +short
```

---

## Section 4: Stripe Configuration

### Step 1: Create Products

1. Go to [stripe.com → Products → Add Product](https://dashboard.stripe.com/products)
2. Create **Suture Pro**:
   - Name: `Suture Pro`
   - Type: **Recurring**
   - Price: `$9 / month`
   - Copy the **price ID** (looks like `price_1Qx...`)
3. Create **Suture Enterprise**:
   - Name: `Suture Enterprise`
   - Type: **Recurring**
   - Price: `$29 / month`
   - Copy the **price ID**

### Step 2: Create Webhook Endpoint

1. Go to [stripe.com → Developers → Webhooks → Add endpoint](https://dashboard.stripe.com/webhooks)
2. Endpoint URL: `https://suture-platform.fly.dev/billing/webhook`
3. Events to listen for (select these four):
   - `checkout.session.completed`
   - `customer.subscription.updated`
   - `customer.subscription.deleted`
   - `invoice.payment_failed`
4. Click **Add endpoint**
5. Copy the **Signing secret** (looks like `whsec_...`)

### Step 3: Set Stripe secrets on Fly.io

```bash
fly secrets set STRIPE_SECRET_KEY=sk_test_...
fly secrets set STRIPE_WEBHOOK_SECRET=whsec_...
fly secrets set STRIPE_PRICE_PRO=price_...
fly secrets set STRIPE_PRICE_ENTERPRISE=price_...
fly deploy
```

### Step 4: Verify

```bash
# Use the Stripe CLI to send a test webhook event
stripe listen --forward-to https://suture-platform.fly.dev/billing/webhook

# In another terminal, trigger a test event
stripe trigger checkout.session.completed

# Verify the platform logs received the event
fly logs
```

---

## Section 5: GitHub Action Marketplace

1. Go to [github.com/marketplace/actions/new](https://github.com/marketplace/actions/new)
2. Fill in:
   - **Name:** `Suture Semantic Merge`
   - **Description:** `Automatically merge JSON, YAML, TOML, XML, CSV, and 12+ structured file formats in CI/CD pipelines`
   - **Owner:** `WyattAu`
   - **Repository:** `WyattAu/suture`
   - **Action file:** `action.yml` (root of repo)
3. Click **Publish action**

### Verify the action works

Create a test workflow in a separate repo:

```yaml
# .github/workflows/test-suture.yml
name: Test Suture Merge
on: [push]
jobs:
  merge:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: WyattAu/suture@v5.1.0
        with:
          formats: "json,yaml,toml"
```

---

## Section 6: VS Code Marketplace

```bash
# 1. Install vsce
npm install -g @vscode/vsce

# 2. Package the extension (run from vscode-extension/ directory)
cd vscode-extension
vsce package
# Creates: suture-merge-5.1.0.vsix

# 3. Create a publisher (one-time only)
#    Go to https://marketplace.visualstudio.com/manage/publishers
#    Click "Create Publisher"
#    Fill in publisher ID (e.g., "wyattau") and display name

# 4. Login to publisher
vsce login wyattau

# 5. Publish
vsce publish
```

---

## Section 7: crates.io Publishing

### Setup (one-time)

```bash
# Login to crates.io
cargo login
# Paste API token from https://crates.io/settings/tokens
```

### Publish in dependency order

```bash
# First: leaf crates with no internal dependencies
cargo publish -p suture-common
cargo publish -p suture-bench
cargo publish -p suture-driver
cargo publish -p suture-wasm-plugin

# Then: format drivers (depend on suture-driver)
cargo publish -p suture-driver-json
cargo publish -p suture-driver-yaml
cargo publish -p suture-driver-toml
cargo publish -p suture-driver-xml
cargo publish -p suture-driver-csv
cargo publish -p suture-driver-sql
cargo publish -p suture-driver-html
cargo publish -p suture-driver-markdown
cargo publish -p suture-driver-svg
cargo publish -p suture-driver-properties
cargo publish -p suture-driver-docx
cargo publish -p suture-driver-xlsx
cargo publish -p suture-driver-pptx
cargo publish -p suture-driver-pdf
cargo publish -p suture-driver-image
cargo publish -p suture-driver-feed
cargo publish -p suture-driver-ical
cargo publish -p suture-driver-otio
cargo publish -p suture-driver-example

# Then: core crates
cargo publish -p suture-core
cargo publish -p suture-protocol
cargo publish -p suture-merge
cargo publish -p suture-raft
cargo publish -p suture-s3
cargo publish -p suture-vfs

# Then: application crates
cargo publish -p suture-cli
cargo publish -p suture-tui
cargo publish -p suture-lsp
cargo publish -p suture-daemon
cargo publish -p suture-hub

# Finally: platform
cargo publish -p suture-platform
```

### Batch republish (if some crates already exist at v5.1.0)

```bash
for crate in suture-common suture-core suture-driver suture-driver-json suture-driver-yaml suture-driver-toml suture-driver-xml suture-driver-csv suture-driver-sql suture-driver-html suture-driver-markdown suture-driver-svg suture-driver-properties suture-driver-docx suture-driver-xlsx suture-driver-pptx suture-driver-pdf suture-driver-image suture-driver-feed suture-driver-ical suture-driver-otio suture-driver-example suture-hub suture-daemon suture-cli suture-tui suture-lsp suture-merge suture-raft suture-vfs suture-s3 suture-wasm-plugin suture-bench suture-platform; do
  cargo publish -p $crate 2>/dev/null || echo "SKIP: $crate"
done
```

---

## Section 8: npm Publishing

```bash
# Login (one-time)
npm login

# Publish from packages/ directory
cd packages/npm
npm publish
```

---

## Section 9: PyPI Publishing

```bash
# Install build tools
pip install build twine

# Build from bindings/ directory
cd bindings/python
python -m build

# Upload
twine upload dist/*
```

---

## Section 10: Git Tag and GitHub Release

```bash
# Create and push the tag
git tag -a v5.1.0 -m "Suture 5.1.0: Semantic Merge for Every Format"
git push origin v5.1.0

# The release workflow (.github/workflows/release.yml) will:
# - Build binaries for Linux (x86_64), macOS (x86_64 + aarch64), Windows (x86_64)
# - Generate SHA256 checksums
# - Create a GitHub Release draft

# Go to https://github.com/WyattAu/suture/releases
# Edit the draft release:
#   - Title: "Suture 5.1.0"
#   - Copy release notes from CHANGELOG.md [5.1.0] section
#   - Click "Publish release"
```

---

## Section 11: Launch Checklist

### Infrastructure

- [ ] Platform deployed and accessible at `https://suture-platform.fly.dev/healthz`
- [ ] Custom domain `suture.dev` pointing to platform
- [ ] SSL certificate active (`fly certs list` shows "Ready")
- [ ] Stripe products configured (Pro + Enterprise)
- [ ] Webhook endpoint receiving events (verify with Stripe CLI)
- [ ] Test registration → login → upgrade → portal flow end-to-end

### Package Registries

- [ ] GitHub Action published to Marketplace
- [ ] VS Code extension published to Marketplace
- [ ] All 37 crates published to crates.io
- [ ] npm v5.1.0 published
- [ ] PyPI v5.1.0 published

### Git

- [ ] `v5.1.0` git tag pushed to origin
- [ ] GitHub Release created and published
- [ ] Release notes include CHANGELOG entries

### Marketing

- [ ] Blog post published (`docs/blog/launch-v5.1.0.md`)
- [ ] Landing page updated with v5.1.0 features
- [ ] Hacker News post submitted
- [ ] Reddit post submitted (r/rust, r/programming)
- [ ] Twitter/X announcement posted
