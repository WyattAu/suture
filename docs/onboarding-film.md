# Suture VCS — Onboarding Guide for Film & TV Post-Production

This guide walks post-production teams through using Suture for timeline
versioning, multi-department collaboration, and delivery. Suture is a
semantic version control system with native OTIO support — it is not Git.

---

## 1. Installation

### macOS

```bash
brew install suture
```

### Linux (Debian/Ubuntu)

```bash
curl -fsSL https://releases.suture.dev/apt/gpg.key | sudo gpg --dearmor -o /etc/apt/keyrings/suture.gpg
echo "deb [signed-by=/etc/apt/keyrings/suture.gpg] https://releases.suture.dev/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/suture.list
sudo apt update && sudo apt install suture
```

### Verify

```bash
suture --version
suture --help
```

You need a Suture account and a workspace. If your post supervisor or
assistant editor has already created a workspace, ask them for an invite.
Otherwise:

```bash
suture auth login
suture workspace create "ProjectName_Show_S01"
```

---

## 2. Initial Setup

Navigate to your project's shared storage or local working directory and
initialize a Suture project:

```bash
cd /mnt/projects/projectname/show
suture init
```

This creates a `.suture/` directory with project metadata. Do not manually
edit `.suture/`.

Set your identity so commits are attributed correctly:

```bash
suture config set user.name "Jane Doe"
suture config set user.email "jane@studio.com"
suture config set user.department "editorial"
```

Configure default timeline format:

```bash
suture config set timeline.format "otio"
suture config set timeline.frame-rate "23.976"
```

### Invite collaborators

```bash
suture team invite vfx-lead@studio.com --role editor
suture team invite sound-designer@studio.com --role editor
suture team invite colorist@studio.com --role editor
```

---

## 3. Importing an Existing Timeline from Editorial

Assume editorial has delivered an AAF or XML export from Avid/Premiere.
First convert to OTIO if needed, then import into Suture:

```bash
suture timeline import --source "s01e01_lock_v1.xml" \
  --format "premiere-xml" \
  --label "S01E01 editorial lock v1"
```

If you already have an OTIO file:

```bash
suture timeline import --source "s01e01_lock_v1.otio" \
  --label "S01E01 editorial lock v1"
```

Verify the import:

```bash
suture timeline summary
```

Expected output includes track count, clip count, total duration, and
timecode range.

Tag the import so it serves as a reference point:

```bash
suture tag add "editorial-lock-v1" --message "Picture lock from editorial"
```

---

## 4. Daily Workflow

### Modify a timeline

Open your timeline in your NLE as usual. Edit, trim, add markers. When
you're ready to save a version, export from your NLE and re-import:

```bash
suture timeline import --source "s01e01_v2.otio" \
  --label "S01E01 v2 — temp VFX added"
```

### Commit changes

Every import creates a pending change set. Commit it:

```bash
suture commit --message "Add temp VFX shots on reels 2 and 3"
```

### View history

```bash
suture log
suture log --oneline --last 10
```

### Undo a bad commit

```bash
suture revert HEAD
suture commit --message "Revert bad temp VFX placement"
```

---

## 5. Multi-Department Collaboration

### Branch per department

After editorial lock, each department works on its own branch. The
assistant editor typically manages branching:

```bash
suture branch create "vfx" --from "editorial-lock-v1"
suture branch create "sound" --from "editorial-lock-v1"
suture branch create "color" --from "editorial-lock-v1"
suture branch create "titles" --from "editorial-lock-v1"
```

### Switch to a branch

```bash
suture branch checkout "vfx"
```

### Work on a branch

Each department imports their modified timeline into their branch:

```bash
suture timeline import --source "s01e01_vfx_comp_v1.otio" \
  --label "VFX comp pass 1"
suture commit --message "VFX: first comp pass for hero shots"
```

### Merge departments back

When departments finish, merge their branches into a main delivery branch:

```bash
suture branch checkout "main"
suture merge "vfx" --message "Merge VFX comp pass 1"
suture merge "sound" --message "Merge sound design mix"
suture merge "color" --message "Merge color grade"
```

Suture performs OTIO-aware merges. Timeline tracks from each department
are preserved — VFX clips on VFX tracks, sound on audio tracks, color
adjustments on metadata. Conflicts are flagged with markers on affected
clips rather than silently overwriting.

### Resolve merge conflicts

If a merge produces conflicts, list them:

```bash
suture merge conflicts
```

Open the conflicted timeline in your NLE, resolve, re-export, and commit:

```bash
suture timeline import --source "s01e01_resolved.otio" \
  --label "Merge conflict resolution"
suture commit --message "Resolve overlap between VFX and color trim changes"
```

---

## 6. Timeline Diff and Comparison

### Compare two versions

```bash
suture diff "editorial-lock-v1" HEAD
```

This shows added, removed, and modified clips with timecode positions.
Use `--track` to filter to specific tracks:

```bash
suture diff "editorial-lock-v1" HEAD --track "VFX1"
suture diff "editorial-lock-v1" HEAD --track "A1" --track "A2"
```

### Blame — who changed what, and when

`suture blame` shows which commit last modified each clip in the current
timeline:

```bash
suture blame
```

Use `--at` to inspect a specific timecode range:

```bash
suture blame --at "00:12:30:00-00:14:00:00"
```

This is useful during review sessions. If a clip moved unexpectedly, blame
tells you which commit moved it and who made that commit.

### Side-by-side summary

```bash
suture timeline summary --at "editorial-lock-v1"
suture timeline summary --at HEAD
```

Compare track counts, durations, and clip counts between versions.

---

## 7. Export and Delivery

### Single timeline export

Export the current timeline to a format your facility or client needs:

```bash
suture timeline export --format "otio" --output "delivery/s01e01_final.otio"
suture timeline export --format "premiere-xml" --output "delivery/s01e01_final.xml"
suture timeline export --format "avid-aaf" --output "delivery/s01e01_final.aaf"
suture timeline export --format "fcpxml" --output "delivery/s01e01_final.fcpxml"
```

### Batch export

When delivering multiple episodes or reels at once:

```bash
suture batch export \
  --episodes "s01e01,s01e02,s01e03" \
  --format "otio" \
  --output "delivery/batch/"
```

Or export from a manifest file:

```bash
suture batch export --manifest "delivery/manifest.json"
```

Manifest format:

```json
{
  "outputs": [
    {
      "episode": "s01e01",
      "tag": "delivery-final",
      "formats": ["otio", "premiere-xml"]
    },
    {
      "episode": "s01e02",
      "tag": "delivery-final",
      "formats": ["otio", "avid-aaf"]
    }
  ],
  "output_dir": "delivery/batch/"
}
```

### Batch operations on timelines

Process multiple timelines in bulk — useful for conforming or reformatting:

```bash
suture batch retime --episodes "s01e01,s01e02" --target-fps 25
suture batch rewrap --episodes "s01e01,s01e02" --output "delivery/rewrapped/"
```

---

## 8. Review and Approval Workflow

### Tag milestones

Use tags to mark review gates:

```bash
suture tag add "director-review-v1" --message "First director review cut"
suture tag add "producer-approval" --message "Producer sign-off on final cut"
suture tag add "network-delivery" --message "Delivered to network"
```

### Generate review reports

```bash
suture report generate \
  --from "editorial-lock-v1" \
  --to "director-review-v1" \
  --format "pdf" \
  --output "reports/s01e01_editorial_to_director_review.pdf"
```

Reports include a change summary, clip-level diff, and per-department
commit log. Available formats: `pdf`, `xlsx`, `html`.

### Video template for review screenings

Generate a reference video from the timeline with burn-in metadata:

```bash
suture video-template \
  --tag "director-review-v1" \
  --template "burnin-timecode" \
  --output "review/s01e01_director_review.mp4"
```

Available templates vary by installation. Check what's available:

```bash
suture video-template --list
```

### Freeze a delivery version

Once approved, lock the version:

```bash
suture tag add "delivery-final-s01e01" --message "Final delivery — do not modify"
suture branch checkout "main"
suture export delivery-tag "delivery-final-s01e01"
```

---

## Quick Reference

| Task | Command |
|---|---|
| Initialize project | `suture init` |
| Import timeline | `suture timeline import --source <file> --label <name>` |
| Commit changes | `suture commit --message "<msg>"` |
| View history | `suture log` |
| Create branch | `suture branch create <name> --from <tag>` |
| Switch branch | `suture branch checkout <name>` |
| Merge branch | `suture merge <branch> --message "<msg>"` |
| Diff versions | `suture diff <from> <to>` |
| Blame at timecode | `suture blame --at <tc>` |
| Export timeline | `suture timeline export --format <fmt> --output <path>` |
| Batch export | `suture batch export --manifest <file>` |
| Generate report | `suture report generate --from <tag> --to <tag>` |
| Tag a version | `suture tag add <name> --message "<msg>"` |
| Review video | `suture video-template --tag <tag> --output <path>` |

---

## Common Issues

### Import fails with format error

Ensure your NLE export is a supported format. Check supported formats:

```bash
suture timeline import --help
```

If importing AAF/XML, Suture converts internally to OTIO. If the source
timeline has unsupported effects or transitions, import will warn but
proceed — unsupported elements are preserved as metadata.

### Merge produces many conflicts

This usually means two departments edited the same timecode range on the
same track. Coordinate with departments to work on separate tracks, or
use `suture diff` before merging to preview changes:

```bash
suture diff "main" "vfx" --track "V1"
```

### Large timelines are slow

For timelines with over 10 000 clips, consider splitting into reels and
using batch operations:

```bash
suture batch export --episodes "s01e01_r1,s01e01_r2,s01e01_r3" \
  --format "otio" --output "delivery/"
```
