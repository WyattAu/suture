#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

printf '\x00\x01\x02\xff' > binary.bin
suture add binary.bin
suture commit "add binary file"
assert_file_exists "binary.bin"

printf '\x00\x01\x02\xfe\x03\x04' > binary.bin
diff_out=$(suture diff)
assert_contains "$diff_out" "binary" "diff reports binary file change"
suture add binary.bin
suture commit "modify binary file"

log_out=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$log_out" "add binary file" "log shows binary file add commit"
assert_contains "$log_out" "modify binary file" "log shows binary file modify commit"

show_out=$(suture show HEAD)
assert_contains "$show_out" "modify binary file" "show HEAD displays latest binary commit"

report_results "Edge Case: Binary Files"
