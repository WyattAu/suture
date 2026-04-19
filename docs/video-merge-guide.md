# Video Merge Guide: Version Control for OTIO Timelines

A deep dive into how video editing teams use Suture for collaborative version control of OpenTimelineIO timelines.

## The Problem: Version Control for Video Timelines

Non-linear editors — DaVinci Resolve, Adobe Premiere Pro, Avid Media Composer — store projects as opaque binary files. A `.drp`, `.prproj`, or `.avp` is a monolithic blob that can't be meaningfully diffed or merged. Open your project file in a hex editor and you'll see compressed or proprietary data, not something a VCS can reason about.

Even when you export to OpenTimelineIO (OTIO) — the industry-standard interchange format for editorial timelines — traditional version control still falls short. OTIO files are JSON, but they're deeply nested JSON describing clips, tracks, transitions, rational time values, and metadata. A Git diff of an OTIO timeline is unintelligible:

```json
-            "duration": {"value": 500.0, "rate": 24.0}
+            "duration": {"value": 600.0, "rate": 24.0}
```

That could be any clip on any track. There's no indication of *which* clip changed, *where* it sits in the timeline, or *why* the change matters. When two editors work on the same OTIO file and both modify different clips, Git sees overlapping line changes and produces conflict markers. Collaborative editing is reduced to file locking and manual replay.

## How Suture Handles OTIO Timelines

Suture's `suture-driver-otio` parses the OTIO JSON schema into a structured element tree. It understands [OpenTimelineIO](https://opentimelineio.io/) types: `Timeline`, `Track`, `Clip`, `Transition`, and `Stack`. Each element gets a deterministic hierarchical ID:

```
0:timeline:FeatureFilm_RoughCut
  0:timeline:FeatureFilm_RoughCut/0:track:V1_Main
    0:timeline:FeatureFilm_RoughCut/0:track:V1_Main/0:clip:Scene1_Take3
    0:timeline:FeatureFilm_RoughCut/0:track:V1_Main/1:transition:Dissolve_1
    0:timeline:FeatureFilm_RoughCut/0:track:V1_Main/2:clip:Scene2_Take1
  0:timeline:FeatureFilm_RoughCut/1:track:V2_BRoll
  0:timeline:FeatureFilm_RoughCut/2:track:A1_Dialogue
```

Changes are mapped to these element addresses. The driver uses **touch-set propagation**: when a track is modified, all its child clips and transitions are included in the patch's touch set. This means changing a track name and changing a clip on that track are correctly detected as overlapping changes.

Three-way merge operates at the element level. If Editor A trims a clip on `V1_Main` (address `.../0:track:V1_Main/0:clip:Scene1_Take3`) and Editor B adds a clip to `A1_Dialogue` (address `.../2:track:A1_Dialogue`), the touch sets are disjoint and both changes merge cleanly. Reordering clips within a track changes the positional index, which causes the touch sets to overlap — this is correctly flagged as a conflict because both editors modified the same structural region.

## Workflow: Editing with Suture

### 1. Initialize a video-aware repository

```bash
suture init --type video
suture config user.name "Jane Editor"
```

### 2. Import your timeline

Export from your NLE as OTIO and commit:

```bash
cp /path/to/export/timeline.otio .
suture add timeline.otio && suture commit "initial assembly"
```

### 3. Branch for parallel work

```bash
suture branch editor-B && suture checkout editor-B
```

### 4. Edit, commit, merge

Editor B adjusts audio levels on `A1_Dialogue`, then commits:

```bash
suture add timeline.otio && suture commit "raise dialogue 3dB"
```

Back on main, you trim Act 2 and commit:

```bash
suture checkout main
# Edit timeline — trim clips on V1_Main
suture add timeline.otio && suture commit "tighten Act 2"
```

Merge cleanly:

```bash
$ suture merge editor-B

 merge editor-B into main

 track: V1_Main
   clip "Scene2_Take1" trimmed 00:00:04:00
 track: A1_Dialogue
   clip "Boom_Scene1" gain raised +3.0 dB

 2 clips changed, 0 conflicts
```

Both edits land without conflict — different tracks, different touch sets.

### 5. Inspect history

```bash
$ suture log --oneline
e7f2a1  raise dialogue 3dB
b3c9d4  tighten Act 2
4f2d0b  initial assembly

$ suture diff HEAD~1 timeline.otio
 track: A1_Dialogue
   clip "Boom_Scene1"
-    duration.value: 480.0
+    duration.value: 480.0
     metadata.gain: -3.0 dB → 0.0 dB
```

## Supported Scenarios

| Scenario | Result |
|----------|--------|
| Non-overlapping track edits | Clean merge — both editors' clips preserved |
| Same-track clip edits | Conflict — editors resolve manually |
| New track addition | Clean merge — existing tracks + new track |
| Metadata changes (name, color space) | Clean merge — independent fields |
| Nested stack restructuring | Detected at depth — child elements tracked individually |
| Clip reordering within a track | Conflict — positional index overlap |

## Integration Points

**VS Code extension**: OTIO files get semantic diff coloring. Open a `.otio` file in VS Code and see clip-level additions, removals, and modifications with track-aware grouping — not raw JSON noise.

**Git merge driver**: Install Suture as a Git merge driver to get OTIO-aware merging in any Git repository:

```bash
suture git driver install
```

This configures `.gitattributes` to route `.otio` files through `suture merge-file`, so `git merge` produces clean merges instead of conflict markers when two branches modify different parts of the same timeline.

**CI/CD**: The [Suture GitHub Action](github-action.md) runs semantic merge on PRs that touch OTIO files. If a PR introduces changes to `V2_BRoll` while `main` has changes to `A1_Dialogue`, the action auto-resolves the merge and posts a comment with the results.

## See Also

- [Suture for Video Editors](video-editors.md) — high-level overview
- [Semantic Merge](semantic-merge.md) — how Suture's merge engine works
- [Git Merge Driver](git_merge_driver.md) — using Suture inside Git
- [CLI Reference](cli-reference.md) — full command listing
