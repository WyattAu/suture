# CI Integration Guide

Suture works with any CI system. This guide covers GitHub Actions, GitLab CI, CircleCI, Jenkins, and generic bash environments.

## Overview

All integrations follow the same pattern:

1. Extract file content at three git refs: base (common ancestor), ours (current branch), theirs (incoming branch).
2. Send the three versions to the Suture merge API.
3. Write the merged result back to disk.
4. Optionally commit and push.

Two approaches are available:

- **GitHub Action** — pre-built, zero-config for GitHub.
- **Shell script** (`scripts/merge-ci.sh`) — portable, works everywhere.

## Supported Formats

DOCX, XLSX, PPTX, JSON, YAML, TOML, CSV, XML, Markdown, OTIO, SQL, PDF, and image formats (PNG, JPG, GIF, BMP, WebP, TIFF, ICO, AVIF).

---

## GitHub Actions

### Quick Start

```yaml
# .github/workflows/semantic-merge.yml
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

### Full Configuration

```yaml
- uses: WyattAu/suture-action@v1
  with:
    files: |
      package.json
      tsconfig.json
      .eslintrc.json
    api-url: https://merge.suture.dev/api
    api-token: ${{ secrets.SUTURE_API_TOKEN }}
    base-ref: origin/main
    ours-ref: HEAD
    theirs-ref: origin/${{ github.head_ref }}
    fail-on-conflict: true
    create-comment: true
```

### Outputs

| Output | Description |
|---|---|
| `merged-files` | Newline-separated list of successfully merged files |
| `conflict-files` | Newline-separated list of files with conflicts |
| `has-conflicts` | `true` if any conflicts were detected |
| `conflict-count` | Number of files with conflicts |

---

## GitLab CI

### Quick Start

Copy the example config:

```yaml
# .gitlab-ci.yml
stages:
  - merge-check

suture:merge:
  stage: merge-check
  image: node:20-alpine
  rules:
    - if: $CI_MERGE_REQUEST_IID
  before_script:
    - apk add --no-cache git jq
  script:
    - |
      git fetch origin $CI_MERGE_REQUEST_TARGET_BRANCH_NAME
      BASE_SHA=$(git merge-base origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME HEAD)
      bash scripts/merge-ci.sh \
        --files "package.json tsconfig.json Cargo.toml" \
        --base-ref "$BASE_SHA" \
        --theirs-ref "origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"
      git diff --quiet || {
        git config user.name "Suture Bot"
        git config user.email "suture-bot@example.com"
        git commit -am "Auto-merge structured files via Suture"
        git push origin HEAD:$CI_MERGE_REQUEST_SOURCE_BRANCH_NAME
      }
```

See the [GitLab CI Integration](gitlab-ci-integration.md) doc for full details.

---

## CircleCI

```yaml
# .circleci/config.yml
version: 2.1

orbs:
  node: circleci/node@5

jobs:
  suture-merge:
    docker:
      - image: cimg/base:stable
    steps:
      - checkout
      - run:
          name: Install dependencies
          command: sudo apt-get update && sudo apt-get install -y jq
      - run:
          name: Fetch and merge
          command: |
            git fetch origin $CIRCLE_BRANCH
            BASE_SHA=$(git merge-base origin/$CIRCLE_BRANCH HEAD)
            bash scripts/merge-ci.sh \
              --files "package.json tsconfig.json" \
              --base-ref "$BASE_SHA"
            git diff --quiet || {
              git config user.name "Suture Bot"
              git config user.email "suture-bot@example.com"
              git commit -am "Auto-merge structured files via Suture"
              git push origin HEAD
            }

workflows:
  merge-check:
    jobs:
      - suture-merge:
          filters:
            branches:
              ignore: main
```

---

## Jenkins

```groovy
// Jenkinsfile
pipeline {
    agent any

    environment {
        SUTURE_API_URL = 'https://merge.suture.dev/api'
    }

    stages {
        stage('Semantic Merge') {
            when {
                changeRequest()
            }
            steps {
                sh '''
                    BASE_SHA=$(git merge-base origin/${CHANGE_TARGET} HEAD)
                    bash scripts/merge-ci.sh \
                        --files "package.json tsconfig.json Cargo.toml" \
                        --base-ref "$BASE_SHA" \
                        --theirs-ref "origin/${CHANGE_TARGET}"
                    git diff --quiet || {
                        git config user.name "Suture Bot"
                        git config user.email "suture-bot@example.com"
                        git commit -am "Auto-merge structured files via Suture"
                    }
                '''
            }
        }
    }
}
```

---

## Generic / Any CI

The shell script works anywhere bash, curl, jq, and git are available:

```bash
bash scripts/merge-ci.sh \
    --files "package.json tsconfig.json pyproject.toml" \
    --base-ref HEAD~1 \
    --ours-ref HEAD \
    --theirs-ref origin/main \
    --api-url https://merge.suture.dev/api \
    --token $SUTURE_API_TOKEN \
    --no-fail
```

### Arguments

| Flag | Description | Default |
|---|---|---|
| `--files` | Space-separated list of files to merge (required) | — |
| `--driver` | Override auto-detected driver | auto |
| `--base-ref` | Git ref for the common ancestor | `HEAD~1` |
| `--ours-ref` | Git ref for our version | `HEAD` |
| `--theirs-ref` | Git ref for their version | same as ours |
| `--api-url` | Suture API endpoint | `https://merge.suture.dev/api` |
| `--token` | API token for higher rate limits | — |
| `--no-fail` | Continue on conflict instead of exiting with error | fail on conflict |

### Environment Variables

| Variable | Description |
|---|---|
| `SUTURE_API_URL` | Default API URL |
| `SUTURE_API_TOKEN` | Default API token |

### Supported Drivers

| Extension | Driver |
|---|---|
| `.json` | `json` |
| `.yaml`, `.yml` | `yaml` |
| `.toml` | `toml` |
| `.xml` | `xml` |
| `.csv` | `csv` |
| `.sql` | `sql` |
| `.html`, `.htm` | `html` |
| `.md` | `markdown` |
| `.svg` | `svg` |
| `.properties`, `.ini` | `properties` |

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | All files merged successfully |
| `1` | One or more files had unresolvable conflicts (unless `--no-fail` is set) |

---

## Self-Hosted Suture

Point `--api-url` or `SUTURE_API_URL` at your self-hosted instance:

```bash
SUTURE_API_URL="https://suture.internal.example.com/api" \
SUTURE_API_TOKEN="your-token" \
bash scripts/merge-ci.sh --files "package.json"
```
