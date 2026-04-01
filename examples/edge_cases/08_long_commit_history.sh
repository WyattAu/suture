#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "base" > history.txt
suture add history.txt
suture commit "initial commit"

for i in $(seq 2 100); do
    echo "line $i" >> history.txt
    suture add history.txt
    suture commit "commit $i"
done

log_out=$("$SUTURE_BIN" log --oneline 2>&1)
line_count=$(echo "$log_out" | wc -l)
assert_eq "$line_count" "101" "log should show 101 commits"

suture show HEAD~50
show_out=$("$SUTURE_BIN" show HEAD~50 2>&1)
assert_contains "$show_out" "commit" "show HEAD~50 shows a commit"

suture show HEAD~100
show_root=$("$SUTURE_BIN" show HEAD~100 2>&1)
assert_contains "$show_root" "commit" "show HEAD~100 shows root commit"
assert_contains "$show_root" "Initial commit" "HEAD~100 is the initial commit"

suture_fail show HEAD~101

report_results "Edge Case: Long Commit History"
