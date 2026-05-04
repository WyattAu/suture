#!/usr/bin/env bash
set -euo pipefail

echo "=== Suture Platform Deployment ==="
echo ""

# Check prerequisites
command -v fly >/dev/null 2>&1 || { echo "ERROR: fly CLI not installed. Install from https://fly.io/docs/hands-on/installing/"; exit 1; }

# Login if needed
if ! fly auth whoami >/dev/null 2>&1; then
    echo "Logging into Fly.io..."
    fly auth login
fi

# Launch the app (creates if not exists)
echo "Launching app on Fly.io..."
fly launch --no-deploy --region iad

# Set secrets
echo ""
echo "Setting secrets..."
read -p "Enter JWT secret (min 32 chars, press Enter to generate): " JWT_SECRET
if [ -z "$JWT_SECRET" ]; then
    JWT_SECRET=$(openssl rand -hex 32)
    echo "Generated JWT secret: $JWT_SECRET"
fi
fly secrets set JWT_SECRET="$JWT_SECRET"

read -p "Enter Stripe secret key (or press Enter to skip): " STRIPE_KEY
if [ -n "$STRIPE_KEY" ]; then
    fly secrets set STRIPE_KEY="$STRIPE_KEY"
    
    read -p "Enter Stripe webhook secret (or press Enter to skip): " STRIPE_WEBHOOK_SECRET
    if [ -n "$STRIPE_WEBHOOK_SECRET" ]; then
        fly secrets set STRIPE_WEBHOOK_SECRET="$STRIPE_WEBHOOK_SECRET"
    fi
    
    read -p "Enter Stripe Pro price ID (or press Enter to skip): " STRIPE_PRO_PRICE_ID
    if [ -n "$STRIPE_PRO_PRICE_ID" ]; then
        fly secrets set STRIPE_PRO_PRICE_ID="$STRIPE_PRO_PRICE_ID"
    fi
    
    read -p "Enter Stripe Enterprise price ID (or press Enter to skip): " STRIPE_ENTERPRISE_PRICE_ID
    if [ -n "$STRIPE_ENTERPRISE_PRICE_ID" ]; then
        fly secrets set STRIPE_ENTERPRISE_PRICE_ID="$STRIPE_ENTERPRISE_PRICE_ID"
    fi
fi

read -p "Enter Google OAuth client ID (or press Enter to skip): " GOOGLE_CLIENT_ID
if [ -n "$GOOGLE_CLIENT_ID" ]; then
    fly secrets set GOOGLE_CLIENT_ID="$GOOGLE_CLIENT_ID"
    
    read -p "Enter Google OAuth client secret: " GOOGLE_CLIENT_SECRET
    fly secrets set GOOGLE_CLIENT_SECRET="$GOOGLE_CLIENT_SECRET"
fi

read -p "Enter GitHub OAuth client ID (or press Enter to skip): " GITHUB_CLIENT_ID
if [ -n "$GITHUB_CLIENT_ID" ]; then
    fly secrets set GITHUB_CLIENT_ID="$GITHUB_CLIENT_ID"
    
    read -p "Enter GitHub OAuth client secret: " GITHUB_CLIENT_SECRET
    fly secrets set GITHUB_CLIENT_SECRET="$GITHUB_CLIENT_SECRET"
fi

echo ""
echo "Deploying..."
fly deploy

echo ""
echo "=== Deployment Complete ==="
echo "App URL: https://suture-platform.fly.dev"
echo "Machine: $(fly machines list --json 2>/dev/null | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d[0].get("region","unknown"))' 2>/dev/null || echo 'unknown')"
echo ""
echo "Next steps:"
echo "  1. Configure Stripe products at https://dashboard.stripe.com"
echo "  2. Set webhook URL to https://suture-platform.fly.dev/billing/webhook"
echo "  3. Run: fly scale memory 1024 --app suture-platform  (for production)"
