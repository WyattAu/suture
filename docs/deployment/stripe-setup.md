# Stripe Setup Guide

## Prerequisites
- Stripe account at https://dashboard.stripe.com/register
- Suture Platform deployed (Fly.io or self-hosted)

## Step 1: Create Products

### Pro Plan ($9/seat/month)
1. Go to https://dashboard.stripe.com/products → "Add product"
2. Name: "Suture Pro"
3. Description: "Professional plan with unlimited private repos, merge API access, and priority support"
4. Pricing: Recurring → Monthly → $9.00
5. Note the **Price ID** (starts with `price_`)

### Enterprise Plan ($29/seat/month)
1. Go to https://dashboard.stripe.com/products → "Add product"
2. Name: "Suture Enterprise"
3. Description: "Enterprise plan with SSO, audit logging, WASM plugins, SLA, and dedicated support"
4. Pricing: Recurring → Monthly → $29.00
5. Note the **Price ID** (starts with `price_`)

## Step 2: Create Webhook Endpoint
1. Go to https://dashboard.stripe.com/webhooks → "Add endpoint"
2. URL: `https://YOUR-DOMAIN/billing/webhook`
3. Events to listen for:
   - `checkout.session.completed`
   - `customer.subscription.created`
   - `customer.subscription.updated`
   - `customer.subscription.deleted`
   - `invoice.payment_failed`
   - `invoice.paid`
4. Note the **Signing secret** (starts with `whsec_`)

## Step 3: Configure Environment Variables
```
STRIPE_KEY=sk_live_...
STRIPE_WEBHOOK_SECRET=whsec_...
STRIPE_PRO_PRICE_ID=price_...
STRIPE_ENTERPRISE_PRICE_ID=price_...
```

## Step 4: OAuth Providers (Optional)
### Google OAuth
1. https://console.cloud.google.com/apis/credentials
2. Create OAuth 2.0 Client ID (Web application)
3. Authorized redirect URI: `https://YOUR-DOMAIN/auth/google/callback`
4. Note Client ID and Client Secret

### GitHub OAuth
1. https://github.com/settings/developers
2. New OAuth App
3. Authorization callback URL: `https://YOUR-DOMAIN/auth/github/callback`
4. Note Client ID and Client Secret
