#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

mkdir -p a/b/c/d/e/f/g/h/i/j
echo "deep content" > a/b/c/d/e/f/g/h/i/j/file.txt
suture add a/b/c/d/e/f/g/h/i/j/file.txt
suture commit "add deeply nested file"
assert_file_exists "a/b/c/d/e/f/g/h/i/j/file.txt"
assert_file_eq "a/b/c/d/e/f/g/h/i/j/file.txt" "deep content" "deep file has correct content"

echo "modified deep" > a/b/c/d/e/f/g/h/i/j/file.txt
suture add a/b/c/d/e/f/g/h/i/j/file.txt
suture commit "modify deeply nested file"
assert_file_eq "a/b/c/d/e/f/g/h/i/j/file.txt" "modified deep" "deep file modified correctly"

status_out=$(suture status)
assert_contains "$status_out" "3 patches" "status shows 3 commits"

diff_out=$(suture diff)
assert_contains "$diff_out" "No differences" "no unstaged differences"

log_out=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$log_out" "deeply nested" "log shows deep nested commit"

report_results "Edge Case: Deep Directory Nesting"
