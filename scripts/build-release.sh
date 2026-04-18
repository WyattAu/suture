#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_DIR"

echo "=== Suture Release Build ==="
echo ""

echo "Building release binary..."
cargo build --release -p suture-cli

BINARY="$PROJECT_DIR/target/release/suture"

if [ ! -f "$BINARY" ]; then
  echo "ERROR: Binary not found at $BINARY"
  exit 1
fi

echo ""
echo "Binary built successfully: $BINARY"
echo ""

echo "Version check:"
"$BINARY" --version

echo ""
echo "File size: $(du -h "$BINARY" | cut -f1)"
echo ""

CROSS_TARGETS=(
  aarch64-unknown-linux-gnu
  x86_64-apple-darwin
  aarch64-apple-darwin
  x86_64-pc-windows-msvc
)

echo "Checking cross-compilation targets..."
for target in "${CROSS_TARGETS[@]}"; do
  if rustup target list --installed | grep -q "^${target}"; then
    echo "  $target: installed, building..."
    cargo build --release --target "$target" -p suture-cli
    echo "  $target: OK ($(du -h "$PROJECT_DIR/target/$target/release/suture.exe" 2>/dev/null || du -h "$PROJECT_DIR/target/$target/release/suture" | cut -f1))"
  else
    echo "  $target: skipped (not installed)"
  fi
done

echo ""
echo "=== Build complete ==="
