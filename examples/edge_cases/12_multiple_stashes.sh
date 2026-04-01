#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "base" > base.txt
suture add base.txt
suture commit "initial commit"

echo "file1 content" > file1.txt
suture add file1.txt
suture stash push -m "stash file1"

echo "file2 content" > file2.txt
suture add file2.txt
suture stash push -m "stash file2"

echo "file3 content" > file3.txt
suture add file3.txt
suture stash push -m "stash file3"

stash_list=$("$SUTURE_BIN" stash list 2>&1)
assert_contains "$stash_list" "stash file1" "stash list shows first stash"
assert_contains "$stash_list" "stash file2" "stash list shows second stash"
assert_contains "$stash_list" "stash file3" "stash list shows third stash"

suture stash pop
assert_file_exists "file3.txt" "pop restores file3.txt"
assert_file_eq "file3.txt" "file3 content" "file3.txt has correct content"

stash_list2=$("$SUTURE_BIN" stash list 2>&1)
assert_contains "$stash_list2" "stash file1" "first stash still in list"
assert_contains "$stash_list2" "stash file2" "second stash still in list"
assert_not_contains "$stash_list2" "stash file3" "third stash removed after pop" || true

suture stash pop
assert_file_eq "file2.txt" "file2 content" "second pop restores file2.txt"

suture stash pop
assert_file_eq "file1.txt" "file1 content" "third pop restores file1.txt"

stash_list3=$("$SUTURE_BIN" stash list 2>&1)
assert_contains "$stash_list3" "No stashes" "stash list empty after all pops" || true

report_results "Edge Case: Multiple Stashes"
