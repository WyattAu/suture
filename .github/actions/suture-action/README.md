# Suture Semantic Merge Action

Automatically resolve merge conflicts in structured files (JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX) using [Suture](https://github.com/WyattAu/suture) semantic merge.

## How it works

1. **Installs Suture** — tries `cargo install suture-cli` first (fastest if cached), then falls back to building from source.
2. **Configures a git merge driver** — registers Suture as a custom merge driver for each requested file format.
3. **Writes `.gitattributes`** — tells git to route matching files through the Suture driver during merges.
4. **Performs the merge** — when git encounters a conflict in a supported format, Suture resolves it semantically (understanding the file structure) instead of leaving conflict markers.
5. **Falls back gracefully** — if Suture cannot resolve a conflict, git falls back to standard text-based merge.

## Supported formats

| Format   | Extensions                    |
|----------|-------------------------------|
| JSON     | `*.json`                      |
| YAML     | `*.yaml`, `*.yml`            |
| TOML     | `*.toml`                      |
| CSV      | `*.csv`                       |
| XML      | `*.xml`                       |
| Markdown | `*.md`, `*.markdown`         |
| DOCX     | `*.docx`                      |
| XLSX     | `*.xlsx`                      |
| PPTX     | `*.pptx`                      |

## Inputs

| Input              | Default                                              | Description                                                      |
|--------------------|------------------------------------------------------|------------------------------------------------------------------|
| `formats`          | `json,yaml,toml,csv,xml,markdown`                    | Comma-separated list of formats to handle                         |
| `working-directory`| `.`                                                  | Directory to run in                                               |
| `fail-on-conflict` | `true`                                               | Fail the workflow if semantic merge cannot resolve a conflict      |

## Example workflow

```yaml
name: Auto-merge
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  auto-merge:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Auto-resolve structured conflicts
        uses: WyattAu/suture/.github/actions/suture-action@v4.0.0
        with:
          formats: 'json,yaml,toml'
          fail-on-conflict: 'false'

      - name: Push resolution
        run: |
          git diff --quiet && echo "No conflicts to resolve" && exit 0
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git commit -am "Auto-resolve structured merge conflicts"
          git push
```

## How to use

1. Reference this action in your workflow using the local path within the repo.
2. Set `fetch-depth: 0` on the checkout step so the full history is available for merging.
3. Configure the `formats` input to match the file types in your project.
4. Set `fail-on-conflict` to `false` if you want the workflow to continue even when Suture can't resolve a conflict.
5. Follow with a commit/push step to persist resolved conflicts on the PR branch.
