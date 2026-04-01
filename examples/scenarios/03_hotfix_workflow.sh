#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

for i in 1 2 3; do
    echo "commit $i" > "file${i}.txt"
    suture add "file${i}.txt"
    suture commit "Add file ${i}"
done

suture branch hotfix

for i in 4 5; do
    echo "commit $i" > "file${i}.txt"
    suture add "file${i}.txt"
    suture commit "Add file ${i}"
done

output=$(suture log --oneline)
line_count=$(echo "$output" | grep -c .)
assert_eq "$line_count" "6" "main has 6 commits"

suture checkout hotfix

echo "fix applied" > fix.txt
suture add fix.txt
suture commit "Critical hotfix"

suture checkout main

output=$(suture merge hotfix)
assert_contains "$output" "Merge successful\|Applied" "hotfix merge succeeds"

output=$(suture log --oneline)
assert_contains "$output" "Add file 4" "main commit 4 still in log"
assert_contains "$output" "Add file 5" "main commit 5 still in log"

assert_file_exists "fix.txt" "hotfix file exists on main"
assert_file_exists "file1.txt" "file1 exists on main"
assert_file_exists "file5.txt" "file5 exists on main"

report_results "Hotfix Workflow"
