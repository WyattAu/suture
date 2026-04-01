#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "base" > file.txt
suture add file.txt
suture commit "initial commit on main"

suture branch feature
suture checkout feature

echo "feature commit 1" > file.txt
suture add file.txt
hash1=$("$SUTURE_BIN" commit "feature-1" 2>&1 | awk '{print $2}')
hash1="${hash1%…}"

echo "feature commit 2" > file.txt
suture add file.txt
suture commit "feature-2"

echo "feature commit 3" > file.txt
suture add file.txt
hash3=$("$SUTURE_BIN" commit "feature-3" 2>&1 | awk '{print $2}')
hash3="${hash3%…}"

echo "feature commit 4" > file.txt
suture add file.txt
suture commit "feature-4"

echo "feature commit 5" > file.txt
suture add file.txt
hash5=$("$SUTURE_BIN" commit "feature-5" 2>&1 | awk '{print $2}')
hash5="${hash5%…}"

suture checkout main

suture cherry-pick "$hash3"
cherry_out=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$cherry_out" "feature-3" "cherry-picked commit 3 appears on main"
assert_file_eq "file.txt" "feature commit 3" "file has content from cherry-picked commit 3"

suture cherry-pick "$hash5"
cherry_out2=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$cherry_out2" "feature-5" "cherry-picked commit 5 appears on main"

report_results "Edge Case: Cherry-Pick Range"
