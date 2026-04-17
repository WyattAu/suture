# CLI Reference

Global flag: `-C <path>` -- run as if started in `<path>`.

---

## Repository

### `suture init [path]`

Initialize a new repository. Creates a `.suture/` directory.

```bash
suture init                # current directory
suture init my-project     # new directory
```

### `suture status`

Show working tree status (branch, staged files, patch count).

```bash
suture status
```

### `suture config [key[=value]]`

Get or set configuration values.

```bash
suture config                          # list all config
suture config user.name                # get a value
suture config user.name=Alice          # set a value
suture config --global user.name=Alice # set global config
```

Flag | Description
-----|------------
`--global` | Operate on `~/.config/suture/config.toml` instead of repo config

---

## Staging

### `suture add <path>...`

Stage files for the next commit.

```bash
suture add file.txt        # stage a file
suture add src/            # stage a directory
suture add --all           # stage all modified/deleted files
suture add -p              # interactively choose files
```

Flag | Description
-----|------------
`-a`, `--all` | Stage all changes (respects `.sutureignore`)
`-p`, `--patch` | Interactive file selection

### `suture rm <path>...`

Remove files from the working tree and staging area.

```bash
suture rm file.txt         # remove from tree and staging
suture rm --cached file    # remove from staging only
```

Flag | Description
-----|------------
`--cached` | Keep the file on disk, remove from staging only

### `suture mv <src> <dst>`

Move or rename a tracked file.

```bash
suture mv old.txt new.txt  # rename
suture mv file dir/        # move into directory
```

---

## Commits

### `suture commit <message>`

Commit staged changes.

```bash
suture commit "fix typo"
suture commit -a "update all"   # auto-stage and commit
```

Flag | Description
-----|------------
`-a`, `--all` | Auto-stage all modified/deleted files before committing

### `suture log [branch]`

Show commit history.

```bash
suture log                         # HEAD history
suture log --oneline               # compact format
suture log --graph                 # ASCII graph
suture log --all                   # all branches
suture log --author alice          # filter by author
suture log --grep "fix"            # filter by message
suture log --since "2 weeks ago"   # filter by date
```

Flag | Description
-----|------------
`--graph` | Show ASCII graph of branch topology
`--oneline` | Compact one-line format
`--all` | Show commits across all branches
`--author` | Filter by author name
`--grep` | Filter by commit message pattern
`--since` | Show commits newer than date
`--until` | Show commits older than date
`--first-parent` | Show only the first-parent chain

### `suture diff`

Show differences between commits or the working tree.

```bash
suture diff                        # working tree vs staging
suture diff --cached               # staging vs HEAD
suture diff --from main            # compare main to working tree
suture diff --from main --to feat  # compare two refs
```

Flag | Description
-----|------------
`--from` | From ref (commit hash or branch). Omit for HEAD.
`--to` | To ref (commit hash or branch). Omit for working tree.
`--cached` | Show staged changes (staging vs HEAD)

### `suture blame <path>`

Show per-line commit attribution.

```bash
suture blame src/main.rs
```

### `suture show <commit>`

Show detailed information about a commit.

```bash
suture show HEAD
suture show abc123
```

---

## Branches

### `suture branch [name]`

List, create, delete, or protect branches.

```bash
suture branch                      # list branches
suture branch feature              # create branch
suture branch feature main         # create from specific target
suture branch -d old-branch        # delete
suture branch --protect main       # protect from force-push
suture branch --unprotect main     # unprotect
```

Flag | Description
-----|------------
`-t`, `--target` | Start branch from this target
`-d`, `--delete` | Delete a branch
`--list` | List branches (explicit)
`--protect` | Protect branch from force-push/deletion
`--unprotect` | Unprotect a branch

### `suture checkout <branch>`

Switch to a different branch.

```bash
suture checkout main               # switch branch
suture checkout -b feature         # create and switch
suture checkout -b feat main       # create from main
```

Flag | Description
-----|------------
`-b` | Create a new branch before switching

### `suture merge <branch>`

Merge a branch into the current branch.

```bash
suture merge feature               # merge into current
suture merge --dry-run feature     # preview without applying
```

Flag | Description
-----|------------
`--dry-run` | Preview merge without modifying the working tree

---

## Advanced

### `suture rebase <target>`

Rebase the current branch onto another branch.

```bash
suture rebase main                 # rebase onto main
suture rebase -i main              # interactive rebase
suture rebase --abort              # abort in-progress rebase
suture rebase --continue           # continue after resolving
```

Flag | Description
-----|------------
`-i`, `--interactive` | Open editor to reorder/edit/squash commits
`--continue` | Continue an in-progress interactive rebase
`--abort` | Abort an in-progress interactive rebase

### `suture cherry-pick <commit>`

Apply a specific commit onto the current branch.

```bash
suture cherry-pick abc123
```

### `suture revert <commit>`

Revert a commit by creating a new patch that undoes it.

```bash
suture revert abc123
suture revert abc123 -m "revert fix"
```

Flag | Description
-----|------------
`-m` | Custom revert message

### `suture stash`

Stash changes away and re-apply them later.

```bash
suture stash push                  # stash current changes
suture stash push -m "WIP"         # with a message
suture stash pop                   # apply and remove latest stash
suture stash apply 0               # apply specific stash
suture stash list                  # list stashes
suture stash drop 0                # discard specific stash
```

### `suture reset <target>`

Reset HEAD to a specific commit.

```bash
suture reset HEAD~1                # mixed (default)
suture reset abc123 --soft         # keep changes staged
suture reset abc123 --hard         # discard all changes
```

Flag | Description
-----|------------
`--soft` | Keep changes staged
`--mixed` | Unstage changes, keep in working tree (default)
`--hard` | Discard all changes

### `suture undo [steps]`

Soft reset to undo recent commits.

```bash
suture undo                        # undo last commit
suture undo --steps 3              # undo last 3 commits
```

### `suture squash <count>`

Squash the last N commits into one.

```bash
suture squash 3                    # squash last 3 commits
suture squash 3 -m "combined"      # with custom message
```

---

## Tags

### `suture tag [name]`

Create, list, or delete tags.

```bash
suture tag                         # list tags
suture tag v1.0                    # lightweight tag
suture tag -a v1.0 -m "release"    # annotated tag
suture tag -d v0.9                 # delete
```

Flag | Description
-----|------------
`-a`, `--annotate` | Create annotated tag
`-m` | Tag message (used with `--annotate`)
`-t`, `--target` | Target commit/branch (default: HEAD)
`-d`, `--delete` | Delete a tag

---

## Remotes

### `suture remote`

Manage remote Hub connections.

```bash
suture remote add origin http://localhost:50051
suture remote list
suture remote remove upstream
suture remote login                 # authenticate with origin
suture remote mirror http://upstream/repo upstream-name
```

### `suture push`

Push patches to a remote Hub.

```bash
suture push                        # push all branches to origin
suture push --force                # force push
suture push origin feature         # push specific branch
```

Flag | Description
-----|------------
`--force` | Skip fast-forward check
`<remote>` | Remote name (default: `origin`)
`<branch>` | Specific branch to push

### `suture pull`

Pull patches from a remote Hub.

```bash
suture pull                        # pull and merge from origin
suture pull --rebase               # pull with rebase
suture pull upstream               # pull from specific remote
```

Flag | Description
-----|------------
`--rebase` | Rebase local commits on top of fetched history

### `suture fetch`

Fetch patches without merging.

```bash
suture fetch                       # fetch from origin
suture fetch --depth 5             # shallow fetch last 5 commits
```

Flag | Description
-----|------------
`--depth` | Limit fetch to last N commits

### `suture clone <url> [dir]`

Clone a repository from a remote Hub.

```bash
suture clone http://localhost:50051/my-repo
suture clone http://localhost:50051/my-repo my-local-dir
suture clone --depth 10 http://localhost:50051/my-repo
```

Flag | Description
-----|------------
`--depth` | Shallow clone: fetch only the last N patches

---

## Notes

### `suture notes`

Attach notes to commits.

```bash
suture notes add abc123 -m "reviewed"    # add note
suture notes add abc123                 # enter note interactively
suture notes list abc123                # list notes
suture notes show abc123                # show notes (alias for list)
suture notes remove abc123 0            # remove note by index
```

---

## Maintenance

### `suture gc`

Garbage collect unreachable objects.

```bash
suture gc
```

### `suture fsck`

Verify repository integrity.

```bash
suture fsck
```

### `suture key`

Signing key management.

```bash
suture key generate           # generate default Ed25519 keypair
suture key generate deploy    # generate named key
suture key list               # list local public keys
suture key public             # show default public key
suture key public deploy      # show named key
```

---

## Other

### `suture version`

Show version information.

### `suture tui`

Launch the terminal UI (7 tabs: Status, Log, Staging, Diff, Branches, Remote, Help).

### `suture bisect`

Binary search for bug-introducing commit.

```bash
suture bisect start abc123 def456   # abc123=good, def456=bad
suture bisect run abc123 def456 -- cargo test  # automated
suture bisect reset                  # cancel session
```

### `suture reflog`

Show the reference log (HEAD movements).

### `suture shortlog`

Compact commit summary grouped by author.

```bash
suture shortlog
suture shortlog -n 10
```

### `suture drivers`

List available semantic merge drivers.

### `suture merge-file`

Standalone three-way file merge (no repo required).

```bash
suture merge-file base.txt ours.txt theirs.txt
suture merge-file --driver json base.json ours.json theirs.json
suture merge-file -o merged.yaml base.yaml ours.yaml theirs.yaml
```

Flag | Description
-----|------------
`--driver` | Semantic driver (json, yaml, toml, csv, xml, markdown, docx, xlsx, pptx). Auto-detected if omitted.
`-o`, `--output` | Write result to file instead of stdout
`--label-ours` | Label for ours side in conflict markers
`--label-theirs` | Label for theirs side in conflict markers

### `suture worktree`

Manage working trees.

```bash
suture worktree add ../feature         # create worktree
suture worktree add hotfix -b fix      # create on new branch
suture worktree list                   # list all worktrees
suture worktree remove feature         # remove worktree
```

### `suture ignore`

Inspect `.sutureignore` patterns.

```bash
suture ignore list         # list ignore patterns
suture ignore check foo.o  # check if path is ignored
```

### `suture completions <shell>`

Generate shell completions.

```bash
suture completions bash > ~/.bash_completion.d/suture
suture completions zsh > ~/.zfunc/_suture
suture completions fish > ~/.config/fish/completions/suture.fish
suture completions nushell | save -f ~/.cache/suture/completions.nu
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `nushell`.
