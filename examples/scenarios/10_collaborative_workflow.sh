#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "shared base" > shared.txt
suture add shared.txt
suture commit "Initial shared code"

suture branch feature-a
suture checkout feature-a

echo "dev A component" > dev_a.txt
suture add dev_a.txt
suture commit "Dev A: add component A"

feat_a_hash=$(suture log --oneline | head -1 | awk '{print $1}')

echo "dev A utility" > util_a.txt
suture add util_a.txt
suture commit "Dev A: add utility"

suture checkout main

suture branch feature-b
suture checkout feature-b

echo "dev B component" > dev_b.txt
suture add dev_b.txt
suture commit "Dev B: add component B"

suture cherry-pick "$feat_a_hash"

echo "dev B tests" > tests_b.txt
suture add tests_b.txt
suture commit "Dev B: add tests"

suture checkout main

output=$(suture merge feature-a)
assert_contains "$output" "Merge\|merge\|Applied" "merge feature-a succeeds"

output=$(suture merge feature-b)
assert_contains "$output" "Merge\|merge\|Applied" "merge feature-b succeeds"

log=$(suture log --oneline)
assert_contains "$log" "Merge" "merge commits appear in history"

assert_file_exists "dev_a.txt" "dev A component file exists"
assert_file_exists "dev_b.txt" "dev B component file exists"
assert_file_exists "util_a.txt" "cherry-picked utility file exists"
assert_file_exists "tests_b.txt" "dev B tests file exists"
assert_file_exists "shared.txt" "shared base file exists"

report_results "Collaborative Workflow"
