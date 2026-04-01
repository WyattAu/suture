#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "v1" > file.txt
suture add file.txt
suture commit "Version 1"

echo "v2" > file.txt
suture add file.txt
suture commit "Version 2"

echo "v3" > file.txt
suture add file.txt
suture commit "Version 3"

output=$(suture reset --mode soft HEAD~1)
assert_contains "$output" "HEAD" "soft reset moves HEAD"

assert_file_eq "file.txt" "v3" "soft reset preserves file content"

output=$(suture status)
assert_contains "$output" "Staged\|staged\|Changes" "soft reset keeps changes staged"

suture add file.txt
suture commit "Re-commit v3"

output=$(suture reset --mode mixed HEAD~1)
assert_contains "$output" "HEAD" "mixed reset moves HEAD"

assert_file_eq "file.txt" "v3" "mixed reset preserves file content"

output=$(suture status)
assert_contains "$output" "Unstaged\|unstaged\|modified" "mixed reset shows unstaged changes"

setup_repo

echo "h1" > data.txt
suture add data.txt
suture commit "Hard v1"

echo "h2" > data.txt
suture add data.txt
suture commit "Hard v2"

output=$(suture reset --mode hard HEAD~1)
assert_contains "$output" "HEAD" "hard reset moves HEAD"

assert_file_eq "data.txt" "h1" "hard reset restores file to earlier version"

output=$(suture status)
assert_not_contains "$output" "Staged" "no staged changes after hard reset"
assert_not_contains "$output" "Unstaged" "no unstaged changes after hard reset"

report_results "Reset Workflow"
