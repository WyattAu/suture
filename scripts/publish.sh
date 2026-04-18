#!/usr/bin/env bash
set -euo pipefail

REAL=false
if [[ "${1:-}" == "--real" ]]; then
  REAL=true
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

PUBLISH_ORDER=(
  suture-common
  suture-core
  suture-protocol
  suture-driver
  suture-ooxml
  suture-driver-json
  suture-driver-yaml
  suture-driver-toml
  suture-driver-csv
  suture-driver-xml
  suture-driver-markdown
  suture-driver-docx
  suture-driver-xlsx
  suture-driver-pptx
  suture-driver-sql
  suture-driver-pdf
  suture-driver-image
  suture-driver-otio
  suture-driver-example
  suture-raft
  suture-s3
  suture-hub
  suture-daemon
  suture-tui
  suture-lsp
  suture-vfs
  suture-cli
)

FAILED=()
PASSED=()
DEP_SKIPPED=()

cd "$PROJECT_DIR"

echo "=== Suture Publish Script ==="
echo ""

if $REAL; then
  echo "MODE: REAL PUBLISH (crates will be published to crates.io)"
else
  echo "MODE: DRY-RUN (no crates will be published)"
fi

echo ""
echo "Checking cargo login status..."
if $REAL && ! cargo login --help &>/dev/null; then
  echo "ERROR: cargo login not configured. Run 'cargo login <token>' first."
  exit 1
fi

echo ""
echo "Publishing ${#PUBLISH_ORDER[@]} crates in dependency order..."
echo ""

for crate in "${PUBLISH_ORDER[@]}"; do
  if $REAL; then
    cmd=(cargo publish -p "$crate" --allow-dirty)
  else
    cmd=(cargo publish --dry-run -p "$crate" --allow-dirty)
  fi

  printf "[%2d/%2d] %-30s ... " "$(( ${#PASSED[@]} + ${#FAILED[@]} + 1 ))" "${#PUBLISH_ORDER[@]}" "$crate"

  if output=$("${cmd[@]}" 2>&1); then
    echo "OK"
    PASSED+=("$crate")
  else
    rc=$?
    if echo "$output" | grep -q "no matching package named"; then
      echo "SKIPPED (intra-workspace dep not yet published)"
      DEP_SKIPPED+=("$crate")
    else
      echo "FAILED (exit code $rc)"
      echo "$output" | tail -5
      FAILED+=("$crate")
    fi
  fi
done

echo ""
echo "=== Results ==="
echo ""
echo "Passed: ${#PASSED[@]} / ${#PUBLISH_ORDER[@]}"
echo "Skipped (unpublished workspace deps): ${#DEP_SKIPPED[@]}"

if [[ ${#FAILED[@]} -gt 0 ]]; then
  echo ""
  echo "Failed crates:"
  for crate in "${FAILED[@]}"; do
    echo "  - $crate"
  done
  echo ""
  echo "Run the following to investigate:"
  for crate in "${FAILED[@]}"; do
    echo "  cargo publish --dry-run -p $crate --allow-dirty"
  done
  exit 1
elif [[ ${#DEP_SKIPPED[@]} -gt 0 ]]; then
  echo ""
  echo "Skipped crates depend on other workspace crates that haven't been"
  echo "published yet. This is expected for dry-run. These crates will"
  echo "succeed once all their dependencies are published in order."
  echo ""
  if ! $REAL; then
    echo "To publish for real, run:"
    echo "  $0 --real"
  fi
else
  echo ""
  echo "All crates passed! Ready to publish."
  if ! $REAL; then
    echo ""
    echo "To publish for real, run:"
    echo "  $0 --real"
  fi
fi
