# Suture for Video Editors

## The Problem

Non-linear editors (NLEs) like DaVinci Resolve, Adobe Premiere, and Avid Media Composer have no built-in version control. When a team shares a timeline, the workflow looks like this:

```
Project_v1.drp
Project_v1_JANE_edit.drp
Project_v1_JANE_edit_FINAL.drp
Project_v1_JANE_edit_FINAL_v2_REAL.drp
Project_v2_MIKE_color.drp
```

Nobody knows what changed between versions. Merging two editors' work means manually replaying edits by hand. There's no way to answer "what did Jane change in the color grade last Tuesday?"

## How Suture Helps

Suture understands [OpenTimelineIO](https://opentimelineio.io/) (OTIO) timelines — the industry-standard interchange format for editorial data. It tracks changes at the clip and track level:

- Clip reorders, trims, and replacements
- Grade and effect changes
- Audio track modifications
- Marker and metadata edits

When two editors work on the same timeline, Suture merges non-overlapping changes automatically. If Jane retimes a clip on V2 while Mike adjusts audio on A1, both changes land cleanly.

## Example: Timeline Diff

```
$ suture diff HEAD~1 timeline.otio

 track: V1
   clip "Interview_A" @ 00:01:12:00
-    duration: 00:00:15:00
+    duration: 00:00:12:08        ← trimmed 2s04f
+    effect: "FilmLook_v3"        ← new grade applied

 track: A1
   clip "Ambience_Wind"
-    gain: -6.0 dB
+    gain: -3.0 dB                ← raised 3 dB

2 clips changed, 0 conflicts
```

## 5-Minute Setup

### 1. Install Suture

```bash
cargo install suture-cli
```

### 2. Initialize a project

```bash
mkdir my-project && cd my-project
suture init
suture config user.name "Jane Editor"
```

### 3. Import your timeline

Export from your NLE as OTIO, or use the FUSE mount:

```bash
# Option A: Copy exported OTIO file
cp /path/to/export/timeline.otio .
suture add . && suture commit "initial assembly"

# Option B: Mount and work directly (editors save to the mount)
suture vfs mount . /mnt/project
# Now your NLE can open files from /mnt/project
```

### 4. Branch and experiment

```bash
suture branch try-alt-music
suture checkout try-alt-music
# Edit timeline — swap music track, adjust timing
suture add . && suture commit "alt music track with crossfade"

suture checkout main
# Make different edits on main
suture add . && suture commit "tighten act 2 by 4 seconds"

# Merge — non-overlapping edits combine cleanly
suture merge try-alt-music
```

### 5. Review history

```bash
suture log --oneline
suture diff HEAD~2 timeline.otio
suture blame timeline.otio   # who changed each clip, when
```

## Comparison

| | Suture | Manual file naming | No version control |
|---|---|---|---|
| See what changed | Yes — clip-level diff | No — must open both files | No |
| Merge two editors' work | Automatic for non-overlapping edits | Manual replay | Manual replay |
| History / blame | Full commit log with per-clip attribution | Folder full of files | None |
| Undo a specific change | `suture revert` | Find old file, copy back | Impossible |
| Collaboration | Push/pull via Suture Hub | Dropbox/Drive sync | Drive/email |

## NLE Integration

- **FUSE mount**: Mount the Suture repo as a directory. Your NLE opens and saves files normally. Every save becomes a commit.
- **WebDAV mount**: Cross-platform alternative (macOS Finder, Windows Explorer).
- **OTIO round-trip**: Export from Premiere/Resolve as OTIO, version with Suture, re-import.

## See Also

- [Why Suture?](why-suture.md) — how semantic understanding works
- [Suture vs. Git](comparing-with-git.md) — honest comparison
- [Quick Start](quickstart.md) — general setup guide
