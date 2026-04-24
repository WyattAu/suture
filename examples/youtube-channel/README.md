# YouTube Channel Metadata Management

Never lose a video description, thumbnail spec, or metadata again.

## Problem

Managing a YouTube channel means juggling video titles, descriptions, tags,
playlists, and channel metadata. Changes happen constantly — SEO updates,
rebranding, new uploads. Without version control, it's impossible to track
what changed and when.

## Solution

Suture stores your channel and video metadata as versioned JSON files. You can
branch for experiments (like SEO A/B tests), merge changes from collaborators,
and roll back to any previous version instantly.

## What This Example Demonstrates

- Committing a full channel structure (metadata, videos, playlists)
- Creating an `seo-update` branch to improve video descriptions
- Creating a `new-video` branch to add a new video entry
- Merging both branches — semantic merge handles JSON conflicts gracefully
- Viewing the commit graph to see the full history

## Running

```bash
make
```

## Expected Outcome

Both branches merge cleanly. The final state includes the updated SEO
descriptions and the new video, with no data lost in the merge.
