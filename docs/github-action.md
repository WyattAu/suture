# Suture Semantic Merge GitHub Action

Automatically resolve merge conflicts on structured files using Suture's semantic merge drivers.

## Quick Start

1. Create `.github/workflows/semantic-merge.yml` in your repo:

```yaml
name: Semantic Merge
on:
  pull_request:
    types: [opened, synchronize, reopened]
jobs:
  semantic-merge:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: WyattAu/suture-action@v1
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
```

2. That's it! The action will automatically:
   - Detect merge conflicts on PRs
   - Run Suture's semantic merge drivers on conflicting files
   - Commit resolved files
   - Comment on the PR with results

## Supported Formats

DOCX, XLSX, PPTX, JSON, YAML, TOML, CSV, XML, Markdown, OTIO, SQL, PDF, and image formats (PNG, JPG, GIF, BMP, WebP, TIFF, ICO, AVIF).

## Configuration

No configuration needed. The action auto-detects file types and uses the appropriate semantic driver.

## How It Works

1. When a PR has conflicts, the action checks out both branches
2. For each conflicting file, it extracts the base, ours, and theirs versions
3. It runs `suture merge-file` with the appropriate semantic driver
4. If the merge succeeds, it stages the resolved file
5. If the merge fails (binary files, complex conflicts), it keeps conflict markers
6. It commits resolved files and comments on the PR

## Permissions

The action requires the following permissions:

- `contents: write` — to commit resolved files and push to the PR branch
- `pull-requests: write` — to post comments on the PR

## Manual Trigger

The workflow supports `workflow_dispatch`, so you can also trigger it manually from the GitHub Actions tab.
