# Film/TV Post-Production Timeline Versioning

Track changes to your film edit timeline across cuts and review rounds.

## Problem

Film and TV post-production involves constant revisions. Directors give notes,
editors reorder scenes, and multiple versions pile up with names like
`final_cut_v3_REAL_FINAL.mp4`. Tracking what changed between versions is
painful and error-prone.

## Solution

Suture natively understands OpenTimelineIO (OTIO) files. It can diff timelines
to show exactly which clips were moved, trimmed, or reordered — not just
raw JSON differences. Combined with tags, you get a clear version history
of every cut.

## What This Example Demonstrates

- Committing an OTIO timeline as the base cut
- Creating review branches with director notes
- Modifying the timeline (reordering clips) on a separate branch
- Using `suture diff` to see structural changes between versions
- Tagging versions as `v1.0` and `v2.0`

## Running

```bash
make
```

## Expected Outcome

Two tagged versions (`v1.0` and `v2.0`) with a clear diff showing the scene
reorder between them. The log graph shows the branching and merge history.
