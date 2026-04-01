#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

for i in $(seq 1 100); do
    echo "content of file $i" > "file_$i.txt"
done
suture add -a
suture commit "add 100 files"

assert_file_exists "file_1.txt"
assert_file_exists "file_50.txt"
assert_file_exists "file_100.txt"
assert_file_eq "file_1.txt" "content of file 1" "first file correct"
assert_file_eq "file_100.txt" "content of file 100" "last file correct"

for i in $(seq 1 20); do
    echo "modified $i" > "file_$i.txt"
done
suture add -a
suture commit "modify 20 files"

assert_file_eq "file_1.txt" "modified 1" "file 1 modified correctly"
assert_file_eq "file_20.txt" "modified 20" "file 20 modified correctly"
assert_file_eq "file_21.txt" "content of file 21" "file 21 unchanged"

log_out=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$log_out" "add 100 files" "log shows bulk add commit"
assert_contains "$log_out" "modify 20 files" "log shows bulk modify commit"

report_results "Edge Case: Many Files"
