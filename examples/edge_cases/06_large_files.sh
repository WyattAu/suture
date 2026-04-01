#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

dd if=/dev/zero bs=1024 count=1024 2>/dev/null | tr '\0' 'A' > large1.txt
suture add large1.txt
suture commit "add 1MB file"
assert_file_exists "large1.txt"

file_size=$(wc -c < large1.txt)
assert_eq "$file_size" "1048576" "1MB file should be 1048576 bytes"

dd if=/dev/zero bs=1024 count=5120 2>/dev/null | tr '\0' 'B' > large2.txt
suture add large2.txt
suture commit "add 5MB file"
assert_file_exists "large2.txt"

log_out=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$log_out" "add 1MB file" "log shows 1MB file commit"
assert_contains "$log_out" "add 5MB file" "log shows 5MB file commit"

status_out=$(suture status)
assert_contains "$status_out" "3 patches" "status shows 3 commits"

report_results "Edge Case: Large Files"
