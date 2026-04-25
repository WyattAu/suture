# Suture VCS — Onboarding Guide for YouTube Channel Managers & PE Firms

Replace Google Drive sync with full version history, branching, and audit trail.

---

## 1. Installation

### macOS

```bash
brew install suture-vcs/tap/suture
```

### Linux (Debian/Ubuntu)

```bash
curl -fsSL https://get.suture.dev | bash
```

### Windows (WSL2 recommended)

```bash
curl -fsSL https://get.suture.dev | bash
```

Verify the installation:

```bash
suture --version
```

Authenticate with your Suture account:

```bash
suture auth login
```

---

## 2. Initial Setup — Migrating from Google Drive

### 2.1 Create a workspace

Replace your Google Drive root with a Suture workspace. Each workspace is an independent versioned repository.

```bash
mkdir ~/suture-workspaces && cd ~/suture-workspaces
suture init --name "acme-channels"
```

This creates:

```
acme-channels/
  .suture/
    config.toml
    branches/
    history/
  channels/
  reports/
  exports/
```

### 2.2 Configure sync

Point Suture at the folder you want to version-control. If you're migrating from Google Drive, copy files out first:

```bash
suture config set sync.root ./channels
suture config set sync.watch true
suture config set sync.auto-commit true
suture config set sync.auto-commit-interval 300
```

These settings enable:
- File watching on `./channels`
- Auto-commits every 5 minutes when changes are detected
- Full version history for every save

### 2.3 Migrate existing Google Drive files

```bash
cp -r ~/Google\ Drive/YouTube\ Projects/* ./channels/
suture sync
```

`suture sync` scans the directory, stages all files, and creates an initial commit. Every file now has version history from this point forward.

### 2.4 Connect cloud remote (optional but recommended)

```bash
suture remote add origin suture://acme-channels.cloud.suture.dev
suture push origin main
```

---

## 3. Importing Existing YouTube Metadata

### 3.1 Connect a YouTube channel

You need a Google OAuth token for each channel. Suture stores tokens securely in `.suture/credentials`.

```bash
suture youtube connect --channel "Acme Gaming"
```

Follow the browser prompt to authorize. Repeat for each channel.

### 3.2 Export current metadata from YouTube

Pull all video metadata (titles, descriptions, tags, thumbnails, settings) into your workspace:

```bash
suture youtube export --channel "Acme Gaming" --output ./channels/acme-gaming/metadata/
```

This generates structured JSON per video:

```
channels/acme-gaming/metadata/
  ABC123.json
  DEF456.json
  thumbnails/
    ABC123.jpg
    DEF456.jpg
```

Each JSON file contains:

```json
{
  "videoId": "ABC123",
  "title": "Top 10 Tips for Beginners",
  "description": "In this video we cover...",
  "tags": ["beginner", "tips", "tutorial"],
  "categoryId": "27",
  "privacyStatus": "public",
  "thumbnail": "thumbnails/ABC123.jpg",
  "suture": {
    "lastSynced": "2026-04-25T10:30:00Z",
    "source": "youtube-export"
  }
}
```

### 3.3 Commit the imported metadata

```bash
suture add ./channels/acme-gaming/metadata/
suture commit -m "Import metadata for Acme Gaming channel"
```

### 3.4 Import multiple channels at once

```bash
suture youtube connect --channel "Acme Gaming"
suture youtube connect --channel "Acme Tech"
suture youtube connect --channel "Acme Vlogs"

suture batch add \
  --channels "Acme Gaming,Acme Tech,Acme Vlogs" \
  --output ./channels/

suture batch commit -m "Import metadata for all Acme channels"
```

---

## 4. Daily Workflow

### 4.1 Start the sync daemon

Run this at the start of each session (or set up a launchd/systemd service):

```bash
suture sync --daemon
```

This watches `./channels/` for changes and auto-commits every 5 minutes (or your configured interval).

### 4.2 Edit metadata locally

Open the JSON files in your editor of choice:

```bash
vim ./channels/acme-gaming/metadata/ABC123.json
```

Change the title, description, tags — whatever you need. Save the file.

### 4.3 Preview what changed

Before pushing anything to YouTube, see a diff:

```bash
suture youtube diff --channel "Acme Gaming"
```

Output:

```
ABC123.json:
  title:       "Top 10 Tips for Beginners"
            -> "Top 10 Tips for Beginners (2026 Edition)"
  tags:        ["beginner", "tips", "tutorial"]
            -> ["beginner", "tips", "tutorial", "2026"]
  description:  +2 lines, -0 lines

1 change(s) detected across 1 video(s).
```

### 4.4 Push changes to YouTube

```bash
suture youtube sync --channel "Acme Gaming" --videos ABC123
```

To sync all pending changes across a channel:

```bash
suture youtube sync --channel "Acme Gaming" --all
```

### 4.5 Review the commit history

```bash
suture log --branch main --limit 10
```

```
a3f2c1d  2026-04-25 14:32  Updated title and tags for ABC123
e7b9a4f  2026-04-25 11:00  Auto-commit: 3 files changed
c5d8e2a  2026-04-24 16:45  Batch tag update across Acme Gaming
```

### 4.6 Undo a change

If something went wrong, roll back to a previous state:

```bash
suture revert a3f2c1d
```

Then push the reverted state to YouTube:

```bash
suture youtube sync --channel "Acme Gaming" --all
```

---

## 5. Multi-Channel Management

### 5.1 Strategy: Branch per channel

Create a branch for each channel so changes are isolated:

```bash
suture branch create acme-gaming
suture branch create acme-tech
suture branch create acme-vlogs
```

Switch to a branch and work on it:

```bash
suture branch checkout acme-gaming
# Edit files, make commits
suture youtube sync --channel "Acme Gaming" --all
```

### 5.2 Strategy: Folder per channel (single branch)

If channels share resources (thumbnails, templates), use folders instead:

```
channels/
  acme-gaming/
  acme-tech/
  acme-vlogs/
  shared/
    thumbnails/
    templates/
```

### 5.3 Apply changes across channels

Use batch operations to update metadata across multiple channels at once:

```bash
suture batch add --pattern "channels/*/metadata/*.json"
suture batch commit -m "Q2 tag refresh across all channels"
```

Or run a scripted update:

```bash
suture batch run --channels "Acme Gaming,Acme Tech" \
  --script ./scripts/add-sponsorship-tag.sh
```

---

## 6. Client Delivery and Handoff

### 6.1 Export a snapshot for a client

Generate a clean export of the current state:

```bash
suture export --output ./exports/acme-gaming-q2-2026.zip \
  --path ./channels/acme-gaming/ \
  --format structured
```

The zip contains all metadata, thumbnails, and a manifest file:

```
acme-gaming-q2-2026.zip
  manifest.json        (checksums, timestamps, version info)
  metadata/            (all video JSON files)
  thumbnails/          (all thumbnail images)
  reports/             (optional: include generated reports)
```

### 6.2 Include a report

```bash
suture report --channel "Acme Gaming" \
  --period 2026-04-01:2026-04-30 \
  --format pdf \
  --output ./exports/acme-gaming-q2-2026.pdf
```

Bundle both:

```bash
suture export --output ./exports/acme-gaming-q2-delivery.zip \
  --path ./channels/acme-gaming/ \
  --attach ./exports/acme-gaming-q2-2026.pdf
```

### 6.3 Handoff to another team

If another PE firm or agency takes over, give them the workspace:

```bash
suture export --full --output ./exports/acme-gaming-handoff.tar.gz
```

This includes the entire `.suture/` history, so the new team has full audit trail from day one.

---

## 7. Audit Trail and Compliance

### 7.1 View full change history

```bash
suture log --path ./channels/acme-gaming/metadata/ABC123.json --full
```

```
a3f2c1d  2026-04-25 14:32  user@example.com
  title: "Top 10 Tips for Beginners" -> "Top 10 Tips for Beginners (2026 Edition)"
  tags:  +["2026"]

e7b9a4a  2026-04-20 09:15  user@example.com
  description: +3 lines (sponsorship disclosure added)

c5d8e2a  2026-04-15 11:00  user@example.com
  initial import from YouTube
```

### 7.2 Generate a compliance report

For PE due diligence or regulatory review:

```bash
suture report --channel "Acme Gaming" \
  --type audit \
  --period 2026-01-01:2026-04-25 \
  --output ./reports/acme-gaming-audit-q1q2-2026.pdf
```

### 7.3 Lock a branch

Prevent further changes after a delivery milestone:

```bash
suture branch lock acme-gaming --reason "Q2 delivery finalized"
```

### 7.4 Require approval before YouTube sync

Configure a pre-sync review gate:

```bash
suture config set youtube.sync.require-approval true
suture config set youtube.sync.approvers "manager@example.com,compliance@example.com"
```

Now `suture youtube sync` creates a pending change request instead of pushing directly:

```bash
suture youtube sync --channel "Acme Gaming" --all
# Output: Change request CR-0042 created. Awaiting approval.
```

Approve:

```bash
suture approve CR-0042
```

---

## 8. Backup and Disaster Recovery

### 8.1 Push to a remote

```bash
suture remote add backup suture://backup.suture.dev/acme-channels
suture push backup --all-branches
```

Schedule this with cron:

```bash
crontab -e
# Add:
0 */6 * * * suture push backup --all-branches --quiet
```

### 8.2 Local backup

Export the entire workspace (history included):

```bash
suture export --full --output /mnt/nas/backups/suture-acme-$(date +%Y%m%d).tar.gz
```

### 8.3 Restore from backup

```bash
mkdir restore && cd restore
suture init --name "acme-channels-restored"
suture import /mnt/nas/backups/suture-acme-20260425.tar.gz
```

### 8.4 Recover a deleted file

```bash
suture log --deleted --path ./channels/acme-gaming/metadata/
# Find the commit where the file was last present
suture checkout c5d8e2a -- ./channels/acme-gaming/metadata/ABC123.json
```

### 8.5 Verify workspace integrity

```bash
suture verify --full
```

This checks checksums for all files against the commit history and reports any corruption.

---

## Quick Reference

| Command | Purpose |
|---|---|
| `suture init` | Create a new workspace |
| `suture sync --daemon` | Start file watcher + auto-commit |
| `suture youtube connect` | Link a YouTube channel |
| `suture youtube export` | Pull metadata from YouTube |
| `suture youtube diff` | Preview pending YouTube changes |
| `suture youtube sync` | Push metadata changes to YouTube |
| `suture batch add` | Stage files across multiple channels |
| `suture batch commit` | Commit staged batch changes |
| `suture report` | Generate performance or audit reports |
| `suture export` | Create a delivery snapshot |
| `suture log` | View commit history |
| `suture revert` | Undo a commit |
| `suture verify` | Check workspace integrity |
