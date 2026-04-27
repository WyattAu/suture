# Suture VCS — Defence Contractor Onboarding Guide

This guide covers the minimum steps to set up, operate, and maintain a classified
workspace using Suture VCS on Ubuntu 22.04 LTS.

---

## 1. Installation

### From GitHub Releases (recommended)

```bash
# Download the latest Linux amd64 binary
curl -fSL -o suture.tar.gz \
  https://github.com/WyattAu/suture/releases/latest/download/suture-x86_64-linux.tar.gz

# Verify the SHA-256 checksum
curl -fSL -o suture.tar.gz.sha256 \
  https://github.com/WyattAu/suture/releases/latest/download/suture-x86_64-linux.tar.gz.sha256
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

---

## 2. Initial Setup

### Identity and signing key

```bash
# Set your identity (used in commit metadata and audit log)
suture config user.name="Sgt. Jane Doe"
suture config user.email="jane.doe@mil.example.gov"

# Generate an Ed25519 signing key (default name: "default")
suture key generate

# Verify the key was created
suture key list

# Restrict key permissions
chmod 600 ~/.config/suture/keys/
```

### Verify your setup

```bash
suture doctor
```

This runs the repository health check and reports any configuration issues. Address
all warnings before proceeding to classified work.

---

## 3. Creating a Classified Workspace

```bash
mkdir -p /data/projects/alpha-protocol
cd /data/projects/alpha-protocol

# Initialise the workspace
suture init

# Verify the repository was created
suture status
```

All files tracked in this workspace are version-controlled with full audit trail.
Commit metadata and signatures are embedded in the tamper-evident chain log.

---

## 4. Daily Workflow

### Tracking files

Suture supports semantic diffing across all registered formats (YAML, JSON, TOML,
XML, DOCX, XLSX, iCal, OTIO, and 17+ more). Files are added exactly as they are
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

When a signing key is configured, commits are signed automatically.

```bash
suture commit "Update risk matrix per REV-42 review"
```

### Reviewing state

```bash
# Show workspace status
suture status

# Show commit history
suture log --oneline

# Show detailed history with signature verification
suture log --verify
```

---

## 5. Branching and Merging

### Create a branch for your task

```bash
# Create and switch to a new branch
suture checkout -b feature/rev42-updates

# Work on the branch
suture add updated-timeline.otio
suture commit "Add OTIO timeline for REV-42"

# Return to the main line
suture checkout main
```

### Merging with semantic drivers

When two branches modify the same YAML or XML file, Suture applies semantic merge
drivers instead of line-based diffing. This preserves structure and avoids false
conflicts on reordered keys or whitespace.

```bash
# Merge a feature branch into main
suture checkout main
suture merge feature/rev42-updates

# If there are conflicts, review them
suture merge --continue

# Or abort and try again
suture merge --abort
```

### List and clean up branches

```bash
suture branch              # List branches
suture branch -d feature/rev42-updates  # Delete a branch
```

---

## 6. Audit Trail Verification

Suture maintains a tamper-evident chain log in every workspace. Each entry
contains the parent hash, file hashes, commit metadata, and a signature.
Altering any historical entry invalidates all subsequent hashes.

### Inspect the audit log

```bash
# Show last 10 entries
suture audit --show

# Show last 50 entries
suture audit --tail 50

# Count total entries
suture audit --count
```

### Verify integrity

```bash
# Verify the entire chain is intact
suture audit --verify

# Verify commit signatures
suture verify

# Verify with key details (author, fingerprint)
suture verify -v
```

A non-zero exit code means the chain is broken or a signature is invalid. Treat
this as a security incident — see Section 8.

### Export audit trail

```bash
# Export structured audit trail as JSON
suture log --audit --audit-format json > audit-2025-Q1.json

# Filter by date range
suture log --audit --since 2025-01-01 --until 2025-04-01 --audit-format csv > audit-Q1.csv

# Filter by author
suture log --audit --author "jane.doe@mil.example.gov" --audit-format json
```

---

## 7. Classification Scanning and Compliance

### Scan for classification changes

```bash
# Scan all commits for classification marking changes
suture classification scan

# Generate a classification compliance report
suture classification report
```

### Run health checks

```bash
# Full health check
suture doctor

# Auto-fix detected issues
suture doctor --fix
```

### Scheduled compliance sweep (cron)

```bash
# Add to crontab — daily at 0600
0 6 * * * cd /data/projects/alpha-protocol && suture classification scan >> /var/log/suture-compliance.log 2>&1
0 6 * * * cd /data/projects/alpha-protocol && suture classification report >> /var/log/suture-compliance.log 2>&1
```

---

## 8. Multi-Team Collaboration

Each team operates on its own branch. Merges into the shared integration branch
use semantic drivers to handle structured file conflicts.

### Team leads set up the integration branch

```bash
cd /data/projects/alpha-protocol
suture branch integration
```

### Each team creates their working branch from integration

```bash
# Team Alpha
suture checkout integration
suture checkout -b team-alpha/hardware-specs

# Team Bravo
suture checkout integration
suture checkout -b team-bravo/software-requirements
```

### Teams work independently

```bash
suture checkout team-alpha/hardware-specs
suture add hardware-spec.yaml
suture commit "Add chassis dimensions v3"
```

### Merge into integration

```bash
suture checkout integration
suture merge team-alpha/hardware-specs
suture merge team-bravo/software-requirements
```

If both teams modified the same YAML/JSON file, the semantic merge driver resolves
structural conflicts automatically. Remaining conflicts are surfaced for manual
resolution.

### Verify after merge

```bash
suture verify
suture audit --verify
suture doctor
```

---

## 9. Incident Response

### Find who changed a file and when

```bash
# Annotate each line with the last commit that touched it
suture blame requirements.yaml

# Blame a specific line range
suture blame requirements.yaml -L 50,70

# Blame as of a specific commit
suture blame requirements.yaml --at HEAD~3
```

### Revert a problematic commit

```bash
# Identify the commit
suture log --oneline

# Revert a specific commit (creates a new commit undoing the change)
suture revert <commit-hash> -m "Revert: incorrect classification applied"
```

### Verify workspace integrity after an incident

```bash
# Full integrity check
suture verify -v

# Verify chain integrity
suture audit --verify

# Export audit trail for forensics
suture log --audit --audit-format json > incident-forensics.json
```

### Escalation checklist

1. Run `suture audit --verify` and capture the output.
2. Run `suture verify -v` and check all signatures.
3. Export audit trail: `suture log --audit --audit-format json > incident-<date>.json`.
4. Restrict workspace access until the break is investigated.
5. Do not attempt to repair the chain log manually — this will invalidate
   further entries and destroy forensic evidence.

---

## Quick Reference

| Command                              | Purpose                          |
|--------------------------------------|----------------------------------|
| `suture init`                        | Create a new workspace           |
| `suture add <path>`                  | Stage files                      |
| `suture add --all`                   | Stage all modified files         |
| `suture commit "<msg>"`              | Commit (auto-signed if key set)  |
| `suture branch <name>`               | Create a branch                  |
| `suture branch`                      | List branches                    |
| `suture branch -d <name>`            | Delete a branch                  |
| `suture checkout <name>`             | Switch branches                  |
| `suture checkout -b <name>`          | Create and switch to branch      |
| `suture merge <branch>`              | Merge a branch                   |
| `suture merge --continue`            | Continue after resolving conflicts|
| `suture merge --abort`               | Abort a merge                    |
| `suture log --oneline`               | View commit history (compact)    |
| `suture log --verify`                | View history with signatures     |
| `suture log --audit`                 | Export structured audit trail    |
| `suture audit --show`                | Display all audit entries        |
| `suture audit --verify`              | Verify chain integrity           |
| `suture verify`                      | Verify commit signatures         |
| `suture key generate`                | Generate a new signing key       |
| `suture key list`                    | List local signing keys          |
| `suture doctor`                      | Check repository health          |
| `suture classification scan`         | Scan for classification changes  |
| `suture classification report`       | Generate compliance report       |
| `suture blame <file>`                | Annotate file with commit info   |
| `suture blame <file> -L 10,20`       | Blame specific line range        |
| `suture revert <hash> -m "<msg>"`    | Revert a commit                  |
| `suture config user.name=<name>`     | Set user identity                |
| `suture config user.email=<email>`   | Set user email                   |
