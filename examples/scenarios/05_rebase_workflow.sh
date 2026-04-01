#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "base" > base.txt
suture add base.txt
suture commit "Initial commit"

echo "main update 1" >> base.txt
suture add base.txt
suture commit "Main update 1"

suture branch feature
suture checkout feature

echo "feature A" > feat_a.txt
suture add feat_a.txt
suture commit "Feature A"

echo "feature B" > feat_b.txt
suture add feat_b.txt
suture commit "Feature B"

suture checkout main

echo "main update 2" >> base.txt
suture add base.txt
suture commit "Main update 2"

echo "main update 3" >> base.txt
suture add base.txt
suture commit "Main update 3"

suture checkout feature

output=$(suture rebase main)
assert_contains "$output" "replay\|Rebase\|Already" "rebase succeeds"

output=$(suture log --oneline)
assert_contains "$output" "Feature A" "feature A in history after rebase"
assert_contains "$output" "Feature B" "feature B in history after rebase"
assert_contains "$output" "Main update 3" "rebased onto latest main"
assert_contains "$output" "Main update 2" "rebased includes main update 2"

assert_file_exists "feat_a.txt" "feature file A exists"
assert_file_exists "feat_b.txt" "feature file B exists"
assert_file_exists "base.txt" "base file exists with main updates"

output=$(suture diff)
assert_not_contains "$output" "feat_a\|feat_b" "no uncommitted diffs after rebase"

report_results "Rebase Workflow"
