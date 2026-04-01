#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "space content" > "my file.txt"
suture add "my file.txt"
suture commit "add file with spaces"
assert_file_exists "my file.txt"
assert_file_eq "my file.txt" "space content" "file with spaces has correct content"

echo "cafe content" > "café.txt"
suture add "café.txt"
suture commit "add file with unicode name"
assert_file_exists "café.txt"

echo "hidden" > ".hidden"
suture add ".hidden"
suture commit "add dotfile"
assert_file_exists ".hidden"
assert_file_eq ".hidden" "hidden" "dotfile has correct content"

mkdir -p "my dir"
echo "nested" > "my dir/nested file.txt"
suture add "my dir/nested file.txt"
suture commit "add nested file in dir with spaces"
assert_file_exists "my dir/nested file.txt"
assert_file_eq "my dir/nested file.txt" "nested" "nested file has correct content"

status_out=$(suture status)
assert_contains "$status_out" "5 patches" "status shows 5 commits"

report_results "Edge Case: Special Characters in Filenames"
