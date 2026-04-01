#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "first" > file.txt
suture add file.txt
suture commit "first commit"

echo "second" > file.txt
suture add file.txt
suture commit "second commit"

echo "third" > file.txt
suture add file.txt
suture commit "third commit"

log_full=$("$SUTURE_BIN" log --oneline 2>&1)
line_count=$(echo "$log_full" | wc -l)
assert_eq "$line_count" "4" "log shows all 4 commits"

log_since=$("$SUTURE_BIN" log --since 2020-01-01 2>&1)
assert_contains "$log_since" "first commit" "since filter includes old commits"

log_oneline=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$log_oneline" "first commit" "oneline shows first commit"
assert_contains "$log_oneline" "second commit" "oneline shows second commit"
assert_contains "$log_oneline" "third commit" "oneline shows third commit"

suture branch feature
suture checkout feature

echo "feature work" > feature.txt
suture add feature.txt
suture commit "feature commit"

suture checkout main
suture merge feature

log_merged=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$log_merged" "first commit" "merged log has first commit"

log_first_parent=$("$SUTURE_BIN" log --first-parent 2>&1)
assert_contains "$log_first_parent" "first commit" "first-parent shows main line commits"

report_results "Edge Case: Log Filtering"
