#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

printf "line1\nline2\nline3\n" > file.txt
suture add file.txt
suture commit "Initial file on main"

suture branch feature
suture checkout feature

printf "line1\nFEATURE\nline3\n" > file.txt
suture add file.txt
suture commit "Feature modifies line2"

suture checkout main

printf "line1\nMAIN\nline3\n" > file.txt
suture add file.txt
suture commit "Main modifies line2"

output=$(suture merge feature)
assert_contains "$output" "CONFLICT" "merge output reports conflict"

assert_contains "$output" "file.txt" "conflict names the affected file"

output=$(suture status)
assert_contains "$output" "main" "still on main branch after conflict"

report_results "Merge Conflict"
