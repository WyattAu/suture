# Suture VCS — Defence Contractor Onboarding Guide

This guide covers the minimum steps to set up, operate, and maintain a classified
workspace using Suture VCS on Ubuntu 22.04 LTS.

---

## 1. Installation

### From GitHub Releases (recommended)

```bash
# Download the latest Linux amd64 binary
curl -fSL -o suture.tar.gz \
  https://github.com/WyattAu/suture/releases/latest/download/suture-linux-x86_64.tar.gz

# Verify the SHA-256 checksum (compare against the published checksums file)
curl -fSL -o suture.tar.gz.sha256 \
  https://github.com/WyattAu/suture/releases/latest/download/suture-linux-x86_64.tar.gz.sha256
sha256sum -c suture.tar.gz.sha256

# Extract and install to /usr/local
tar xzf suture.tar.gz
sudo install -m 0755 suture /usr/local/bin/suture
suture --version
```

### From Source

```bash
# Install build dependencies
sudo apt-get update && sudo apt-get install -y \
  build-essential pkg-config libssl-dev cmake

# Clone and build
git clone https://github.com/WyattAu/suture.git
cd suture
cargo build --release --bin suture
sudo install -m 0755 target/release/suture /usr/local/bin/suture
suture --version
```

Verify all 37 crates compiled cleanly:

```bash
suture --version
# Expected output: suture x.y.z (N crates loaded)
```

---

## 2. Initial Setup

### Identity and signing key

```bash
# Set your identity (used in commit metadata and audit log)
suture config user.name "Sgt. Jane Doe"
suture config user.email "jane.doe@mil.example.gov"

# Generate an Ed25519 signing key
suture sign-key generate --algorithm ed25519 --output ~/.config/suture/signing.key

# Register the key with Suture
suture config sign.key ~/.config/suture/signing.key

# Restrict key permissions
chmod 600 ~/.config/suture/signing.key
```

### Verify your setup

```bash
suture doctor --compliance
```

This runs the compliance doctor and reports any configuration issues. Address all
warnings before proceeding to classified work.

---

## 3. Creating a Classified Workspace

```bash
mkdir -p /data/projects/alpha-protocol
cd /data/projects/alpha-protocol

# Initialise the workspace
suture init

# Apply a classification level to the entire workspace
suture classify --level SECRET
suture classify --label CUI

# Verify classification
suture classify --show
# Expected: level=SECRET, label=CUI
```

All files tracked in this workspace inherit the classification. The classification
metadata is embedded in `chain.log` and cannot be altered retroactively without
invalidating the hash chain.

---

## 4. Daily Workflow

### Tracking files

Suture supports semantic diffing across all registered formats (YAML, JSON, TOML,
XML, DOCX, XLSX, iCal, OTIO, and 20+ more). Files are added exactly as they are
— no conversion required.

```bash
# Add individual files
suture add requirements.yaml risk-matrix.xlsx

# Add an entire directory
suture add docs/

# Stage everything
suture add --all
```

### Committing with a signature

Every commit should be signed in a classified workspace.

```bash
suture commit --sign -m "Update risk matrix per REV-42 review"
```

If you omit `--sign`, the compliance doctor will flag unsigned commits on next run.

### Reviewing state

```bash
# Show workspace status
suture status

# Show commit history
suture log --oneline

# Show detailed history with signatures
suture log --verify
```

---

## 5. Branching and Merging

### Create a branch for your task

```bash
# Create and switch to a new branch
suture branch create feature/rev42-updates
suture branch checkout feature/rev42-updates

# Work on the branch
suture add updated-timeline.otio
suture commit --sign -m "Add OTIO timeline for REV-42"

# Return to the main line
suture branch checkout main
```

### Merging with semantic drivers

When two branches modify the same YAML or XML file, Suture applies semantic merge
drivers instead of line-based diffing. This preserves structure and avoids false
conflicts on reordered keys or whitespace.

```bash
# Merge a feature branch into main
suture branch checkout main
suture merge feature/rev42-updates --sign

# If there are conflicts, review them
suture merge --continue

# Or abort and try again
suture merge --abort
```

### List and clean up branches

```bash
suture branch list
suture branch delete feature/rev42-updates
```

---

## 6. Audit Trail Verification

Suture maintains a tamper-evident `chain.log` in every workspace. Each entry
contains the parent hash, file hashes, commit metadata, and an optional signature.
Altering any historical entry invalidates all subsequent hashes.

### Inspect the audit log

```bash
# Full audit trail
suture audit

# Audit since a specific date
suture audit --since 2025-01-01

# Audit for a specific author
suture audit --author "jane.doe@mil.example.gov"
```

### Verify integrity

```bash
# Verify the entire chain is intact (hash chain integrity)
suture verify

# Verify and check all signatures
suture verify --signed
```

A non-zero exit code means the chain is broken or a signature is invalid. Treat
this as a security incident — see Section 8.

### Export for external review

```bash
# Export audit log as a signed, portable artifact
suture audit --export audit-2025-Q1.json --sign
```

---

## 7. Classification and Compliance Checks

### Apply classification to individual files

```bash
# Override workspace-level classification for a specific file
suture classify --file export-report.docx --level TOP_SECRET
suture classify --file export-report.docx --label NOFORN
```

### Run compliance checks

```bash
# Full compliance scan
suture doctor --compliance

# Check for unsigned commits
suture doctor --compliance --check unsigned-commits

# Check for missing classifications
suture doctor --compliance --check unclassified-files

# Check for stale branches
suture doctor --compliance --check stale-branches
```

The compliance doctor reports a summary and exit code:

| Exit code | Meaning                        |
|-----------|--------------------------------|
| 0         | All checks pass                |
| 1         | Warnings (non-blocking)        |
| 2         | Errors — action required       |

Integrate `suture doctor --compliance` into your CI pipeline. Exit code 2 should
fail the build.

### Scheduled compliance sweep (cron)

```bash
# Add to crontab — daily at 0600
0 6 * * * cd /data/projects/alpha-protocol && suture doctor --compliance --check unsigned-commits --check unclassified-files >> /var/log/suture-compliance.log 2>&1
```

---

## 8. Multi-Team Collaboration

Each team operates on its own branch. Merges into the shared integration branch
use semantic drivers to handle structured file conflicts.

### Team leads set up the integration branch

```bash
cd /data/projects/alpha-protocol
suture branch create integration
```

### Each team creates their working branch from integration

```bash
# Team Alpha
suture branch checkout integration
suture branch create team-alpha/hardware-specs

# Team Bravo
suture branch checkout integration
suture branch create team-bravo/software-requirements
```

### Teams work independently

```bash
suture branch checkout team-alpha/hardware-specs
suture add hardware-spec.yaml
suture commit --sign -m "Add chassis dimensions v3"
```

### Merge into integration

```bash
suture branch checkout integration
suture merge team-alpha/hardware-specs --sign
suture merge team-bravo/software-requirements --sign
```

If both teams modified the same YAML/JSON file, the semantic merge driver resolves
structural conflicts automatically. Remaining conflicts are surfaced for manual
resolution.

### Verify after merge

```bash
suture verify --signed
suture doctor --compliance
```

---

## 9. Incident Response

### Find who changed a file and when

```bash
# Annotate each line/element with the last commit that touched it
suture blame requirements.yaml

# Blame a specific section
suture blame requirements.yaml --section "3.2.1"
```

### Revert a problematic commit

```bash
# Identify the commit
suture log --oneline

# Revert a specific commit (creates a new commit undoing the change)
suture revert <commit-hash> --sign -m "Revert: incorrect classification applied"
```

### Verify workspace integrity after an incident

```bash
# Full integrity check
suture verify --signed

# If the chain is broken, export what you can for forensics
suture audit --export incident-forensics.json

# Identify the break point
suture verify --signed --verbose
```

### Escalation checklist

1. Run `suture verify --signed` and capture the output.
2. Run `suture audit --export incident-<date>.json --sign`.
3. Restrict workspace access until the break is investigated.
4. Do not attempt to repair `chain.log` manually — this will invalidate
   further entries and destroy forensic evidence.

---

## Quick Reference

| Command                              | Purpose                          |
|--------------------------------------|----------------------------------|
| `suture init`                        | Create a new workspace           |
| `suture add <path>`                  | Stage files                      |
| `suture commit --sign -m "<msg>"`    | Commit with signature            |
| `suture branch create <name>`        | Create a branch                  |
| `suture branch checkout <name>`      | Switch branches                  |
| `suture merge <branch> --sign`       | Merge a branch                   |
| `suture log [--oneline\|--verify]`   | View commit history              |
| `suture audit`                       | View audit trail                 |
| `suture verify --signed`             | Verify chain integrity           |
| `suture classify --level <LEVEL>`    | Set classification level         |
| `suture classify --label <LABEL>`    | Set classification label         |
| `suture doctor --compliance`         | Run compliance checks            |
| `suture blame <file>`                | Annotate file with commit info   |
| `suture revert <hash> --sign`        | Revert a commit                  |
| `suture sign-key generate`           | Generate a new signing key       |
