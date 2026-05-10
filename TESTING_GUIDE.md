# Suture Testing Guide

A step-by-step walkthrough to verify every Suture feature on a real repository.
Run each section in order. Each section is independent — if one fails, note it and continue.

Prerequisites: `cargo install suture-cli` (or use a local build at `target/release/suture`).

---

## 0. Setup

```bash
# Use a dedicated test directory — NOT your real project
mkdir -p /tmp/suture-test && cd /tmp/suture-test
rm -rf demo-repo       # clean slate each run
S=suture               # or: S=/path/to/target/release/suture
```

---

## 1. Init & Config

```bash
$S init demo-repo && cd demo-repo

# 1.1 Set identity
$S config user.name "Tester"
$S config user.email "tester@example.com"
$S config user.name          # should print: Tester

# 1.2 Global config (separate from repo)
$S config --global user.name "Global Tester"
$S config --global user.name # should print: Global Tester
$S config user.name          # should still print: Tester (local wins)

# 1.3 Unknown key should fail
$S config nonexistent.key    # should error

# 1.4 Status on fresh repo
$S status                    # should show: On branch main, 1 patch
```

**Expected:** All commands exit 0. `config` local overrides global. Unknown key errors.

---

## 2. Ignore Patterns

```bash
cat > .sutureignore << 'EOF'
# Build artifacts
*.o
*.log
target/
EOF

# 2.1 List patterns
$S ignore list              # should show 3 patterns (comments stripped)

# 2.2 Check paths
$S ignore check "target/debug/app.o"  # should say: ignored
$S ignore check "src/main.rs"         # should say: NOT ignored

# 2.3 Create ignored file — should NOT appear in status
touch target/debug.o
echo "junk" > build/output.log
$S status | grep -c "debug.o\|output.log"  # should print: 0
```

**Expected:** Patterns listed correctly. Ignored files don't appear in status.

---

## 3. Basic Workflow — Add, Commit, Status, Log, Diff

```bash
# 3.1 Create files in multiple formats
cat > config.json << 'EOF'
{"server":{"host":"localhost","port":8080},"logging":{"level":"info"}}
EOF

cat > pipeline.yaml << 'EOF'
stages:
  - name: build
    image: rust:latest
EOF

cat > app.toml << 'EOF'
[database]
url = "postgres://localhost/db"
pool = 10
EOF

cat > users.csv << 'EOF'
id,name,email
1,Alice,alice@example.com
2,Bob,bob@example.com
EOF

cat > layout.xml << 'EOF'
<?xml version="1.0"?>
<window title="App" width="800" height="600">
  <panel id="main"><label text="Hello"/></panel>
</window>
EOF

cat > README.md << 'EOF'
# Demo Project
## Getting Started
Install the tool.
EOF

cat > notes.txt << 'EOF'
Line one
Line two
Line three
EOF

# 3.2 Status before add — should show untracked files
$S status | grep -c "untracked"  # should be > 0

# 3.3 Add specific files
$S add config.json pipeline.yaml app.toml users.csv layout.xml README.md notes.txt .sutureignore
$S status | grep -c "Added"     # should be 7

# 3.4 Commit
$S commit "Initial commit: all format files"
$S log --oneline                # should show 2 commits (init + this one)

# 3.5 Clean status
$S diff                        # should print: No differences

# 3.6 Modify files and verify semantic diff
cat > config.json << 'EOF'
{"server":{"host":"0.0.0.0","port":9090,"tls":true},"logging":{"level":"debug"}}
EOF

cat > users.csv << 'EOF'
id,name,email
1,Alice,alice@newdomain.com,admin
2,Bob,bob@example.com,user
3,Charlie,charlie@example.com,user
EOF

$S diff config.json   # should show semantic changes: MODIFIED /server/host, ADDED /server/tls, etc.
$S diff users.csv     # should show semantic changes: MODIFIED /email:0, ADDED /rows/2
$S diff notes.txt     # should show line-based diff (no driver for .txt)

# 3.7 Add --all (should respect .sutureignore)
$S add --all
$S commit "Update config and users"

# 3.8 Show commit
$S show HEAD           # should show commit hash, author, message, parents

# 3.9 Blame
$S blame notes.txt     # should show per-line commit attribution

# 3.10 Log graph
$S log --oneline --graph
```

**Expected:** Semantic diffs for JSON/CSV show path-level changes. Line diff for .txt. Blame shows correct attribution. Graph shows branch topology.

---

## 4. Branching

```bash
# 4.1 Create and list branches
$S branch feature/test
$S branch                # should show: feature/test, * main

# 4.2 Create from specific targets
$S branch hotfix --target HEAD     # should succeed
$S branch old --target HEAD~1      # should succeed

# 4.3 Checkout
$S checkout feature/test
$S status | grep "feature/test"    # should show: On branch feature/test

# 4.4 Checkout -b (create + switch)
$S checkout -b feature/test2
$S status | grep "feature/test2"   # should show: On branch feature/test2

# 4.5 Protect/unprotect
$S checkout main
$S branch --protect main
$S branch | grep "protected"       # should show: main [protected]
$S branch --unprotect main
$S branch | grep "protected"       # should NOT show protected

# 4.6 Delete branches
$S branch --delete hotfix
$S branch --delete old
$S branch --delete feature/test
$S branch --delete feature/test2
$S branch                # should show only: * main
```

**Expected:** All branch operations succeed. `--target HEAD` and `--target HEAD~N` work (this was a bug in v0.8.0, fixed in v0.8.1).

---

## 5. Semantic Merge — JSON

This is the #1 selling point. Pay close attention.

```bash
# 5.1 Start from clean state on main
cat > config.json << 'EOF'
{"server":{"host":"localhost","port":8080},"logging":{"level":"info"}}
EOF
$S add config.json && $S commit "Base config"

# 5.2 Create feature branch — change host and add TLS
$S checkout -b feature/add-tls
cat > config.json << 'EOF'
{"server":{"host":"0.0.0.0","port":8080,"tls":true},"logging":{"level":"info"}}
EOF
$S add config.json && $S commit "Feature: change host, add TLS"

# 5.3 On main — change port and log level (different fields)
$S checkout main
cat > config.json << 'EOF'
{"server":{"host":"localhost","port":9090},"logging":{"level":"debug","file":"/var/log/app.log"}}
EOF
$S add config.json && $S commit "Main: change port and logging"

# 5.4 DRY RUN first — should say it would resolve via json driver
$S merge feature/add-tls --dry-run

# 5.5 Actual merge — should resolve cleanly via semantic driver
$S merge feature/add-tls
# Expected output: "Resolved config.json via json driver"

# 5.6 Verify merged result — ALL changes from both sides
cat config.json
# Expected JSON:
#   server.host = "0.0.0.0"  (from feature — both changed, same value wins)
#   server.port = 9090        (from main — both changed, same value wins)
#   server.tls = true         (from feature — only feature added it)
#   logging.level = "debug"   (from main — main changed it from base "info")
#   logging.file = "..."      (from main — only main added it)

# 5.7 Finalize
$S add config.json && $S commit "Merge feature/add-tls"
```

**Expected:** Merge resolves cleanly via JSON driver. No conflict markers. All fields from both branches present. **If you see conflict markers, the JSON driver failed — this is a bug.**

---

## 6. Semantic Merge — CSV

```bash
cat > users.csv << 'EOF'
id,name,email,role
1,Alice,alice@example.com,admin
2,Bob,bob@example.com,user
3,Charlie,charlie@example.com,user
EOF
$S add users.csv && $S commit "Base users"

$S checkout -b feature/update-emails
cat > users.csv << 'EOF'
id,name,email,role
1,Alice,alice@newdomain.com,admin
2,Bob,bob@example.com,user
3,Charlie,charlie@example.com,user
4,Diana,diana@example.com,user
EOF
$S add users.csv && $S commit "Feature: update Alice email, add Diana"

$S checkout main
cat > users.csv << 'EOF'
id,name,email,role
1,Alice,alice@example.com,admin
2,Bob,bob@example.com,user
3,Charlie,charlie@example.com,moderator
EOF
$S add users.csv && $S commit "Main: promote Charlie"

$S merge feature/update-emails
# Expected: "Resolved users.csv via csv driver"

# Verify: Alice's email updated, Charlie promoted, Diana added
cat users.csv
# Check for these data points:
grep "alice@newdomain.com" users.csv   # should exist (from feature)
grep "moderator" users.csv             # should exist (from main)
grep "Diana" users.csv                 # should exist (from feature)

$S add users.csv && $S commit "Merge CSV"
```

**Expected:** CSV driver merges row-level changes. No conflict markers.

---

## 7. Semantic Merge — TOML

```bash
cat > app.toml << 'EOF'
[server]
host = "localhost"
port = 3000

[database]
url = "postgres://localhost/db"
EOF
$S add app.toml && $S commit "Base config"

$S checkout -b feature/add-cache
cat > app.toml << 'EOF'
[server]
host = "0.0.0.0"
port = 3000

[database]
url = "postgres://localhost/db"

[cache]
enabled = true
ttl = 300
EOF
$S add app.toml && $S commit "Feature: change host, add cache"

$S checkout main
cat > app.toml << 'EOF'
[server]
host = "localhost"
port = 9090

[database]
url = "postgres://localhost/db"
pool = 20
EOF
$S add app.toml && $S commit "Main: change port, add pool"

$S merge feature/add-cache
# Expected: "Resolved app.toml via toml driver"

cat app.toml
# Verify: server.host = "0.0.0.0", server.port = 9090, cache section present, pool = 20

$S add app.toml && $S commit "Merge TOML"
```

**Expected:** TOML driver merges table-level changes. Both `[cache]` (from feature) and `pool` (from main) present.

---

## 8. Semantic Merge — XML

```bash
cat > layout.xml << 'EOF'
<?xml version="1.0"?>
<window title="App" width="800" height="600">
  <panel id="sidebar"><button id="home" label="Home"/></panel>
  <panel id="content"><label text="Hello"/></panel>
</window>
EOF
$S add layout.xml && $S commit "Base layout"

$S checkout -b feature/add-settings
cat > layout.xml << 'EOF'
<?xml version="1.0"?>
<window title="App" width="1024" height="600">
  <panel id="sidebar"><button id="home" label="Home"/><button id="settings" label="Settings"/></panel>
  <panel id="content"><label text="Hello"/></panel>
</window>
EOF
$S add layout.xml && $S commit "Feature: wider window, add settings button"

$S checkout main
cat > layout.xml << 'EOF'
<?xml version="1.0"?>
<window title="App" width="800" height="768">
  <panel id="sidebar"><button id="home" label="Home"/></panel>
  <panel id="content"><label text="Hello"/><label text="World"/></panel>
</window>
EOF
$S add layout.xml && $S commit "Main: taller window, add world label"

$S merge feature/add-settings
# Expected: "Resolved layout.xml via xml driver"

cat layout.xml
# Verify: width=1024, height=768, settings button present, World label present

$S add layout.xml && $S commit "Merge XML"
```

**Expected:** XML driver merges element and attribute changes. Note: the XML serializer may reorder attributes — this is cosmetic, not a correctness issue.

---

## 9. Semantic Merge — Markdown

```bash
cat > README.md << 'EOF'
# Demo Project
## Getting Started
Install the tool.
## API
See docs.
EOF
$S add README.md && $S commit "Base README"

$S checkout -b feature/add-examples
cat > README.md << 'EOF'
# Demo Project
## Getting Started
Install the tool.
## Examples
See examples folder.
## API
See docs.
EOF
$S add README.md && $S commit "Feature: add examples section"

$S checkout main
cat > README.md << 'EOF'
# Demo Project
## Getting Started
Install the tool.
Run `suture init`.
## API
See docs.
EOF
$S add README.md && $S commit "Main: add init instruction"

$S merge feature/add-examples
# Expected: "Resolved README.md via markdown driver"

cat README.md
# Verify: all sections present (Getting Started with both lines, Examples, API)

$S add README.md && $S commit "Merge markdown"
```

**Expected:** Markdown driver merges at block level. All sections from both branches present.

---

## 10. Plain Text Conflict (No Semantic Driver)

```bash
cat > notes.txt << 'EOF'
Line 1
Line 2
Line 3
EOF
$S add notes.txt && $S commit "Base notes"

$S checkout -b feature/change-notes
cat > notes.txt << 'EOF'
Line 1
Modified by feature
Line 3
EOF
$S add notes.txt && $S commit "Feature: modify line 2"

$S checkout main
cat > notes.txt << 'EOF'
Line 1
Modified by main
Line 3
EOF
$S add notes.txt && $S commit "Main: modify line 2 differently"

$S merge feature/change-notes
# Expected: "Merge has 1 conflict(s)"

cat notes.txt
# Should contain conflict markers:
# <<<<<<< main (HEAD)
# Modified by main
# =======
# Modified by feature
# >>>>>>> feature/change-notes

# 10.1 Manual resolution
cat > notes.txt << 'EOF'
Line 1
Modified by both
Line 3
EOF
$S add notes.txt && $S commit "Merge notes (manual resolve)"
```

**Expected:** Plain text produces conflict markers. Manual resolution and commit works.

---

## 11. Fast-Forward Merge

```bash
$S checkout -b feature/ff
echo "ff test" > ff-test.txt
$S add ff-test.txt && $S commit "Feature: ff test"

$S checkout main
$S merge feature/ff
# Expected: "Merge successful" or "Fast-forward"

# Already up to date (merge again)
$S merge feature/ff
# Expected: "Already up to date" or similar
```

**Expected:** FF merge succeeds. Second merge is a no-op.

---

## 12. Stash

```bash
# 12.1 Stash a modification
echo "stash me" >> notes.txt
$S stash push -m "work in progress"
# Expected: "Saved as stash@{0}"

# 12.2 Status should be clean
$S status | grep -c "Staged\|Unstaged"  # should be 0 (or only untracked)

# 12.3 List stashes
$S stash list        # should show: stash@{0}: work in progress

# 12.4 Pop restores the change
$S stash pop         # Expected: "Restored stash@{0}"
grep "stash me" notes.txt   # should exist

# 12.5 Stash again and use apply (keeps stash entry)
$S stash push -m "second stash"
$S stash apply 0
grep "stash me" notes.txt   # should exist
$S stash list        # should still show stash@{0}

# 12.6 Drop
$S stash drop 0
$S stash list        # should be empty

# 12.7 Pop empty stash
$S stash pop         # Expected: "No stashes to pop"
```

**Expected:** Stash saves, restores, and removes correctly. `pop` removes, `apply` keeps.

---

## 13. Undo (Soft Reset)

```bash
echo "undo test 1" >> notes.txt
$S add notes.txt && $S commit "Commit A"
echo "undo test 2" >> notes.txt
$S add notes.txt && $S commit "Commit B"
echo "undo test 3" >> notes.txt
$S add notes.txt && $S commit "Commit C"

$S log --oneline | head -3
# Should show: Commit C, Commit B, Commit A

# 13.1 Undo 1 step
$S undo --steps 1
$S log --oneline | head -3
# Should show: Commit B, Commit A, ... (C gone from log)

# 13.2 Undo 2 more steps
$S undo --steps 2
$S log --oneline | head -3
# Should show: ... (A and B gone from log)

# 13.3 Verify: undo is soft reset — file content is preserved but staged
$S status | grep "notes.txt"
# Should show notes.txt as staged+unstaged
```

**Expected:** `undo` moves HEAD back. File content stays in working tree (soft reset by design).

---

## 14. Reset Modes

```bash
# Make a clean commit to reset from
cat > reset-test.txt << 'EOF'
original content
EOF
$S add reset-test.txt && $S commit "Before reset"

# Modify the file
cat > reset-test.txt << 'EOF'
modified content
EOF
$S add reset-test.txt && $S commit "After modification"

# 14.1 Hard reset — restores files
$S reset HEAD~1 --mode hard
cat reset-test.txt
# Should print: original content
$S status | grep -c "reset-test"  # should be 0 (clean)

# 14.2 Mixed reset — unstages but keeps files
cat > reset-test.txt << 'EOF'
mixed content
EOF
$S add reset-test.txt && $S commit "Mixed test"
$S reset HEAD~1 --mode mixed
$S status | grep "reset-test"    # should show as untracked
cat reset-test.txt               # should print: mixed content
```

**Expected:** `--mode hard` restores files. `--mode mixed` unstages but keeps working tree.

---

## 15. Cherry-Pick, Revert, Rebase, Squash

```bash
# 15.1 Cherry-pick
$S checkout -b feature/_pick-test
echo "pick this" > pick-test.txt
$S add pick-test.txt && $S commit "Commit to pick"
PICK_HASH=$($S log --oneline | head -1 | awk '{print $1}')

$S checkout main
$S cherry-pick "$PICK_HASH"
# Expected: "Cherry-picked ... as ..."
$S log --oneline | head -3  # should show cherry-pick commit

# 15.2 Revert
HEAD=$($S log --oneline | head -1 | awk '{print $1}')
$S revert "$HEAD" -m "Revert the cherry-pick"
# Expected: "Reverted: ..."

# 15.3 Rebase
$S checkout -b feature/rebase-test HEAD~3
echo "rebase content" > rebase-test.txt
$S add rebase-test.txt && $S commit "Feature: rebase test"
$S checkout main
$S checkout feature/rebase-test
$S rebase main
# Expected: "Rebase onto 'main': 1 patch(es) replayed"

# 15.4 Squash
$S checkout main
echo "sq1" > squash-test.txt
$S add squash-test.txt && $S commit "Squash part 1"
echo "sq2" >> squash-test.txt
$S add squash-test.txt && $S commit "Squash part 2"
$S squash 1 -m "Squashed commit"
$S log --oneline | head -3
# Should show 1 squashed commit instead of 2
```

**Expected:** All four operations succeed. Commits appear in correct order.

---

## 16. Tags, Notes, Reflog

```bash
# 16.1 Lightweight tag
$S tag v1.0.0
$S tag --list          # should show v1.0.0

# 16.2 Annotated tag
$S tag v1.1.0 --annotate --message "Release candidate"
$S tag --list          # should show v1.0.0 and v1.1.0 (annotated)

# 16.3 Delete tag
$S tag --delete v1.1.0

# 16.4 Notes
HEAD=$($S log --oneline | head -1 | awk '{print $1}')
$S notes add "$HEAD" -m "Test note"
$S notes list "$HEAD"  # should show: Note 0: Test note

# 16.5 Reflog
$S reflog | head -5   # should show HEAD movements with timestamps
```

**Expected:** Tags, notes, and reflog all work.

---

## 17. Move, Remove

```bash
# 17.1 Move/rename
echo "content" > mv-test.txt
$S add mv-test.txt && $S commit "Add file to rename"
$S mv mv-test.txt renamed.txt
$S status | grep -c "Deleted mv-test\|Added renamed"  # should be 2
$S add renamed.txt && $S commit "Rename file"
ls mv-test.txt 2>/dev/null     # should NOT exist
ls renamed.txt                  # should exist

# 17.2 Remove
$S rm renamed.txt
ls renamed.txt 2>/dev/null     # should NOT exist
$S commit "Remove file"

# 17.3 Remove --cached (keep file on disk)
echo "cached" > cached-test.txt
$S add cached-test.txt && $S commit "Add cached test"
$S rm --cached cached-test.txt
ls cached-test.txt              # should STILL exist
$S commit "Remove from index"
```

**Expected:** `mv` stages rename. `rm` deletes from disk. `rm --cached` only untracks.

---

## 18. Bisect

```bash
# Create a known-good and known-bad commit chain
echo "good" > bisect-test.txt
$S add bisect-test.txt && $S commit "good: v1"
echo "still good" >> bisect-test.txt
$S add bisect-test.txt && $S commit "good: v2"
echo "bug introduced" >> bisect-test.txt
$S add bisect-test.txt && $S commit "bad: v3"
echo "still broken" >> bisect-test.txt
$S add bisect-test.txt && $S commit "bad: v4"

# Get commit hashes (good is older, bad is newer)
GOOD=$($S log --oneline | sed -n '4p' | awk '{print $1}')
BAD=$($S log --oneline | head -1 | awk '{print $1}')

$S bisect start "$GOOD" "$BAD"
# Expected: bisect session started

$S bisect reset
# Expected: "Bisect reset"
```

**Expected:** Bisect starts and resets. (In real use, you'd run a test command at each step.)

---

## 19. Key Management

```bash
$S key generate              # Expected: "Generated keypair 'default'"
$S key list                 # Should show a public key
$S key public               # Should print the public key hex
ls .suture/keys/default.ed25519  # Should exist
```

**Expected:** Ed25519 keypair generated and listed.

---

## 20. Worktree

```bash
$S worktree add /tmp/suture-test/wt-demo -b wt-branch
# Expected: "Worktree 'wt-demo' created"

$S worktree list
# Should show main repo and wt-demo

# Verify worktree is a separate working directory
ls /tmp/suture-test/wt-demo/.suture/   # should exist (symlinked)
cd /tmp/suture-test/wt-demo
$S status | grep "wt-branch"          # should show correct branch

cd /tmp/suture-test/demo-repo
$S worktree remove wt-demo
# Expected: "Worktree 'wt-demo' removed"
```

**Expected:** Worktree created with separate branch, listed, and removed. **Note:** May show a non-fatal CAS warning during creation — this is a known cosmetic issue.

---

## 21. Garbage Collection & Integrity

```bash
# 21.1 GC
$S gc
# Expected: "Garbage collection complete" with count of removed patches

# 21.2 Integrity check
$S fsck
# Expected: "Repository integrity check complete" with check count
```

**Expected:** Both succeed. fsck may show warnings for non-UTF-8 payloads (benign).

---

## 22. Log Filtering

```bash
$S log --oneline --author "Tester" | head -3     # should filter by author
$S log --oneline --grep "merge"                   # should filter by message
$S log --oneline --first-parent | head -5          # should show first-parent chain only
$S log --oneline --all | head -5                   # should show commits across all branches
```

**Expected:** All filters work correctly.

---

## 23. Diff Between Commits

```bash
# 23.1 Diff between two commits using short hashes
A=$($S log --oneline | sed -n '3p' | awk '{print $1}')
B=$($S log --oneline | head -1 | awk '{print $1}')
$S diff --from "$A" --to "$B"
# Should show the diff between those two commits

# 23.2 Diff using branch names
$S diff --from main --to HEAD
# Should work (may be empty if HEAD == main)
```

**Expected:** Short hash prefixes work (8-char). This was a bug in v0.8.0, fixed in v0.8.1.

---

## 24. Shell Completions

```bash
$S completions bash | head -3    # should output bash completion script
$S completions zsh | head -3     # should output zsh completion script
$S completions fish | head -3    # should output fish completion script
$S completions powershell | head -3  # should output powershell completion script
$S completions nushell | head -3  # should output nushell completion script
```

**Expected:** All 5 shell completion formats generate valid output.

---

## 25. Misc

```bash
$S version          # should print: suture 0.8.1
$S drivers          # should list 9 drivers with extensions
$S shortlog         # should show compact commit summary
```

---

## Summary Checklist

| # | Feature | Status |
|---|---------|--------|
| 1 | Init & Config | [ ] |
| 2 | Ignore patterns | [ ] |
| 3 | Basic workflow (add/commit/status/log/diff) | [ ] |
| 4 | Branching (create/list/checkout/protect/delete/target) | [ ] |
| 5 | Semantic merge — JSON | [ ] |
| 6 | Semantic merge — CSV | [ ] |
| 7 | Semantic merge — TOML | [ ] |
| 8 | Semantic merge — XML | [ ] |
| 9 | Semantic merge — Markdown | [ ] |
| 10 | Plain text conflict | [ ] |
| 11 | Fast-forward merge | [ ] |
| 12 | Stash (push/pop/apply/drop/list) | [ ] |
| 13 | Undo (soft reset) | [ ] |
| 14 | Reset modes (hard/mixed) | [ ] |
| 15 | Cherry-pick, Revert, Rebase, Squash | [ ] |
| 16 | Tags, Notes, Reflog | [ ] |
| 17 | Move, Remove, Remove --cached | [ ] |
| 18 | Bisect | [ ] |
| 19 | Key management | [ ] |
| 20 | Worktree | [ ] |
| 21 | GC & fsck | [ ] |
| 22 | Log filtering | [ ] |
| 23 | Diff between commits | [ ] |
| 24 | Shell completions | [ ] |
| 25 | Version, drivers, shortlog | [ ] |
