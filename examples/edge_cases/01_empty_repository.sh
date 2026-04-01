#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

status_out=$(suture status)
assert_contains "$status_out" "main" "status shows default branch"
assert_contains "$status_out" "1 patches" "status shows 1 patch from initial commit"

log_out=$(suture log --oneline)
assert_contains "$log_out" "Initial commit" "log shows initial commit"

diff_out=$(suture diff)
assert_contains "$diff_out" "No differences" "diff shows no differences on empty repo"

suture_fail commit "should fail with nothing staged"

suture branch test-branch
branch_list=$("$SUTURE_BIN" branch --list 2>&1)
assert_contains "$branch_list" "main" "branch list shows main"
assert_contains "$branch_list" "test-branch" "branch list shows pre-commit branch"

report_results "Edge Case: Empty Repository"
