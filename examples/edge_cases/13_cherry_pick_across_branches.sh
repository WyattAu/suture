#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/../test_lib.sh"

setup_repo

echo "main base" > file.txt
suture add file.txt
suture commit "main commit 1"

echo "main update" > file.txt
suture add file.txt
suture commit "main commit 2"

echo "main final" > file.txt
suture add file.txt
suture commit "main commit 3"

suture branch feature
suture checkout feature

echo "feature change A" > feature.txt
suture add feature.txt
hashA=$("$SUTURE_BIN" commit "feature commit A" 2>&1 | awk '{print $2}')
hashA="${hashA%…}"

echo "feature change B" > feature.txt
suture add feature.txt
hashB=$("$SUTURE_BIN" commit "feature commit B" 2>&1 | awk '{print $2}')
hashB="${hashB%…}"

suture checkout main
sleep 1

main_log_before=$("$SUTURE_BIN" log --oneline 2>&1)
assert_not_contains "$main_log_before" "feature commit A" "feature commit not on main before cherry-pick" || true

suture cherry-pick "$hashA"
assert_file_exists "feature.txt" "feature.txt exists after cherry-pick"
assert_file_eq "feature.txt" "feature change A" "feature.txt has correct content"

main_log_after=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$main_log_after" "feature commit A" "cherry-picked commit A appears on main"

suture cherry-pick "$hashB" || true
assert_file_eq "feature.txt" "feature change B" "feature.txt updated after second cherry-pick"
main_log_after2=$("$SUTURE_BIN" log --oneline 2>&1)
assert_contains "$main_log_after2" "feature commit B" "cherry-picked commit B appears on main"

report_results "Edge Case: Cherry-Pick Across Branches"
