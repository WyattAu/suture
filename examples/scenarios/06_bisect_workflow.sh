#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "" > status.txt
for i in $(seq 1 7); do
    if [[ $i -eq 4 ]]; then
        echo "BUG" >> status.txt
    else
        echo "OK" >> status.txt
    fi
    suture add status.txt
    suture commit "Commit $i"
done

log_output=$(suture log --oneline)
commit_count=$(echo "$log_output" | grep -c . || true)
assert_eq "$commit_count" "8" "commit count (7 loop + 1 init)"

oldest_hash=$(echo "$log_output" | tail -1 | awk '{print $1}')
newest_hash=$(echo "$log_output" | head -1 | awk '{print $1}')

output=$(suture bisect start "$oldest_hash" "$newest_hash")
assert_contains "$output" "Bisecting" "bisect output mentions Bisecting"

assert_contains "$output" "commit" "bisect reports a midpoint commit"

output=$(suture bisect reset)
assert_contains "$output" "reset" "bisect reset succeeds"

log_after=$(suture log --oneline)
assert_eq "$log_after" "$log_output" "log unchanged after bisect reset"

report_results "Bisect Workflow"
