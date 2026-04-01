#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "v1" > file.txt
suture add file.txt
hash1=$("$SUTURE_BIN" commit "create file with v1" 2>&1 | awk '{print $2}')
hash1="${hash1%…}"
assert_file_exists "file.txt"
assert_file_eq "file.txt" "v1" "file has v1 content"

suture rm file.txt
hash2=$("$SUTURE_BIN" commit "delete file.txt" 2>&1 | awk '{print $2}')
hash2="${hash2%…}"
assert_file_not_exists "file.txt"

echo "v2" > file.txt
suture add file.txt
hash3=$("$SUTURE_BIN" commit "recreate file.txt with v2" 2>&1 | awk '{print $2}')
hash3="${hash3%…}"
assert_file_exists "file.txt"
assert_file_eq "file.txt" "v2" "recreated file has v2 content"

log_out=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$log_out" "create file with v1" "log shows creation commit"
assert_contains "$log_out" "delete file.txt" "log shows deletion commit"
assert_contains "$log_out" "recreate file.txt with v2" "log shows recreation commit"

show1=$("$SUTURE_BIN" show "$hash1" 2>&1)
assert_contains "$show1" "create file with v1" "show for creation commit"

show2=$("$SUTURE_BIN" show "$hash2" 2>&1)
assert_contains "$show2" "delete file.txt" "show for deletion commit"

show3=$("$SUTURE_BIN" show "$hash3" 2>&1)
assert_contains "$show3" "recreate file.txt with v2" "show for recreation commit"

report_results "Edge Case: File Deletion and Recreation"
