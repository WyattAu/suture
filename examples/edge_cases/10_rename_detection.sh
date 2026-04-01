#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "original content" > file.txt
suture add file.txt
suture commit "add file.txt"
assert_file_exists "file.txt"

suture mv file.txt renamed.txt
status_out=$(suture status)
assert_contains "$status_out" "renamed" "status after mv shows rename" || true
assert_file_not_exists "file.txt" "original gone after mv"
assert_file_exists "renamed.txt" "renamed file exists"
assert_file_eq "renamed.txt" "original content" "content preserved after rename"

suture add renamed.txt
suture commit "rename file.txt to renamed.txt"

echo "manual content" > another.txt
suture rm renamed.txt
suture add renamed.txt
echo "manual content" > manually_renamed.txt
suture add manually_renamed.txt
suture commit "manual rename: rm old, add new"
assert_file_not_exists "renamed.txt" "old file gone after manual rename"
assert_file_exists "manually_renamed.txt" "new manually renamed file exists"
assert_file_eq "manually_renamed.txt" "manual content" "manual rename preserves content"

report_results "Edge Case: Rename Detection"
