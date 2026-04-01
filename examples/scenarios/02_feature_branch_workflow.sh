#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "readme" > README.md
suture add README.md
suture commit "Initial commit"

suture branch feature

echo "main work" > main.txt
suture add main.txt
suture commit "Work on main"

echo "more main work" >> main.txt
suture add main.txt
suture commit "More work on main"

suture checkout feature

echo "feature code" > feature.txt
suture add feature.txt
suture commit "Add feature"

echo "more feature code" >> feature.txt
suture add feature.txt
suture commit "Extend feature"

suture checkout main

output=$(suture merge feature)
assert_contains "$output" "Merge successful\|Applied" "merge succeeds"

output=$(suture log --oneline)
assert_contains "$output" "Work on main" "log contains main commit"
assert_contains "$output" "More work on main" "log contains second main commit"

output=$(suture log --first-parent --oneline)
assert_contains "$output" "Work on main" "first-parent includes main-line commits"
assert_contains "$output" "More work on main" "first-parent includes second main commit"

assert_file_exists "feature.txt" "feature file exists after merge"
assert_file_exists "main.txt" "main file exists after merge"

report_results "Feature Branch Workflow"
