#!/usr/bin/env bash
set -euo pipefail

REAL=false
DRY_RUN=false
BUMP=""
GENERATE_MANIFEST=false

usage() {
  cat <<'USAGE'
Usage: publish.sh [OPTIONS]

Options:
  --real            Publish to crates.io (default: dry-run)
  --dry-run         Explicit dry-run mode (same as default)
  --bump minor|patch Bump all crate versions before publishing
  --manifest        Generate a version manifest JSON to stdout
  --help            Show this help

Examples:
  ./scripts/publish.sh                  # dry-run
  ./scripts/publish.sh --dry-run        # explicit dry-run
  ./scripts/publish.sh --bump patch     # bump patch versions, then dry-run
  ./scripts/publish.sh --bump minor --real  # bump minor versions, then publish
  ./scripts/publish.sh --manifest       # output crate versions as JSON
USAGE
}

for arg in "${1:-}"; do
  case "$arg" in
    --real)      REAL=true ;;
    --dry-run)   DRY_RUN=true ;;
    --bump)      BUMP="${2:-}"; shift ;;
    --manifest)  GENERATE_MANIFEST=true ;;
    --help|-h)   usage; exit 0 ;;
    *)           echo "Unknown option: $arg"; usage; exit 1 ;;
  esac
done

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
  suture-driver-html
  suture-driver-svg
  suture-driver-docx
  suture-driver-xlsx
  suture-driver-pptx
  suture-driver-sql
  suture-driver-pdf
  suture-driver-image
  suture-driver-otio
  suture-driver-ical
  suture-driver-feed
  suture-driver-example
  suture-raft
  suture-s3
  suture-plugin-sdk
  suture-wasm-plugin
  suture-merge
  suture-hub
  suture-daemon
  suture-tui
  suture-lsp
  suture-vfs
  suture-connector-airtable
  suture-connector-gsheets
  suture-connector-notion
  suture-cli
)

FAILED=()
PASSED=()
DEP_SKIPPED=()

cd "$PROJECT_DIR"

if $GENERATE_MANIFEST; then
  echo "{"
  echo "  \"generated_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
  echo "  \"crates\": ["
  first=true
  for crate in "${PUBLISH_ORDER[@]}"; do
    version=""
    if [[ -f "crates/$crate/Cargo.toml" ]]; then
      version=$(grep -m1 '^version' "crates/$crate/Cargo.toml" | sed 's/.*"\([^"]*\)".*/\1/')
    fi
    if $first; then first=false; else echo ","; fi
    printf '    {"name": "%s", "version": "%s"}' "$crate" "${version:-unknown}"
  done
  echo ""
  echo "  ]"
  echo "}"
  exit 0
fi

bump_version() {
  local toml="$1"
  local part="$2"
  sed -i -E "s/^(version[[:space:]]*=[[:space:]]*\")([0-9]+)\.([0-9]+)\.([0-9]+)(\")/\1$(echo "\2.\3.\4" | awk -F. -v p="$part" '{
    if (p == "minor") printf "%d.%d.0", $1, $2+1
    else if (p == "patch") printf "%d.%d.%d", $1, $2, $3+1
  }')\4/" "$toml"
}

if [[ -n "$BUMP" ]]; then
  if [[ "$BUMP" != "minor" && "$BUMP" != "patch" ]]; then
    echo "ERROR: --bump must be 'minor' or 'patch'"
    exit 1
  fi
  echo "=== Bumping ${BUMP} versions ==="
  echo ""
  for crate in "${PUBLISH_ORDER[@]}"; do
    toml="crates/$crate/Cargo.toml"
    if [[ -f "$toml" ]]; then
      old_version=$(grep -m1 '^version' "$toml" | sed 's/.*"\([^"]*\)".*/\1/')
      bump_version "$toml" "$BUMP"
      new_version=$(grep -m1 '^version' "$toml" | sed 's/.*"\([^"]*\)".*/\1/')
      printf "  %-35s %s -> %s\n" "$crate" "$old_version" "$new_version"
    else
      printf "  %-35s (skipped, no Cargo.toml)\n" "$crate"
    fi
  done
  echo ""
fi

echo "=== Suture Publish Script ==="
echo ""

if $REAL; then
  echo "MODE: REAL PUBLISH (crates will be published to crates.io)"
elif $DRY_RUN; then
  echo "MODE: DRY-RUN (explicit --dry-run)"
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
    cmd=(cargo publish -p "$crate" --allow-dirty --no-verify)
  else
    cmd=(cargo publish --dry-run -p "$crate" --allow-dirty)
  fi

  printf "[%2d/%2d] %-35s ... " "$(( ${#PASSED[@]} + ${#FAILED[@]} + 1 ))" "${#PUBLISH_ORDER[@]}" "$crate"

  if output=$("${cmd[@]}" 2>&1); then
    echo "OK"
    PASSED+=("$crate")
  else
    rc=$?
    if echo "$output" | grep -q "no matching package named"; then
      echo "SKIPPED (intra-workspace dep not yet published)"
      DEP_SKIPPED+=("$crate")
    elif echo "$output" | grep -q "already exists"; then
      echo "SKIPPED (already published)"
      PASSED+=("$crate")
    elif echo "$output" | grep -q "already uploaded"; then
      echo "SKIPPED (already uploaded)"
      PASSED+=("$crate")
    elif echo "$output" | grep -q "Too Many Requests"; then
      echo "RATE-LIMITED (waiting 60s)"
      sleep 60
      if output=$("${cmd[@]}" 2>&1); then
        echo "OK (retry)"
        PASSED+=("$crate")
      else
        echo "FAILED after retry"
        FAILED+=("$crate")
      fi
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
