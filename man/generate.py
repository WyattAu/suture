#!/usr/bin/env python3
"""Generate man pages for all suture commands from help text."""
import os
import re

DATE = "April 2026"
VERSION = "v5.0.0"
MAN_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)))

def esc(s):
    s = s.replace('\\', '\\\\')
    s = s.replace('-', '\\-')
    return s

def write_main_page():
    path = os.path.join(MAN_DIR, 'suture.1')
    commands = [
        ("init", "Initialize a new Suture repository"),
        ("status", "Show repository status"),
        ("ignore", "Inspect .sutureignore patterns"),
        ("add", "Add files to the staging area"),
        ("rm", "Remove files from the working tree and staging area"),
        ("commit", "Create a commit"),
        ("branch", "Branch operations"),
        ("log", "Show commit history"),
        ("checkout", "Switch to a different branch"),
        ("switch", "Switch to a different branch (modern alternative to checkout)"),
        ("restore", "Restore working tree files"),
        ("mv", "Move or rename a tracked file"),
        ("diff", "Show differences between commits or branches"),
        ("revert", "Revert a commit"),
        ("merge", "Merge a branch into the current branch"),
        ("merge-file", "Perform three-way file merge"),
        ("apply", "Apply a patch/diff file to the working tree"),
        ("cherry-pick", "Apply a specific commit onto the current branch"),
        ("rebase", "Rebase the current branch onto another branch"),
        ("blame", "Show per-line commit attribution for a file"),
        ("tag", "Tag operations"),
        ("config", "Get or set configuration values"),
        ("remote", "Remote operations"),
        ("push", "Push patches to a remote Hub"),
        ("pull", "Pull patches from a remote Hub"),
        ("fetch", "Fetch patches from a remote Hub without merging"),
        ("ls-remote", "List references on a remote Hub"),
        ("clone", "Clone a repository from a remote Hub"),
        ("reset", "Reset HEAD to a specific commit"),
        ("key", "Signing key management"),
        ("stash", "Stash management"),
        ("completions", "Generate shell completions"),
        ("show", "Show detailed information about a commit"),
        ("reflog", "Show the reference log (HEAD movements)"),
        ("drivers", "List available semantic drivers"),
        ("shortlog", "Show compact commit summary grouped by author"),
        ("notes", "Manage commit notes"),
        ("worktree", "Manage working trees"),
        ("version", "Show version information"),
        ("verify", "Verify commit signatures"),
        ("gc", "Garbage collect unreachable objects"),
        ("grep", "Search for a pattern in tracked files"),
        ("fsck", "Verify repository integrity"),
        ("doctor", "Check repository health and configuration"),
        ("audit", "Inspect the tamper-evident audit log"),
        ("clean", "Remove untracked files from the working tree"),
        ("describe", "Describe a commit using the nearest tag"),
        ("rev-parse", "Parse revision names to hashes"),
        ("git", "Interact with Git repositories"),
        ("bisect", "Binary search for bug-introducing commit"),
        ("hook", "Manage repository hooks"),
        ("classification", "Bulk classification marking scanning and compliance reporting"),
        ("undo", "Undo the last operation"),
        ("squash", "Squash N commits into one"),
        ("sync", "Synchronize with remote"),
        ("tui", "Launch terminal UI"),
        ("export", "Export a clean snapshot without repository metadata"),
        ("report", "Generate reports about the repository"),
        ("batch", "Batch operations for managing multiple files or clients"),
        ("timeline", "OTIO timeline operations"),
        ("archive", "Create an archive of the repository"),
    ]
    lines = []
    lines.append(f'.TH SUTURE "1" "{DATE}" "Suture {VERSION}" "User Commands"')
    lines.append('.SH NAME')
    lines.append('suture \\- Universal Semantic Version Control System')
    lines.append('.SH SYNOPSIS')
    lines.append('.B suture')
    lines.append('[\\fIoptions\\fR] \\fIcommand\\fR [\\fIargs\\fR]')
    lines.append('.SH DESCRIPTION')
    lines.append('Suture is a universal semantic version control system that supports')
    lines.append('video (OTIO), document (DOCX/XLSX/PPTX), and data (CSV/JSON/XML/YAML)')
    lines.append('workflows with content-aware semantic diffing, merging, and integrity')
    lines.append('verification.')
    lines.append('.SH COMMANDS')
    for name, desc in commands:
        lines.append('.TP')
        lines.append(f'.B {esc(name)}')
        lines.append(esc(desc))
    lines.append('.SH OPTIONS')
    lines.append('.TP')
    lines.append('.BR \\-C " " \\fI<REPO_PATH>\\fR')
    lines.append('Run as if suture was started in <path>.')
    lines.append('.TP')
    lines.append('.BR \\-h ", " \\-\\-help')
    lines.append('Print help.')
    lines.append('.TP')
    lines.append('.BR \\-V ", " \\-\\-version')
    lines.append('Print version.')
    lines.append('.SH SEE ALSO')
    lines.append('.BR suture-init (1),')
    lines.append('.BR suture-add (1),')
    lines.append('.BR suture-commit (1),')
    lines.append('.BR suture-log (1),')
    lines.append('.BR suture-diff (1)')
    with open(path, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print(f"Generated {path}")

def write_page(cmd, description, usage, options, examples=None, subcommands=None):
    man_name = f"suture-{cmd}"
    upper = cmd.upper().replace('-', '_')
    path = os.path.join(MAN_DIR, f"{man_name}.1")
    lines = []
    lines.append(f'.TH SUTURE-{upper} "1" "{DATE}" "Suture {VERSION}" "User Commands"')
    lines.append('.SH NAME')
    lines.append(f'{esc(man_name)} \\- {esc(description)}')
    lines.append('.SH SYNOPSIS')
    lines.append(f'.B {esc(usage)}')
    lines.append('.SH DESCRIPTION')
    lines.append(esc(description) + '.')
    if subcommands:
        lines.append('.SH SUBCOMMANDS')
        for sc_name, sc_desc in subcommands:
            lines.append('.TP')
            lines.append(f'.B {esc(sc_name)}')
            lines.append(esc(sc_desc))
    lines.append('.SH OPTIONS')
    for flags, desc in options:
        lines.append('.TP')
        lines.append(f'.BR {esc(flags)}')
        lines.append(esc(desc))
    if examples:
        lines.append('.SH EXAMPLES')
        lines.append('.nf')
        for ex in examples:
            lines.append(esc(ex))
        lines.append('.fi')
    lines.append('.SH SEE ALSO')
    lines.append('.BR suture (1)')
    with open(path, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print(f"Generated {path}")

C = 'Run as if suture was started in <path>'
H = 'Print help'

write_page('init', 'Initialize a new Suture repository',
    'suture init [\\fIoptions\\fR] [\\fIpath\\fR]',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-t, \\-\\-type \\fI<type>\\fR', 'Repository type: video, document, or data (default: auto-detect)'),
        ('\\-\\-template \\fI<template>\\fR', 'Repository template to bootstrap from (video, document, data, report)'),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture init                         # Initialize in current directory',
        'suture init my-project              # Initialize in a new directory',
        'suture init \\-\\-type video            # Configure for video workflows',
        'suture init \\-\\-type document         # Configure for document workflows',
        'suture init \\-\\-type data             # Configure for data workflows',
    ])

write_page('status', 'Show repository status',
    'suture status [\\fIoptions\\fR]',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=['suture status              # Show working tree status'])

write_page('ignore', 'Inspect .sutureignore patterns',
    'suture ignore [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('list', 'List current ignore patterns'),
        ('check', 'Check if a path matches any ignore pattern'),
    ],
    examples=[
        'suture ignore list         # List ignore patterns',
        'suture ignore check foo.o  # Check if a path is ignored',
    ])

write_page('add', 'Add files to the staging area',
    'suture add [\\fIoptions\\fR] [\\fIpaths\\fR]...',
    [
        ('\\-a, \\-\\-all', 'Add all files (respecting .sutureignore)'),
        ('\\-p, \\-\\-patch', 'Interactively review and choose which files to stage'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture add file.txt        # Stage a specific file',
        'suture add src/            # Stage all files in src/',
        'suture add \\-\\-all           # Stage all modified/deleted files',
        'suture add \\-p              # Interactively choose which files to stage',
    ])

write_page('rm', 'Remove files from the working tree and staging area',
    'suture rm [\\fIoptions\\fR] [\\fIpaths\\fR]...',
    [
        ('\\-c, \\-\\-cached', 'Only remove from staging area, keep the file on disk'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture rm file.txt         # Remove file from tree and staging',
        'suture rm \\-\\-cached file    # Remove from staging only, keep on disk',
    ])

write_page('commit', 'Create a commit',
    'suture commit [\\fIoptions\\fR] \\fImessage\\fR',
    [
        ('\\-a, \\-\\-all', 'Auto-stage all modified/deleted files before committing'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture commit "fix typo"    # Commit staged changes',
        'suture commit \\-a "update"  # Auto-stage all and commit',
        'suture commit \\-\\-all "WIP"  # Same as above',
    ])

write_page('branch', 'Branch operations',
    'suture branch [\\fIoptions\\fR] [\\fIname\\fR]',
    [
        ('\\-t, \\-\\-target \\fI<target>\\fR', 'Start branch from this target (branch name or HEAD)'),
        ('\\-d, \\-\\-delete', 'Delete a branch'),
        ('\\-l, \\-\\-list', 'List branches'),
        ('\\-\\-protect', 'Protect a branch from force-push/deletion'),
        ('\\-\\-unprotect', 'Unprotect a branch'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture branch              # List branches',
        'suture branch \\-\\-list       # List branches (explicit)',
        'suture branch feature      # Create branch "feature"',
        'suture branch \\-d old-branch  # Delete a branch',
        'suture branch \\-\\-protect main   # Protect "main" from force-push',
        'suture branch \\-\\-unprotect main # Unprotect "main"',
    ])

write_page('log', 'Show commit history',
    'suture log [\\fIoptions\\fR] [\\fIbranch\\fR]',
    [
        ('\\-g, \\-\\-graph', 'Show ASCII graph of branch topology'),
        ('\\-\\-first-parent', 'Show only the first-parent chain (skip merge parents)'),
        ('\\-\\-oneline', 'Show compact one-line format'),
        ('\\-\\-author \\fI<author>\\fR', 'Filter by author name'),
        ('\\-\\-grep \\fI<pattern>\\fR', 'Filter by commit message pattern'),
        ('\\-\\-all', 'Show commits across all branches'),
        ('\\-\\-since \\fI<date>\\fR', 'Show commits newer than a date/time'),
        ('\\-\\-until \\fI<date>\\fR', 'Show commits older than a date/time'),
        ('\\-\\-stat', 'Show which files changed in each commit'),
        ('\\-\\-diff', 'Show patch content (diff) for each commit'),
        ('\\-\\-audit', 'Export structured audit trail (compliance format)'),
        ('\\-\\-audit-format \\fI<format>\\fR', 'Output format for --audit (json, csv, text)'),
        ('\\-\\-verify', 'Verify commit signatures'),
        ('\\-\\-diff-filter \\fI<filter>\\fR', 'Filter by diff status: A (added), D (deleted), M (modified)'),
        ('\\-\\-limit \\fI<n>\\fR', 'Limit number of commits shown (default: 100, 0 = unlimited)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture log                 # Show log for HEAD',
        'suture log \\-\\-oneline       # Compact one-line format',
        'suture log \\-\\-graph         # ASCII graph of branch topology',
        'suture log \\-\\-all           # Show commits across all branches',
        'suture log \\-\\-author alice  # Filter by author',
        'suture log \\-\\-grep "fix"   # Filter by message pattern',
        'suture log \\-\\-audit         # Export structured audit trail',
    ])

write_page('checkout', 'Switch to a different branch',
    'suture checkout [\\fIoptions\\fR] [\\fIbranch\\fR]',
    [
        ('\\-b, \\-\\-new-branch \\fI<name>\\fR', 'Create a new branch before switching'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture checkout main       # Switch to "main" branch',
        'suture checkout \\-b feature # Create and switch to "feature"',
        'suture checkout \\-b feat main  # Create "feat" from "main"',
    ])

write_page('switch', 'Switch to a different branch (modern alternative to checkout)',
    'suture switch [\\fIoptions\\fR] [\\fIbranch\\fR]',
    [
        ('\\-c, \\-\\-create \\fI<name>\\fR', 'Create a new branch before switching'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture switch main          # Switch to "main"',
        'suture switch \\-c feature   # Create and switch to "feature"',
        'suture switch \\-c feat main # Create "feat" from "main"',
    ])

write_page('restore', 'Restore working tree files',
    'suture restore [\\fIoptions\\fR] [\\fIpaths\\fR]...',
    [
        ('\\-s, \\-\\-source \\fI<commit>\\fR', 'Restore from a specific commit (default: HEAD)'),
        ('\\-\\-staged', 'Restore staged files (unstage)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture restore file.txt          # Restore file from HEAD',
        'suture restore \\-\\-staged file.txt # Unstage a file',
        'suture restore \\-\\-source HEAD~2 file.txt  # Restore from specific commit',
    ])

write_page('mv', 'Move or rename a tracked file',
    'suture mv [\\fIoptions\\fR] \\fIsource\\fR \\fIdestination\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture mv old.txt new.txt  # Rename a file',
        'suture mv file dir/        # Move file into directory',
    ])

write_page('diff', 'Show differences between commits or branches',
    'suture diff [\\fIoptions\\fR]',
    [
        ('\\-f, \\-\\-from \\fI<ref>\\fR', 'From ref (commit hash or branch name). Omit for HEAD'),
        ('\\-t, \\-\\-to \\fI<ref>\\fR', 'To ref (commit hash or branch name). Omit for working tree'),
        ('\\-\\-cached', 'Show staged changes (diff of staging area vs HEAD)'),
        ('\\-\\-integrity', 'Show supply chain integrity analysis (entropy, risk indicators)'),
        ('\\-\\-name-only', 'Show only names of changed files'),
        ('\\-\\-classification', 'Detect classification marking changes (defence/compliance)'),
        ('\\-\\-summary', 'Human-readable change summary (no diff output)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture diff                              # Working tree vs staging area',
        'suture diff \\-\\-cached                     # Staging area vs HEAD',
        'suture diff \\-\\-from main \\-\\-to feature     # Compare two branches',
        'suture diff \\-\\-integrity                  # Supply chain integrity analysis',
        'suture diff \\-\\-summary                    # Human-readable change summary',
    ])

write_page('revert', 'Revert a commit',
    'suture revert [\\fIoptions\\fR] \\fIcommit\\fR',
    [
        ('\\-m, \\-\\-message \\fI<msg>\\fR', 'Custom revert message'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture revert abc123       # Revert a commit',
        'suture revert abc123 \\-m "revert fix"  # With custom message',
    ])

write_page('merge', 'Merge a branch into the current branch',
    'suture merge [\\fIoptions\\fR] \\fIsource\\fR',
    [
        ('\\-\\-dry-run', 'Preview merge without modifying the working tree'),
        ('\\-s, \\-\\-strategy \\fI<strategy>\\fR', 'Conflict resolution strategy: semantic, ours, theirs, manual (default: semantic)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture merge feature       # Merge feature into current branch',
        'suture merge \\-\\-dry-run feature  # Preview merge',
        'suture merge \\-s ours feature    # Auto-resolve by keeping our version',
        'suture merge \\-s theirs feature  # Auto-resolve by keeping their version',
    ])

write_page('merge-file', 'Perform three-way file merge',
    'suture merge-file [\\fIoptions\\fR] \\fIbase\\fR \\fIours\\fR \\fItheirs\\fR',
    [
        ('\\-o, \\-\\-output \\fI<path>\\fR', 'Write merged result to a file instead of stdout'),
        ('\\-\\-driver \\fI<driver>\\fR', 'Use a semantic merge driver (json, yaml, toml, csv, xml, markdown, docx, xlsx, pptx)'),
        ('\\-\\-label-ours \\fI<label>\\fR', 'Label for "ours" side in conflict markers'),
        ('\\-\\-label-theirs \\fI<label>\\fR', 'Label for "theirs" side in conflict markers'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture merge-file base.txt ours.txt theirs.txt',
        'suture merge-file \\-\\-driver json base.json ours.json theirs.json',
        'suture merge-file \\-\\-driver yaml \\-o merged.yaml base.yaml ours.yaml theirs.yaml',
    ])

write_page('apply', 'Apply a patch/diff file to the working tree',
    'suture apply [\\fIoptions\\fR] \\fIpatch-file\\fR',
    [
        ('\\-R, \\-\\-reverse', 'Apply in reverse'),
        ('\\-\\-stat', 'Show summary instead of applying'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture apply fix.patch        # Apply a patch file',
        'suture apply \\-R fix.patch     # Apply in reverse',
        'suture apply \\-\\-stat fix.patch # Show summary only',
    ])

write_page('cherry-pick', 'Apply a specific commit onto the current branch',
    'suture cherry-pick [\\fIoptions\\fR] \\fIcommit\\fR',
    [
        ('\\-n, \\-\\-no-commit', 'Apply changes to working tree and staging area without committing'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture cherry-pick abc123  # Apply commit onto current branch',
        'suture cherry-pick \\-n abc123  # Apply without committing',
    ])

write_page('rebase', 'Rebase the current branch onto another branch',
    'suture rebase [\\fIoptions\\fR] \\fIbranch\\fR',
    [
        ('\\-i, \\-\\-interactive', 'Interactive rebase -- open editor to reorder/edit/squash commits'),
        ('\\-\\-resume, \\-\\-continue', 'Continue an in-progress interactive rebase'),
        ('\\-\\-abort', 'Abort an in-progress interactive rebase'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture rebase main         # Rebase current branch onto "main"',
        'suture rebase \\-i main      # Interactive rebase onto "main"',
        'suture rebase \\-\\-abort      # Abort an in-progress rebase',
    ])

write_page('blame', 'Show per-line commit attribution for a file',
    'suture blame [\\fIoptions\\fR] \\fIpath\\fR',
    [
        ('\\-\\-at \\fI<commit>\\fR', 'Blame as of a specific commit (default: HEAD)'),
        ('\\-L, \\-\\-lines \\fI<range>\\fR', 'Only show lines in range (e.g., -L 10,20)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture blame src/main.rs          # Show line-by-line attribution at HEAD',
        'suture blame src/main.rs \\-\\-at HEAD~3  # Show attribution as of HEAD~3',
    ])

write_page('tag', 'Tag operations',
    'suture tag [\\fIoptions\\fR] [\\fIname\\fR]',
    [
        ('\\-t, \\-\\-target \\fI<target>\\fR', 'Target commit/branch (default: HEAD)'),
        ('\\-d, \\-\\-delete', 'Delete a tag'),
        ('\\-l, \\-\\-list', 'List tags'),
        ('\\-a, \\-\\-annotate', 'Create an annotated tag with a message'),
        ('\\-m, \\-\\-message \\fI<msg>\\fR', 'Tag message (required with --annotate)'),
        ('\\-\\-sort \\fI<field>\\fR', 'Sort tags by "date" (newest first) or "name" (alphabetical)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture tag                 # List tags',
        'suture tag v1.0            # Create lightweight tag',
        'suture tag v1.0 \\-m "release 1.0"  # Create annotated tag',
        'suture tag \\-d v0.9         # Delete a tag',
    ])

write_page('config', 'Get or set configuration values',
    'suture config [\\fIoptions\\fR] [\\fIkey=value\\fR]...',
    [
        ('\\-\\-global', 'Operate on the global config (~/.config/suture/config.toml) instead of the repo config'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture config              # List all config',
        'suture config user.name    # Get a config value',
        'suture config user.name=Alice  # Set a config value',
        'suture config \\-\\-global user.name=Alice  # Set global config',
    ])

write_page('remote', 'Remote operations',
    'suture remote [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('add', 'Add a remote Hub'),
        ('list', 'List configured remotes'),
        ('remove', 'Remove a configured remote'),
        ('rename', 'Rename a remote'),
        ('login', 'Authenticate with a remote Hub and store a token'),
        ('mirror', 'Mirror a remote repository locally'),
    ])

write_page('push', 'Push patches to a remote Hub',
    'suture push [\\fIoptions\\fR] [\\fIremote\\fR] [\\fIbranch\\fR]',
    [
        ('\\-\\-force', 'Force push even if not fast-forward'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture push                # Push all branches to origin',
        'suture push \\-\\-force        # Force push (skip fast-forward check)',
        'suture push origin feature # Push only "feature" branch to "origin"',
    ])

write_page('pull', 'Pull patches from a remote Hub',
    'suture pull [\\fIoptions\\fR] [\\fIremote\\fR]',
    [
        ('\\-\\-rebase', 'Rebase local commits on top of fetched remote history'),
        ('\\-\\-autostash', 'Stash changes before pulling and pop after'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture pull                # Pull and merge from origin',
        'suture pull \\-\\-rebase       # Pull with rebase',
        'suture pull upstream       # Pull from a specific remote',
    ])

write_page('fetch', 'Fetch patches from a remote Hub without merging',
    'suture fetch [\\fIoptions\\fR] [\\fIremote\\fR]',
    [
        ('\\-\\-depth \\fI<n>\\fR', 'Limit fetch to the last N commits'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture fetch               # Fetch from origin',
        'suture fetch \\-\\-depth 5     # Shallow fetch last 5 commits',
    ])

write_page('ls-remote', 'List references (branches) on a remote Hub',
    'suture ls-remote [\\fIoptions\\fR] \\fIremote-or-url\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture ls-remote http://localhost:50051/my-repo',
        'suture ls-remote origin',
    ])

write_page('clone', 'Clone a repository from a remote Hub',
    'suture clone [\\fIoptions\\fR] \\fIurl\\fR [\\fIdir\\fR]',
    [
        ('\\-d, \\-\\-depth \\fI<n>\\fR', 'Shallow clone: fetch only the last N patches'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture clone http://localhost:50051/my-repo',
        'suture clone http://localhost:50051/my-repo my-local-dir',
        'suture clone \\-\\-depth 10 http://localhost:50051/my-repo',
    ])

write_page('reset', 'Reset HEAD to a specific commit',
    'suture reset [\\fIoptions\\fR] \\fItarget\\fR',
    [
        ('\\-m, \\-\\-mode \\fI<mode>\\fR', 'Reset mode: soft, mixed, hard (default: mixed)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture reset HEAD~1        # Reset to parent (mixed mode)',
        'suture reset abc123 \\-\\-soft  # Keep changes staged',
        'suture reset abc123 \\-\\-hard  # Discard all changes',
    ])

write_page('key', 'Signing key management',
    'suture key [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('generate', 'Generate a new Ed25519 keypair'),
        ('list', 'List local signing keys (public keys)'),
        ('public', 'Show the public key for a named key'),
    ])

write_page('stash', 'Stash management',
    'suture stash [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('push', 'Stash current changes'),
        ('save', 'Stash current changes (alias for push)'),
        ('pop', 'Pop the most recent stash'),
        ('apply', 'Apply a specific stash'),
        ('list', 'List stashes'),
        ('drop', 'Drop a specific stash'),
        ('branch', 'Create and checkout a new branch from a stash entry'),
        ('show', 'Show stash contents'),
        ('clear', 'Drop all stash entries'),
    ])

write_page('completions', 'Generate shell completions',
    'suture completions [\\fIoptions\\fR] \\fIshell\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture completions bash > ~/.bash_completion.d/suture',
        'suture completions zsh > ~/.zfunc/_suture',
        'suture completions fish > ~/.config/fish/completions/suture.fish',
        'suture completions nushell | save \\-f ~/.cache/suture/completions.nu',
    ])

write_page('show', 'Show detailed information about a commit',
    'suture show [\\fIoptions\\fR] \\fIcommit\\fR',
    [
        ('\\-\\-stat', 'Show file change statistics'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture show HEAD           # Show HEAD commit',
        'suture show abc123         # Show specific commit',
        'suture show \\-\\-stat HEAD    # Show with file change statistics',
    ])

write_page('reflog', 'Show the reference log (HEAD movements)',
    'suture reflog [\\fIoptions\\fR]',
    [
        ('\\-\\-show', 'Show full patch details for each reflog entry'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('drivers', 'List available semantic drivers',
    'suture drivers [\\fIoptions\\fR]',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('shortlog', 'Show compact commit summary grouped by author',
    'suture shortlog [\\fIoptions\\fR] [\\fIbranch\\fR]',
    [
        ('\\-n, \\-\\-number \\fI<n>\\fR', 'Number of commits to show (default: all)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('notes', 'Manage commit notes',
    'suture notes [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('add', 'Add a note to a commit'),
        ('list', 'List notes for a commit'),
        ('show', 'Show notes for a commit (alias for list)'),
        ('remove', 'Remove a note from a commit'),
    ])

write_page('worktree', 'Manage working trees',
    'suture worktree [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('add', 'Create a new worktree'),
        ('list', 'List all worktrees'),
        ('remove', 'Remove a worktree'),
        ('prune', 'Prune worktree entries whose directories no longer exist'),
    ],
    examples=[
        'suture worktree add ../feature   # Create worktree at ../feature',
        'suture worktree add hotfix \\-b fix   # Create "hotfix" on new branch "fix"',
        'suture worktree list             # List all worktrees',
        'suture worktree remove feature   # Remove worktree "feature"',
    ])

write_page('version', 'Show version information',
    'suture version [\\fIoptions\\fR]',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('verify', 'Verify commit signatures',
    'suture verify [\\fIoptions\\fR] [\\fIcommit-ref\\fR]',
    [
        ('\\-v, \\-\\-verbose', 'Show key details (author, fingerprint, patch id)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture verify             # Verify HEAD commit signature',
        'suture verify abc123      # Verify specific commit',
        'suture verify \\-v HEAD     # Show key details',
    ])

write_page('gc', 'Garbage collect unreachable objects',
    'suture gc [\\fIoptions\\fR]',
    [
        ('\\-\\-dry-run', 'Show what would be pruned without actually deleting'),
        ('\\-\\-aggressive', 'Also prune old reflog entries and repack blobs'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('grep', 'Search for a pattern in tracked files',
    'suture grep [\\fIoptions\\fR] \\fIpattern\\fR [\\fIpaths\\fR]...',
    [
        ('\\-i, \\-\\-ignore-case', 'Case insensitive matching'),
        ('\\-n, \\-\\-line-number', 'Show line numbers'),
        ('\\-\\-context \\fI<n>\\fR', 'Show context lines around matches'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('fsck', 'Verify repository integrity',
    'suture fsck [\\fIoptions\\fR]',
    [
        ('\\-\\-full', 'Also verify blob integrity, parent chains, and branch refs'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('doctor', 'Check repository health and configuration',
    'suture doctor [\\fIoptions\\fR]',
    [
        ('\\-\\-fix', 'Automatically fix detected issues'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('audit', 'Inspect the tamper-evident audit log',
    'suture audit [\\fIoptions\\fR]',
    [
        ('\\-\\-verify', 'Verify chain integrity'),
        ('\\-\\-show', 'Display all entries'),
        ('\\-\\-count', 'Show entry count'),
        ('\\-\\-tail \\fI<n>\\fR', 'Show last N entries (default: 10)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('clean', 'Remove untracked files from the working tree',
    'suture clean [\\fIoptions\\fR] [\\fIpaths\\fR]...',
    [
        ('\\-n, \\-\\-dry-run', 'Show what would be deleted without actually deleting'),
        ('\\-d, \\-\\-dirs', 'Also remove empty untracked directories'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture clean                # Remove all untracked files',
        'suture clean \\-n             # Preview what would be deleted',
        'suture clean \\-d             # Also remove empty untracked directories',
        'suture clean build/         # Only clean files under build/',
    ])

write_page('describe', 'Describe a commit using the nearest tag',
    'suture describe [\\fIoptions\\fR] [\\fIcommit-ref\\fR]',
    [
        ('\\-\\-all', 'Search all tags (not just annotated)'),
        ('\\-\\-tags', 'Only search tags (default behavior)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture describe             # Describe HEAD',
        'suture describe HEAD~3      # Describe a specific commit',
        'suture describe \\-\\-all       # Search all tags',
    ])

write_page('rev-parse', 'Parse revision names to hashes',
    'suture rev-parse [\\fIoptions\\fR] [\\fIrefs\\fR]...',
    [
        ('\\-\\-short', 'Output abbreviated hash'),
        ('\\-\\-verify', 'Verify the ref exists (error if not)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture rev-parse HEAD       # Resolve HEAD to full hash',
        'suture rev-parse \\-\\-short HEAD  # Abbreviated hash',
        'suture rev-parse \\-\\-verify main  # Verify ref exists',
    ])

write_page('git', 'Interact with Git repositories',
    'suture git [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('import', 'Import Git history into the current Suture repository'),
        ('log', 'Show Git commits that would be imported'),
        ('status', 'Show import summary'),
        ('driver', 'Manage Suture as a Git merge driver'),
    ],
    examples=[
        'suture git import ./my-project     # Import Git history into Suture',
        'suture git log ./my-project        # Preview Git commits to import',
        'suture git status ./my-project     # Show import summary',
    ])

write_page('bisect', 'Binary search for bug-introducing commit',
    'suture bisect [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('start', 'Start a bisect session'),
        ('run', 'Automatically bisect using a test command'),
        ('reset', 'Reset/cancel a bisect session'),
    ])

write_page('hook', 'Manage repository hooks',
    'suture hook [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('list', 'List all configured hooks'),
        ('run', 'Manually execute a hook'),
        ('edit', 'Create or edit a hook in $EDITOR'),
    ],
    examples=[
        'suture hook list                # List all hooks with their scripts',
        'suture hook run pre-commit      # Manually trigger a hook',
        'suture hook edit pre-commit     # Open hook in $EDITOR',
    ])

write_page('classification', 'Bulk classification marking scanning and compliance reporting',
    'suture classification [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('scan', 'Scan all commits for classification marking changes'),
        ('report', 'Generate classification compliance report'),
    ])

write_page('undo', 'Undo the last operation (commit, merge, checkout, etc.)',
    'suture undo [\\fIoptions\\fR]',
    [
        ('\\-s, \\-\\-steps \\fI<n>\\fR', 'Number of operations to undo (default: 1)'),
        ('\\-\\-hard', 'Discard working tree changes (like --hard reset)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture undo                # Undo the last operation (soft)',
        'suture undo \\-\\-steps 3      # Undo the last 3 operations',
        'suture undo \\-\\-hard         # Undo and discard working changes',
    ])

write_page('squash', 'Squash N commits into one',
    'suture squash [\\fIoptions\\fR] \\fIcount\\fR',
    [
        ('\\-m, \\-\\-message \\fI<msg>\\fR', 'Custom message for the squashed commit (default: combined messages)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture squash 3            # Squash last 3 commits',
        'suture squash 3 \\-m "combined"  # With custom message',
    ])

write_page('sync', 'Synchronize with remote -- commit changes and push/pull',
    'suture sync [\\fIoptions\\fR] [\\fIremote\\fR]',
    [
        ('\\-\\-no-push', 'Auto-commit but don\'t push'),
        ('\\-\\-pull-only', 'Only pull from remote (don\'t commit or push)'),
        ('\\-m, \\-\\-message \\fI<msg>\\fR', 'Custom commit message (default: auto-generated)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture sync                     # Auto-commit staged+unstaged, then push/pull',
        'suture sync \\-\\-no-push           # Auto-commit but don\'t push',
        'suture sync \\-\\-pull-only         # Only pull from remote',
        'suture sync \\-\\-message \'WIP\'     # Custom commit message',
    ])

write_page('tui', 'Launch terminal UI',
    'suture tui [\\fIoptions\\fR]',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ])

write_page('export', 'Export a clean snapshot without repository metadata',
    'suture export [\\fIoptions\\fR] \\fIoutput\\fR',
    [
        ('\\-\\-zip', 'Export as zip instead of directory'),
        ('\\-\\-at \\fI<ref>\\fR', 'Commit ref to export (default: HEAD)'),
        ('\\-\\-template \\fI<dir>\\fR', 'Custom template directory (files to include in export)'),
        ('\\-\\-include-meta', 'Include .suture metadata in export'),
        ('\\-\\-client \\fI<name>\\fR', 'Client name (creates {output}/{client}/ subdirectory)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture export ../client-delivery       # Export HEAD to a directory',
        'suture export ../v2 main              # Export specific branch',
        'suture export \\-\\-zip ../delivery.zip   # Export as zip file',
        'suture export \\-\\-include-meta ./full-export',
    ])

write_page('report', 'Generate reports about the repository',
    'suture report [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('change', 'Generate a change summary report (what changed between two refs)'),
        ('activity', 'Generate a contributor/activity report'),
        ('stats', 'Generate a file statistics report'),
    ])

write_page('batch', 'Batch operations for managing multiple files or clients',
    'suture batch [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('stage', 'Stage files matching a pattern'),
        ('commit', 'Commit files matching a pattern'),
        ('export-clients', 'Export multiple clients at once'),
    ])

write_page('timeline', 'OTIO timeline operations',
    'suture timeline [\\fIoptions\\fR] \\fIcommand\\fR',
    [
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    subcommands=[
        ('import', 'Import an OTIO timeline into the repo'),
        ('export', 'Export the current timeline to OTIO format'),
        ('summary', 'Show timeline summary (clips, duration, tracks)'),
        ('diff', 'Diff two timeline versions'),
        ('list', 'List timeline-related files in the repo'),
    ])

write_main_page()

write_page('archive', 'Create an archive of the repository',
    'suture archive [\\fIoptions\\fR] \\-o \\fIoutput\\fR [\\fIcommit\\fR]',
    [
        ('\\-o, \\-\\-output \\fI<path>\\fR', 'Output file path'),
        ('\\-f, \\-\\-format \\fI<fmt>\\fR', 'Archive format (default: auto-detect from output extension)'),
        ('\\-\\-prefix \\fI<prefix>\\fR', 'Prefix directory in the archive (default: repo name)'),
        ('\\-C \\fI<REPO_PATH>\\fR', C),
        ('\\-h, \\-\\-help', H),
    ],
    examples=[
        'suture archive \\-o project.tar.gz         # Archive HEAD as tar.gz',
        'suture archive \\-\\-format zip \\-o out.zip   # Archive HEAD as zip',
        'suture archive main \\-o release.tar.gz    # Archive a specific branch',
    ])

count = len([f for f in os.listdir(MAN_DIR) if f.endswith('.1')])
print(f"\n=== Generated {count} man pages ===")
