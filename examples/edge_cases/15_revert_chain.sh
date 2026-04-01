#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "original" > file.txt
suture add file.txt
suture commit "add original content"
assert_file_eq "file.txt" "original" "file has original content"

echo "changed1" > file.txt
suture add file.txt
hash1=$("$SUTURE_BIN" commit "change to v1" 2>&1 | awk '{print $2}')
hash1="${hash1%…}"
assert_file_eq "file.txt" "changed1" "file has changed1 content"

echo "changed2" > file.txt
suture add file.txt
hash2=$("$SUTURE_BIN" commit "change to v2" 2>&1 | awk '{print $2}')
hash2="${hash2%…}"
assert_file_eq "file.txt" "changed2" "file has changed2 content"

suture revert "$hash2"
revert_log1=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$revert_log1" "Revert" "first revert appears in log" || true

suture revert "$hash1"
revert_log2=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$revert_log2" "Revert" "second revert appears in log" || true

report_results "Edge Case: Revert Chain"
