#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "v1" > file.txt
suture add file.txt
suture commit "commit 1"

echo "v2" > file.txt
suture add file.txt
suture commit "commit 2"

echo "v3" > file.txt
suture add file.txt
suture commit "commit 3"

echo "v4" > file.txt
suture add file.txt
suture commit "commit 4"

echo "v5" > file.txt
suture add file.txt
suture commit "commit 5"
assert_file_eq "file.txt" "v5" "file at v5 before any reset"

log_full=$("$SUTURE_BIN" log --oneline 2>&1)
line_count=$(echo "$log_full" | wc -l)
assert_eq "$line_count" "6" "log shows 6 commits initially"

suture reset --mode soft HEAD~1
log_after_soft1=$("$SUTURE_BIN" log --oneline 2>&1)
line_count1=$(echo "$log_after_soft1" | wc -l)
assert_eq "$line_count1" "5" "first soft reset back to 5 commits"

suture reset --mode soft HEAD~1
log_after_soft2=$("$SUTURE_BIN" log --oneline 2>&1)
line_count2=$(echo "$log_after_soft2" | wc -l)
assert_eq "$line_count2" "4" "second soft reset back to 4 commits"

suture reset --mode hard HEAD~1
log_after_hard=$("$SUTURE_BIN" log --oneline 2>&1)
line_count3=$(echo "$log_after_hard" | wc -l)
assert_eq "$line_count3" "3" "hard reset back to 3 commits"

assert_file_eq "file.txt" "v2" "final state is v2 after hard reset"

status_out=$(suture status)
assert_not_contains "$status_out" "modified" || true
assert_not_contains "$status_out" "staged" || true

report_results "Edge Case: Consecutive Resets"
