#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "v1 content" > app.txt
suture add app.txt
suture commit "Version 1.0 release"

suture tag v1.0

echo "v2 content" > app.txt
suture add app.txt
suture commit "Version 2.0 release"

suture tag -a v2.0 -m "Official v2.0 release"

echo "v3 content" > app.txt
suture add app.txt
suture commit "Version 3.0 release"

output=$(suture tag --list)
assert_contains "$output" "v1.0" "tag list includes v1.0"
assert_contains "$output" "v2.0" "tag list includes v2.0"

output=$(suture show v1.0)
assert_contains "$output" "Version 1.0" "show v1.0 displays correct commit"

output=$(suture show v2.0)
assert_contains "$output" "Version 2.0" "show v2.0 displays correct commit"

output=$(suture show HEAD)
assert_contains "$output" "Version 3.0" "show HEAD displays latest commit"

output=$(suture notes add HEAD -m "Reviewed by QA team")
assert_contains "$output" "Note added\|note" "note added to HEAD"

output=$(suture notes list HEAD)
assert_contains "$output" "Reviewed by QA team" "note content appears in list"

report_results "Tag and Notes"
