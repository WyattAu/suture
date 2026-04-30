#!/usr/bin/env bash
# Suture CI Merge Script
# Works in GitHub Actions, GitLab CI, CircleCI, Jenkins, or any bash environment
#
# Usage:
#   ./scripts/merge-ci.sh --files "package.json tsconfig.json" --driver json --base-ref HEAD~1
#
# Environment variables:
#   SUTURE_API_URL   (default: https://merge.suture.dev/api)
#   SUTURE_API_TOKEN  (optional, for higher rate limits)

set -euo pipefail

API_URL="${SUTURE_API_URL:-https://merge.suture.dev/api}"
API_TOKEN="${SUTURE_API_TOKEN:-}"
BASE_REF="HEAD~1"
OURS_REF="HEAD"
THEIRS_REF=""
DRIVER=""
FILES=""
FAIL_ON_CONFLICT=true

while [[ $# -gt 0 ]]; do
    case $1 in
        --api-url)  API_URL="$2"; shift 2 ;;
        --token)    API_TOKEN="$2"; shift 2 ;;
        --base-ref) BASE_REF="$2"; shift 2 ;;
        --ours-ref) OURS_REF="$2"; shift 2 ;;
        --theirs-ref) THEIRS_REF="$2"; shift 2 ;;
        --driver)   DRIVER="$2"; shift 2 ;;
        --files)    FILES="$2"; shift 2 ;;
        --no-fail)  FAIL_ON_CONFLICT=false; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [ -z "$FILES" ]; then
    echo "Error: --files is required"
    exit 1
fi

get_driver_for_file() {
    local file="$1"
    local ext="${file##*.}"
    case "$ext" in
        json) echo "json" ;;
        yaml|yml) echo "yaml" ;;
        toml) echo "toml" ;;
        xml) echo "xml" ;;
        csv) echo "csv" ;;
        sql) echo "sql" ;;
        html|htm) echo "html" ;;
        md) echo "markdown" ;;
        svg) echo "svg" ;;
        properties|ini) echo "properties" ;;
        *) echo "" ;;
    esac
}

CONFLICT_FILES=""

for FILE in $FILES; do
    [ -f "$FILE" ] || { echo "SKIP: $FILE not found"; continue; }

    drv="${DRIVER:-$(get_driver_for_file "$FILE")}"
    [ -z "$drv" ] && { echo "SKIP: $FILE (unknown driver)"; continue; }

    base_content=$(git show "$BASE_REF:$FILE" 2>/dev/null || echo "")
    ours_content=$(cat "$FILE")
    theirs_content="$ours_content"
    [ -n "$THEIRS_REF" ] && theirs_content=$(git show "$THEIRS_REF:$FILE" 2>/dev/null || echo "$ours_content")

    echo "Merging $FILE (driver=$drv)..."

    headers=(-H "Content-Type: application/json")
    [ -n "$API_TOKEN" ] && headers+=(-H "Authorization: Bearer $API_TOKEN")

    body=$(jq -n \
        --arg driver "$drv" \
        --arg base "$base_content" \
        --arg ours "$ours_content" \
        --arg theirs "$theirs_content" \
        '{driver: $driver, base: $base, ours: $ours, theirs: $theirs}')

    set +e
    result=$(curl -sf -X POST "$API_URL/merge" "${headers[@]}" -d "$body" 2>/dev/null)
    curl_exit=$?
    set -e

    if [ $curl_exit -ne 0 ]; then
        echo "ERROR: API call failed for $FILE"
        CONFLICT_FILES="$CONFLICT_FILES $FILE"
        continue
    fi

    merged=$(echo "$result" | jq -r '.result // empty')
    if [ -n "$merged" ]; then
        echo "$merged" > "$FILE"
        echo "  OK: $FILE merged successfully"
    else
        echo "  CONFLICT: $FILE has unresolvable conflicts"
        CONFLICT_FILES="$CONFLICT_FILES $FILE"
    fi
done

if [ -n "$CONFLICT_FILES" ]; then
    echo ""
    echo "Conflicts in:$CONFLICT_FILES"
    if [ "$FAIL_ON_CONFLICT" = true ]; then
        exit 1
    fi
fi

echo "Done."
