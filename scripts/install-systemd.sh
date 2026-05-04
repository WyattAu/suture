#!/usr/bin/env bash
set -euo pipefail

SUTURE_USER=${SUTURE_USER:-suture}
SUTURE_HOME=/var/lib/suture
SERVICE_NAME=suture-platform
BINARY_PATH=${BINARY_PATH:-/usr/local/bin/suture-platform}

echo "=== Installing Suture Platform as systemd service ==="

id "$SUTURE_USER" &>/dev/null || useradd -r -s /usr/sbin/nologin -d "$SUTURE_HOME" "$SUTURE_USER"

mkdir -p "$SUTURE_HOME"

if [ -f "target/release/suture-platform" ]; then
    cp target/release/suture-platform "$BINARY_PATH"
fi

sed "s|/usr/local/bin/suture-platform|$BINARY_PATH|g" "$SERVICE_NAME.service" \
    > "/etc/systemd/system/$SERVICE_NAME.service"

systemctl daemon-reload
systemctl enable "$SERVICE_NAME"
systemctl start "$SERVICE_NAME"

echo ""
echo "Suture Platform installed and running."
echo "  Service: systemctl status $SERVICE_NAME"
echo "  Logs:    journalctl -u $SERVICE_NAME -f"
echo "  Config:  /etc/suture/env"
echo ""
echo "Set secrets in /etc/suture/env:"
echo "  JWT_SECRET=your-secret-here"
echo "  STRIPE_KEY=sk_live_..."
