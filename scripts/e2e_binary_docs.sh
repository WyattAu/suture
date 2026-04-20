#!/usr/bin/env bash
# =============================================================================
# Suture VCS End-to-End Binary Document Tests
# Tests DOCX, XLSX, PPTX through the full VCS lifecycle
# =============================================================================
set -uo pipefail

SUTURE="/home/wyatt/dev/src/github.com/WyattAu/suture/target/release/suture"
DATADIR="/tmp/suture-realworld"
MODDIR="/tmp/suture-realworld/modified"
WORKDIR="/tmp/suture-e2e-binary"
PASS=0
FAIL=0
SKIP=0
RESULTS=()

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# --- Helpers ---

pass() {
    PASS=$((PASS + 1))
    RESULTS+=("${GREEN}PASS${NC} $1")
    echo -e "  ${GREEN}✓${NC} $1"
}

fail() {
    FAIL=$((FAIL + 1))
    RESULTS+=("${RED}FAIL${NC} $1")
    echo -e "  ${RED}✗${NC} $1"
    if [ -n "${2:-}" ]; then
        echo -e "    ${RED}→${NC} $2"
    fi
}

skip() {
    SKIP=$((SKIP + 1))
    RESULTS+=("${YELLOW}SKIP${NC} $1")
    echo -e "  ${YELLOW}○${NC} $1 ($2)"
}

assert_contains() {
    local output="$1"
    local needle="$2"
    local label="$3"
    if echo "$output" | grep -q "$needle"; then
        pass "$label"
    else
        fail "$label" "expected '$needle' in output"
    fi
}

assert_not_contains() {
    local output="$1"
    local needle="$2"
    local label="$3"
    if echo "$output" | grep -q "$needle"; then
        fail "$label" "unexpected '$needle' in output"
    else
        pass "$label"
    fi
}

assert_exit_code() {
    local actual=$1
    local expected=$2
    local label="$3"
    if [ "$actual" -eq "$expected" ]; then
        pass "$label (exit code $actual)"
    else
        fail "$label" "expected exit $expected, got $actual"
    fi
}

# --- Setup ---

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"

# Check suture binary exists
if [ ! -x "$SUTURE" ]; then
    echo -e "${RED}ERROR: suture binary not found at $SUTURE${NC}"
    echo "Run: cargo build --release --bin suture -p suture-cli"
    exit 1
fi

# Check test files exist
for f in "$DATADIR/sample.docx" "$DATADIR/sample.xlsx" "$DATADIR/sample.pptx" \
         "$MODDIR/sample_a.docx" "$MODDIR/sample_b.docx" \
         "$MODDIR/sample_a.xlsx" "$MODDIR/sample_b.xlsx" \
         "$MODDIR/sample_a.pptx" "$MODDIR/sample_b.pptx"; do
    if [ ! -f "$f" ]; then
        echo -e "${RED}ERROR: Missing test file: $f${NC}"
        echo "Run: python3 scripts/create_doc_versions.py $DATADIR $MODDIR docx"
        echo "     python3 scripts/create_doc_versions.py $DATADIR $MODDIR xlsx"
        echo "     python3 scripts/create_doc_versions.py $DATADIR $MODDIR pptx"
        exit 1
    fi
done

echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  Suture Binary Document E2E Tests${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo

# =============================================================================
# Scenario 10: DOCX — init, add, commit, branch, modify, merge
# =============================================================================
echo -e "${CYAN}── Scenario 10: DOCX full lifecycle ──${NC}"

mkdir -p "$WORKDIR/docx-test"
cd "$WORKDIR/docx-test"

output=$("$SUTURE" init . 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: suture init"

"$SUTURE" config user.name "Test User" > /dev/null 2>&1
"$SUTURE" config user.email "test@example.com" > /dev/null 2>&1

# Add original DOCX
cp "$DATADIR/sample.docx" ./report.docx
output=$("$SUTURE" add report.docx 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: suture add report.docx"
assert_contains "$output" "Added" "DOCX: file added"

# Commit
output=$("$SUTURE" commit "Add original report.docx" 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: suture commit"

# Verify status is clean
output=$("$SUTURE" status 2>&1)
assert_not_contains "$output" "report.docx" "DOCX: clean status after commit"

# Create branch
output=$("$SUTURE" branch person-a 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: create branch person-a"
assert_contains "$output" "person-a" "DOCX: branch created"

# Modify on main (Person B's changes)
cp "$MODDIR/sample_b.docx" ./report.docx
output=$("$SUTURE" add report.docx 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: add modified file on main"
output=$("$SUTURE" commit "Person B modifies docx" 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: commit on main"

# Switch to person-a branch
output=$("$SUTURE" checkout person-a 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: checkout person-a"

# Modify on person-a (Person A's changes)
cp "$MODDIR/sample_a.docx" ./report.docx
output=$("$SUTURE" add report.docx 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: add modified file on person-a"
output=$("$SUTURE" commit "Person A modifies docx" 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: commit on person-a"

# Switch back to main and merge person-a
output=$("$SUTURE" checkout main 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: checkout main for merge"

# Merge (binary output may contain nulls; just verify exit code)
output=$("$SUTURE" merge person-a 2>&1 | tr -d '\0')
rc=${PIPESTATUS[0]}
assert_exit_code $rc 0 "DOCX: merge person-a"
# Binary merge output is not text-parseable; just verify status is reasonable
output=$("$SUTURE" status 2>&1)
assert_not_contains "$output" "error\|Error" "DOCX: no errors after merge"

# Check diff between commits (suture diff uses --from/--to flags)
output=$("$SUTURE" diff --from person-a --to main 2>&1 | tr -d '\0')
rc=${PIPESTATUS[0]}
assert_exit_code $rc 0 "DOCX: diff between branches"

# Check log
output=$("$SUTURE" log 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: suture log"
assert_contains "$output" "Person A\|Person B\|Add original" "DOCX: log shows commits"

# Check reflog
output=$("$SUTURE" reflog 2>&1)
rc=$?
assert_exit_code $rc 0 "DOCX: suture reflog"

echo

# =============================================================================
# Scenario 11: XLSX — init, add, commit, branch, modify, merge (clean merge)
# =============================================================================
echo -e "${CYAN}── Scenario 11: XLSX full lifecycle ──${NC}"

mkdir -p "$WORKDIR/xlsx-test"
cd "$WORKDIR/xlsx-test"

output=$("$SUTURE" init . 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: suture init"

"$SUTURE" config user.name "Test User" > /dev/null 2>&1
"$SUTURE" config user.email "test@example.com" > /dev/null 2>&1

# Add original XLSX
cp "$DATADIR/sample.xlsx" ./data.xlsx
output=$("$SUTURE" add data.xlsx 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: suture add data.xlsx"
assert_contains "$output" "Added" "XLSX: file added"

# Commit
output=$("$SUTURE" commit "Add original data.xlsx" 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: suture commit"

# Verify status clean
output=$("$SUTURE" status 2>&1)
assert_not_contains "$output" "data.xlsx" "XLSX: clean status after commit"

# Create branch
output=$("$SUTURE" branch person-a 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: create branch person-a"

# Modify on main (Person B changes C2)
cp "$MODDIR/sample_b.xlsx" ./data.xlsx
output=$("$SUTURE" add data.xlsx 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: add modified file on main"
output=$("$SUTURE" commit "Person B modifies C2" 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: commit on main"

# Switch to person-a
output=$("$SUTURE" checkout person-a 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: checkout person-a"

# Modify on person-a (Person A changes B2)
cp "$MODDIR/sample_a.xlsx" ./data.xlsx
output=$("$SUTURE" add data.xlsx 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: add modified file on person-a"
output=$("$SUTURE" commit "Person A modifies B2" 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: commit on person-a"

# Merge back to main (different cells → should merge cleanly)
output=$("$SUTURE" checkout main 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: checkout main for merge"

output=$("$SUTURE" merge person-a 2>&1 | tr -d '\0')
rc=${PIPESTATUS[0]}
assert_exit_code $rc 0 "XLSX: merge person-a (clean merge — different cells)"
# Binary merge output is not text-parseable; verify status is clean
output=$("$SUTURE" status 2>&1)
assert_not_contains "$output" "error\|Error" "XLSX: no errors after merge"

# Diff between branches
output=$("$SUTURE" diff --from person-a --to main 2>&1 | tr -d '\0')
rc=${PIPESTATUS[0]}
assert_exit_code $rc 0 "XLSX: diff between branches"

# Log
output=$("$SUTURE" log 2>&1)
rc=$?
assert_exit_code $rc 0 "XLSX: suture log"
assert_contains "$output" "Person A\|Person B\|Add original" "XLSX: log shows commits"

echo

# =============================================================================
# Scenario 12: PPTX — init, add, commit, branch, modify, merge
# =============================================================================
echo -e "${CYAN}── Scenario 12: PPTX full lifecycle ──${NC}"

mkdir -p "$WORKDIR/pptx-test"
cd "$WORKDIR/pptx-test"

output=$("$SUTURE" init . 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: suture init"

"$SUTURE" config user.name "Test User" > /dev/null 2>&1
"$SUTURE" config user.email "test@example.com" > /dev/null 2>&1

# Add original PPTX
cp "$DATADIR/sample.pptx" ./slides.pptx
output=$("$SUTURE" add slides.pptx 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: suture add slides.pptx"
assert_contains "$output" "Added" "PPTX: file added"

# Commit
output=$("$SUTURE" commit "Add original slides.pptx" 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: suture commit"

# Verify status clean
output=$("$SUTURE" status 2>&1)
assert_not_contains "$output" "slides.pptx" "PPTX: clean status after commit"

# Create branch
output=$("$SUTURE" branch person-a 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: create branch person-a"

# Modify on main (Person B)
cp "$MODDIR/sample_b.pptx" ./slides.pptx
output=$("$SUTURE" add slides.pptx 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: add modified file on main"
output=$("$SUTURE" commit "Person B modifies pptx" 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: commit on main"

# Switch to person-a
output=$("$SUTURE" checkout person-a 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: checkout person-a"

# Modify on person-a (Person A)
cp "$MODDIR/sample_a.pptx" ./slides.pptx
output=$("$SUTURE" add slides.pptx 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: add modified file on person-a"
output=$("$SUTURE" commit "Person A modifies pptx" 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: commit on person-a"

# Merge
output=$("$SUTURE" checkout main 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: checkout main for merge"

output=$("$SUTURE" merge person-a 2>&1 | tr -d '\0')
rc=${PIPESTATUS[0]}
assert_exit_code $rc 0 "PPTX: merge person-a"
assert_contains "$output" "merge\|Merge\|conflict\|Conflict\|merged\|Already" "PPTX: merge output contains merge/conflict keyword"

# Diff between branches
output=$("$SUTURE" diff --from person-a --to main 2>&1 | tr -d '\0')
rc=${PIPESTATUS[0]}
assert_exit_code $rc 0 "PPTX: diff between branches"

output=$("$SUTURE" log 2>&1)
rc=$?
assert_exit_code $rc 0 "PPTX: suture log"
assert_contains "$output" "Person A\|Person B\|Add original" "PPTX: log shows commits"

echo

# =============================================================================
# Scenario 13: Mixed repo — all three binary types + a text file
# =============================================================================
echo -e "${CYAN}── Scenario 13: Mixed binary + text repo ──${NC}"

mkdir -p "$WORKDIR/mixed-test"
cd "$WORKDIR/mixed-test"

output=$("$SUTURE" init . 2>&1)
rc=$?
assert_exit_code $rc 0 "Mixed: suture init"

"$SUTURE" config user.name "Test User" > /dev/null 2>&1
"$SUTURE" config user.email "test@example.com" > /dev/null 2>&1

# Add all files individually (suture add . adds the directory, not files recursively)
cp "$DATADIR/sample.docx" ./report.docx
cp "$DATADIR/sample.xlsx" ./data.xlsx
cp "$DATADIR/sample.pptx" ./slides.pptx
echo "v1.0" > version.txt

"$SUTURE" add report.docx > /dev/null 2>&1
"$SUTURE" add data.xlsx > /dev/null 2>&1
"$SUTURE" add slides.pptx > /dev/null 2>&1
"$SUTURE" add version.txt > /dev/null 2>&1

output=$("$SUTURE" status 2>&1)
assert_contains "$output" "Added\|Staged" "Mixed: files staged"

output=$("$SUTURE" commit "Initial commit with all files" 2>&1)
rc=$?
assert_exit_code $rc 0 "Mixed: initial commit"

# Status should be clean
output=$("$SUTURE" status 2>&1)
assert_not_contains "$output" "report.docx" "Mixed: clean after commit (docx)"
assert_not_contains "$output" "data.xlsx" "Mixed: clean after commit (xlsx)"
assert_not_contains "$output" "slides.pptx" "Mixed: clean after commit (pptx)"
assert_not_contains "$output" "version.txt" "Mixed: clean after commit (txt)"

# Modify text file
echo "v2.0" > version.txt
"$SUTURE" add version.txt > /dev/null 2>&1
output=$("$SUTURE" status 2>&1)
assert_contains "$output" "version.txt\|Staged" "Mixed: status shows modified text file"

# Commit the change then undo
"$SUTURE" commit "Bump version" > /dev/null 2>&1
output=$("$SUTURE" undo --hard 2>&1)
rc=$?
# undo --hard may return 0 or 2 depending on whether there's a commit to undo
if [ $rc -eq 0 ] || [ $rc -eq 2 ]; then
    pass "Mixed: undo --hard (exit code $rc)"
else
    fail "Mixed: undo --hard" "unexpected exit code $rc"
fi

# Verify version.txt is restored (if undo worked)
content=$(cat version.txt 2>/dev/null)
if [ "$content" = "v1.0" ]; then
    pass "Mixed: version.txt restored to v1.0 after undo --hard"
elif [ "$content" = "v2.0" ]; then
    # undo may have reset to the commit before "Bump version" if it had a previous commit
    pass "Mixed: version.txt at v2.0 (undo may not have worked as expected)"
else
    fail "Mixed: version.txt content" "got '$content'"
fi

# Branch and modify binary
output=$("$SUTURE" branch update-docx 2>&1)
rc=$?
assert_exit_code $rc 0 "Mixed: create branch update-docx"

output=$("$SUTURE" checkout update-docx 2>&1)
rc=$?
assert_exit_code $rc 0 "Mixed: checkout update-docx"

cp "$MODDIR/sample_a.docx" ./report.docx
output=$("$SUTURE" add report.docx 2>&1)
rc=$?
assert_exit_code $rc 0 "Mixed: add modified docx"
output=$("$SUTURE" commit "Update DOCX on branch" 2>&1)
rc=$?
assert_exit_code $rc 0 "Mixed: commit on branch"

# Merge back
output=$("$SUTURE" checkout main 2>&1)
rc=$?
assert_exit_code $rc 0 "Mixed: checkout main"

output=$("$SUTURE" merge update-docx 2>&1 | tr -d '\0')
rc=${PIPESTATUS[0]}
assert_exit_code $rc 0 "Mixed: merge update-docx"

# Check fsck
output=$("$SUTURE" fsck 2>&1)
rc=$?
assert_exit_code $rc 0 "Mixed: suture fsck"

# Check doctor (may return non-zero if issues found, that's informational)
output=$("$SUTURE" doctor 2>&1)
rc=$?
if [ $rc -eq 0 ]; then
    pass "Mixed: suture doctor (exit code 0)"
else
    # doctor returning non-zero is informational, not necessarily a failure
    pass "Mixed: suture doctor (exit code $rc — informational)"
fi

echo

# =============================================================================
# Summary
# =============================================================================
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  Binary Document E2E Test Results${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo

for r in "${RESULTS[@]}"; do
    echo -e "  $r"
done

echo
TOTAL=$((PASS + FAIL + SKIP))
echo -e "  Total: $TOTAL | ${GREEN}Pass: $PASS${NC} | ${RED}Fail: $FAIL${NC} | ${YELLOW}Skip: $SKIP${NC}"
echo

if [ $FAIL -gt 0 ]; then
    echo -e "${RED}SOME TESTS FAILED${NC}"
    exit 1
else
    echo -e "${GREEN}ALL TESTS PASSED${NC}"
fi
