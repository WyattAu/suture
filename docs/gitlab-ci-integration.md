# GitLab CI Integration

Run Suture's semantic merge on merge requests to automatically resolve conflicts in structured files before they block your pipeline.

## Overview

The GitLab CI integration runs on every merge request and:

- Detects the merge base between source and target branches
- Sends base, ours, and theirs file versions to the Suture API
- Writes the merged result back to the working tree
- Commits and pushes resolved files

## Prerequisites

- GitLab CI runner with network access to the Suture API
- `curl` and `jq` installed on the runner (included in the `node:20-alpine` image)
- A Suture API endpoint (cloud or self-hosted)

## Setup

1. Copy the example config into your project:

   ```bash
   cp .gitlab-ci.yml.example .gitlab-ci.yml
   ```

2. Adjust the `FILES` variable to list the structured files you want to merge semantically.

3. Commit and push. The job will run automatically on merge requests.

## Configuration

### API URL

```yaml
variables:
  SUTURE_API_URL: "https://merge.suture.dev/api"
```

For self-hosted instances:

```yaml
variables:
  SUTURE_API_URL: "https://suture.example.com/api"
```

### API Token

Add a CI/CD variable in your GitLab project (Settings → CI/CD → Variables):

```
SUTURE_API_TOKEN = <your-token>
```

Then reference it in the job:

```yaml
variables:
  SUTURE_API_TOKEN: $SUTURE_API_TOKEN
```

### Files to Merge

Edit the `FILES` variable in the script:

```bash
FILES="package.json tsconfig.json .eslintrc.json Cargo.toml pyproject.toml"
```

### Driver Override

The example auto-detects the driver from the file extension. To override, change the `"driver"` field in the API call.

## How It Works

1. The job runs only on merge requests (`$CI_MERGE_REQUEST_IID`).
2. `git merge-base` finds the common ancestor commit.
3. For each file, `git show` extracts the content at each ref (base, ours, theirs).
4. The Suture API performs a semantic three-way merge.
5. If successful, the merged content overwrites the file in the working tree.
6. If files changed, the job commits and pushes to the source branch.

## Self-Hosted Suture Instance

If you run your own Suture server:

```yaml
variables:
  SUTURE_API_URL: "https://suture.internal.example.com/api"
  SUTURE_API_TOKEN: "$CI_SUTURE_TOKEN"
```

Make sure your runner can reach the Suture server. For private networks, use a runner with appropriate network access or a VPN.

## Using the Standalone Script

Alternatively, use the portable shell script instead of inline YAML:

```yaml
suture:merge:
  stage: merge-check
  image: alpine:3.19
  before_script:
    - apk add --no-cache git curl jq bash
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
  rules:
    - if: $CI_MERGE_REQUEST_IID
```

## Troubleshooting

### Job fails with "merge-base" error

The target branch may not be fetched. Add:

```yaml
variables:
  GIT_DEPTH: "0"
```

Or fetch explicitly in `before_script`:

```bash
git fetch origin $CI_MERGE_REQUEST_TARGET_BRANCH_NAME
```

### "jq: command not found"

Install `jq` in the runner image or use a different base image:

```yaml
image: node:20-alpine  # includes curl, jq
```

### API returns 401 / 403

Check that `SUTURE_API_TOKEN` is set as a CI/CD variable (masked, not file type).

### Files not being merged

Ensure the files exist at the merge base ref. If a file was created after branching, `git show` returns empty and the merge proceeds with the base as empty content.

### Push fails (403)

The job needs write access. In Settings → Repository → Protected Branches, either:
- Allow push from the pipeline user, or
- Use an access token with `api` scope via `git push https://oauth2:$TOKEN@gitlab.com/...`
