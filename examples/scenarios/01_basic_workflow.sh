#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "Hello, World!" > hello.txt
output=$(suture add hello.txt)
assert_contains "$output" "Added hello.txt" "add output"

output=$(suture commit "Initial commit")
assert_contains "$output" "Committed" "commit output"

output=$(suture status)
assert_contains "$output" "On branch main" "status shows branch"
assert_not_contains "$output" "Staged changes" "no staged changes after commit"
assert_not_contains "$output" "Unstaged changes" "no unstaged changes after commit"

output=$(suture log)
assert_contains "$output" "Initial commit" "log shows commit message"

output=$(suture show HEAD)
assert_contains "$output" "Initial commit" "show displays commit message"
assert_contains "$output" "Author" "show displays author"
assert_contains "$output" "commit " "show displays commit hash"

echo "Second file" > second.txt
suture add second.txt
suture commit "Add second file"

output=$(suture log --oneline)
line_count=$(echo "$output" | grep -c .)
assert_eq "$line_count" "3" "oneline log count"
assert_contains "$output" "Add second file" "log shows second commit"
assert_contains "$output" "Initial commit" "log shows first commit"

output=$(suture show HEAD~2)
assert_contains "$output" "Initial commit" "HEAD~2 shows initial commit"

report_results "Basic Workflow"
