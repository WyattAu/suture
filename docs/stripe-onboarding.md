# Stripe Onboarding Guide

This guide walks through setting up Stripe billing for the Suture Platform.

## Prerequisites

- A Stripe account (https://stripe.com)
- Stripe API key (from https://dashboard.stripe.com/apikeys)
- The Suture Platform running locally or deployed

## Step 1: Create Products and Prices

In the Stripe Dashboard:

1. Go to **Products** → **Add Product**
2. Create three products:

### Free Tier (no Stripe product needed)
The free tier is handled entirely by the platform code. No Stripe configuration needed.

### Pro Plan
- **Name:** Suture Pro
- **Description:** Unlimited repos, 10,000 merges/month, 10 GB storage, all drivers
- **Pricing:** One-time → switch to **Recurring**
  - **Price:** $9/month
  - **Billing:** Monthly
  - Note the **Price ID** (starts with `price_`)

### Enterprise Plan
- **Name:** Suture Enterprise
- **Description:** Unlimited everything, SSO, audit log, 99.99% SLA
- **Pricing:** One-time → switch to **Recurring**
  - **Price:** $29/month
  - **Billing:** Monthly
  - Note the **Price ID** (starts with `price_`)

## Step 2: Configure Webhooks

1. Go to **Developers** → **Webhooks**
2. Click **Add endpoint**
3. **URL:** `https://your-domain.com/billing/webhook`
4. **Events to listen for:**
   - `checkout.session.completed`
   - `customer.subscription.updated`
   - `customer.subscription.deleted`
   - `invoice.payment_failed`
   - `invoice.payment_succeeded`
5. Note the **Signing secret** (starts with `whsec_`)

## Step 3: Update Platform Configuration

Set these environment variables:

```bash
# Required
SUTURE_STRIPE_KEY=sk_live_...           # From Stripe API keys (live mode)
SUTURE_JWT_SECRET=<random-secret>       # Generate with: openssl rand -hex 32

# Optional (for webhook signature verification)
STRIPE_WEBHOOK_SECRET=whsec_...        # From Stripe webhooks
STRIPE_WEBHOOK_ID=we_...               # From Stripe webhooks

# For local testing, use test mode:
SUTURE_STRIPE_KEY=sk_test_...
```

## Step 4: Update Price IDs in Code

Edit `crates/suture-platform/src/stripe.rs` and replace the placeholder price IDs:

```rust
// Before:
const PRICE_PRO_MONTHLY: &str = "price_placeholder_pro";
const PRICE_ENTERPRISE_MONTHLY: &str = "price_placeholder_enterprise";

// After (replace with your actual price IDs):
const PRICE_PRO_MONTHLY: &str = "price_1ABC...";        // Your Pro price ID
const PRICE_ENTERPRISE_MONTHLY: &str = "price_1XYZ..."; // Your Enterprise price ID
```

## Step 5: Test the Checkout Flow

1. Start the platform: `suture-platform --stripe-key sk_test_...`
2. Register a user account
3. Call the checkout endpoint:

```bash
curl -X POST http://localhost:8080/billing/checkout \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"tier": "pro"}'
```

4. You'll get a Stripe Checkout URL. Open it in a browser.
5. Use Stripe test card `4242 4242 4242 4242` with any future date/CVC.
6. After completing checkout, the webhook fires and the user's tier updates to "pro".

## Step 6: Verify Tier Upgrade

```bash
# Check subscription
curl http://localhost:8080/billing/subscription \
  -H "Authorization: Bearer $TOKEN"

# Check usage (should now show pro limits)
curl http://localhost:8080/api/usage \
  -H "Authorization: Bearer $TOKEN"
```

## Step 7: Webhook Signature Verification (Production)

For production, verify webhook signatures to prevent spoofing. Add to
`crates/suture-platform/src/stripe.rs` in the `handle_webhook` function:

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

fn verify_webhook_signature(body: &str, signature: &str, secret: &str) -> bool {
    let timestamp = signature.split('.').nth(1).unwrap_or("");
    let signed_payload = format!("{}.{}", timestamp, body);
    let expected = hmac_sha256(secret.as_bytes(), signed_payload.as_bytes());
    let actual = signature.split('=').nth(1).unwrap_or("");
    constant_time_compare(expected, actual)
}
```

## Testing with Stripe CLI

```bash
# Install Stripe CLI
brew install stripe/stripe-cli/stripe

# Login
stripe login

# Forward webhooks to localhost
stripe listen --forward-to localhost:8080/billing/webhook

# Trigger test events
stripe trigger checkout.session.completed
stripe trigger customer.subscription.updated
stripe trigger customer.subscription.deleted
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| "billing is not configured" | Set `SUTURE_STRIPE_KEY` env var |
| Checkout returns invalid URL | Check price IDs are correct |
| Webhook returns "invalid payload" | Check webhook secret matches |
| Tier not updating after payment | Check webhook is firing (Stripe Dashboard → Logs) |
| Rate limit exceeded on Stripe | Use test mode keys for development |
