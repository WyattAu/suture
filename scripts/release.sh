#!/usr/bin/env bash
set -euo pipefail

VERSION="2.9.0"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Suture v${VERSION} Release Script ==="

echo "Running quality gates..."
cd "$PROJECT_DIR"

echo "  cargo check..."
cargo check --workspace --exclude suture-py

echo "  cargo test..."
cargo test --workspace --exclude suture-py -- --test-threads=4

echo "  cargo clippy..."
cargo clippy --workspace --exclude suture-py -- -D warnings

echo "  cargo fmt..."
cargo fmt --check --all

echo ""
echo "=== All quality gates passed ==="

echo ""
echo "Creating git tag v${VERSION}..."
git tag -a "v${VERSION}" -m "Release v${VERSION}"

echo ""
echo "=== Ready to ship ==="
echo ""
echo "Next steps:"
echo "  git push origin main --tags"
echo "  (GitHub Actions will build binaries automatically)"
echo ""
echo "For crates.io:"
echo "  cargo login"
echo "  Follow packaging/PUBLISH.md"
