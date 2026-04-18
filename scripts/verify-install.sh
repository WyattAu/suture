#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_DIR="/tmp/test-suture-verify"

cleanup() {
  rm -rf "$TEST_DIR"
}

trap cleanup EXIT

cd "$PROJECT_DIR"

echo "=== Suture Install Verification ==="
echo ""

echo "[1/5] Building from source..."
cargo build --release -p suture-cli
echo "  OK"

echo ""
echo "[2/5] Verifying binary exists..."
if [[ -f target/release/suture ]]; then
  echo "  OK: target/release/suture exists"
else
  echo "  FAIL: target/release/suture not found"
  exit 1
fi

echo ""
echo "[3/5] Running suture --version..."
VERSION_OUTPUT=$(./target/release/suture --version)
echo "  $VERSION_OUTPUT"

echo ""
echo "[4/5] Running suture --help..."
if ./target/release/suture --help &>/dev/null; then
  echo "  OK: help output produced"
else
  echo "  FAIL: suture --help returned non-zero"
  exit 1
fi

echo ""
echo "[5/5] Running suture init and status..."
cleanup 2>/dev/null || true
./target/release/suture init "$TEST_DIR"
if [[ -d "$TEST_DIR/.suture" ]]; then
  echo "  OK: .suture directory created"
else
  echo "  FAIL: .suture directory not created"
  exit 1
fi

if ./target/release/suture -C "$TEST_DIR" status; then
  echo "  OK: status command succeeded"
else
  echo "  FAIL: status command returned non-zero"
  exit 1
fi

echo ""
echo "=== All verifications passed ==="
