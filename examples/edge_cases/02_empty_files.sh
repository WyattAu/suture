#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

touch empty.txt
suture add empty.txt
suture commit "add empty file"
assert_file_exists "empty.txt"
assert_file_eq "empty.txt" "" "empty file should be empty"

echo "hello" > empty.txt
suture add empty.txt
suture commit "add content to file"
assert_file_eq "empty.txt" "hello" "file should now have content"

printf "" > empty.txt
suture add empty.txt
suture commit "remove all content from file"
assert_file_eq "empty.txt" "" "file should be empty again"

log_out=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$log_out" "add empty file" "log shows empty file commit"
assert_contains "$log_out" "add content" "log shows content addition commit"
assert_contains "$log_out" "remove all content" "log shows emptying commit"

report_results "Edge Case: Empty Files"
