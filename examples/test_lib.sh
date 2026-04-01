#!/usr/bin/env bash
# Shared test library for suture examples
# Source this file in each test script: source "$(dirname "$0")/test_lib.sh"

set -euo pipefail

# --- Configuration ---
SUTURE_BIN="${SUTURE_BIN:-}"
if [[ -z "$SUTURE_BIN" ]]; then
    # Find the binary relative to this script
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
    SUTURE_BIN="$REPO_ROOT/target/debug/suture"
fi

if [[ ! -x "$SUTURE_BIN" ]]; then
    echo "FAIL: suture binary not found at $SUTURE_BIN (run 'cargo build -p suture-cli' first)"
    exit 1
fi

# --- Test state ---
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0
FAILURES=()
CURRENT_TMPDIR=""

# --- Cleanup ---
cleanup() {
    if [[ -n "$CURRENT_TMPDIR" && -d "$CURRENT_TMPDIR" ]]; then
        rm -rf "$CURRENT_TMPDIR"
    fi
}
trap cleanup EXIT

# --- Test primitives ---

# Create a fresh temporary repository
setup_repo() {
    CURRENT_TMPDIR=$(mktemp -d)
    cd "$CURRENT_TMPDIR"
    "$SUTURE_BIN" init >/dev/null 2>&1
    "$SUTURE_BIN" config user.name="Test User" >/dev/null 2>&1
    "$SUTURE_BIN" config user.email="test@example.com" >/dev/null 2>&1
}

# Run a suture command, return stdout. Fail on non-zero exit.
suture() {
    TESTS_RUN=$((TESTS_RUN + 1))
    local output
    output=$("$SUTURE_BIN" "$@" 2>&1) && {
        TESTS_PASSED=$((TESTS_PASSED + 1))
        echo "$output"
        return 0
    } || {
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILURES+=("suture $* → exit $?, output: $(echo "$output" | head -3)")
        return 1
    }
}

# Run a suture command expecting failure. Returns stdout+stderr.
suture_fail() {
    TESTS_RUN=$((TESTS_RUN + 1))
    local output
    output=$("$SUTURE_BIN" "$@" 2>&1) && {
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILURES+=("suture $* → expected failure but succeeded")
        return 1
    } || {
        TESTS_PASSED=$((TESTS_PASSED + 1))
        echo "$output"
        return 0
    }
}

# Assert two strings are equal
assert_eq() {
    TESTS_RUN=$((TESTS_RUN + 1))
    local desc="${3:-assert_eq}"
    if [[ "$1" == "$2" ]]; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILURES+=("$desc: expected '$2', got '$1'")
    fi
}

# Assert output contains a substring
assert_contains() {
    TESTS_RUN=$((TESTS_RUN + 1))
    local desc="${3:-assert_contains}"
    if echo "$1" | grep -q "$2"; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILURES+=("$desc: expected output to contain '$2', got: $(echo "$1" | head -3)")
    fi
}

# Assert output does NOT contain a substring
assert_not_contains() {
    TESTS_RUN=$((TESTS_RUN + 1))
    local desc="${3:-assert_not_contains}"
    if ! echo "$1" | grep -q "$2"; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILURES+=("$desc: expected output NOT to contain '$2'")
    fi
}

# Assert a file exists
assert_file_exists() {
    TESTS_RUN=$((TESTS_RUN + 1))
    local desc="${2:-assert_file_exists $1}"
    if [[ -f "$1" ]]; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILURES+=("$desc: file '$1' does not exist")
    fi
}

# Assert a file does NOT exist
assert_file_not_exists() {
    TESTS_RUN=$((TESTS_RUN + 1))
    local desc="${2:-assert_file_not_exists $1}"
    if [[ ! -f "$1" ]]; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILURES+=("$desc: file '$1' should not exist")
    fi
}

# Assert file content equals
assert_file_eq() {
    TESTS_RUN=$((TESTS_RUN + 1))
    local desc="${3:-assert_file_eq $1}"
    local actual
    actual=$(cat "$1" 2>/dev/null) || true
    if [[ "$actual" == "$2" ]]; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILURES+=("$desc: expected '$2', got '$actual'")
    fi
}

# --- Reporting ---

report_results() {
    local name="$1"
    echo ""
    echo "============================================"
    echo "  $name"
    echo "============================================"
    echo "  Ran:     $TESTS_RUN assertions"
    echo "  Passed:  $TESTS_PASSED"
    echo "  Failed:  $TESTS_FAILED"
    if [[ ${#FAILURES[@]} -gt 0 ]]; then
        echo ""
        echo "  FAILURES:"
        for f in "${FAILURES[@]}"; do
            echo "    - $f"
        done
    fi
    echo "============================================"
    echo ""

    if [[ $TESTS_FAILED -gt 0 ]]; then
        return 1
    fi
    return 0
}
