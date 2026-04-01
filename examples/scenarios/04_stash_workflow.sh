#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "initial" > base.txt
suture add base.txt
suture commit "Initial commit"

echo "feature in progress" > feature.txt
suture add feature.txt

output=$(suture stash push -m "WIP: feature work")
assert_contains "$output" "Saved\|stash" "stash push succeeds"

output=$(suture stash list)
assert_contains "$output" "WIP: feature work" "stash list shows message"

output=$(suture status)
assert_not_contains "$output" "Staged changes" "no staged changes after stash"

echo "urgent fix" > urgent.txt
suture add urgent.txt
suture commit "Urgent fix"

output=$(suture log --oneline)
assert_contains "$output" "Urgent fix" "urgent fix committed"

output=$(suture stash pop)
assert_contains "$output" "popped\|applied" "stash pop succeeds"

assert_file_exists "feature.txt" "stashed file restored after pop"

output=$(suture stash list)
assert_not_contains "$output" "WIP: feature work" "stash removed after pop"

assert_file_exists "urgent.txt" "urgent fix file still exists"
assert_file_eq "feature.txt" "feature in progress" "feature content matches"

report_results "Stash Workflow"
