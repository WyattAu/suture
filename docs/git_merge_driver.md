# Git Merge Driver

Suture can act as a **Git merge driver**, giving Git the ability to perform semantic merges on structured files instead of its default line-based three-way merge. When both sides of a merge edit different parts of a JSON config, YAML manifest, DOCX paragraph, or spreadsheet cell, Suture merges them cleanly — no conflict markers.

## Quick Setup (1 minute)

```bash
# Install suture (if not already)
cargo install suture-cli

# In your Git repo:
suture git driver install

# Commit the configuration
git add .gitattributes .suture/git-merge-driver.sh
git commit -m "Configure suture merge driver"
```

That's it. Future merges on 20+ file types will use Suture's semantic merge automatically.

## What Gets Merged Semantically

| Format | Extensions | Merge Granularity |
|--------|-----------|-------------------|
| JSON | `.json` `.jsonl` | Field-level (RFC 6901 paths) |
| YAML | `.yaml` `.yml` | Key-level |
| TOML | `.toml` | Table and key-aware |
| CSV | `.csv` `.tsv` | Row-level with header detection |
| XML | `.xml` `.xsl` `.svg` | Element/attribute-aware |
| Markdown | `.md` `.markdown` | Section-aware |
| DOCX | `.docx` `.docm` | Paragraph-level (preserves formatting) |
| XLSX | `.xlsx` `.xlsm` | Cell-level (preserves formulas) |
| PPTX | `.pptx` `.pptm` | Slide-level (preserves formatting) |
| OTIO | `.otio` | Clip-level (video timeline merge) |
| SQL | `.sql` | DDL schema diff |

Files without a driver fall back to Git's default line-based merge.

## Example: JSON Config

```bash
# Base config
cat > config.json << 'EOF'
{
  "database": {"host": "localhost", "port": 5432},
  "server": {"port": 3000, "workers": 4}
}
EOF
git add config.json && git commit -m "initial config"

# Branch: coworker changes database host
git checkout -b feature/db-config
cat > config.json << 'EOF'
{
  "database": {"host": "db.example.com", "port": 5432},
  "server": {"port": 3000, "workers": 4}
}
EOF
git add config.json && git commit -m "point database to staging"

# Main: you change server port
git checkout main
cat > config.json << 'EOF'
{
  "database": {"host": "localhost", "port": 5432},
  "server": {"port": 8080, "workers": 4}
}
EOF
git add config.json && git commit -m "change server port"

# Merge — no conflict!
git merge feature/db-config
# config.json now has BOTH changes:
# {"database":{"host":"db.example.com","port":5432},"server":{"port":8080,"workers":4}}
```

Without Suture, Git would produce conflict markers because both sides modified the same lines. With Suture, it merges at the field level.

## Example: DOCX Document

```bash
# Two people edit different paragraphs of a Word document
# Person A changes paragraph 1, person B changes paragraph 3
git merge feature/edits
# Suture merges both paragraph changes — no binary conflict
```

## How It Works

1. When `git merge` encounters a conflict on a file matching `.gitattributes`, Git writes three temporary files — base (`%O`), ours (`%A`), and theirs (`%B`) — and invokes the merge driver.
2. `suture merge-file --driver auto` detects the file type and runs the appropriate semantic driver.
3. **Clean merge**: Suture writes the merged result and exits 0. Git accepts it.
4. **Semantic conflict**: If the semantic driver can't auto-resolve (e.g., both sides edited the same JSON field), Suture falls back to line-based merge. If that also fails, Git inserts standard conflict markers.

## Manual Setup

If you prefer to configure manually instead of using `suture git driver install`:

```bash
# Register the driver (per-repo or --global)
git config merge.suture.name "Suture semantic merge"
git config merge.suture.driver "/path/to/contrib/git-merge-driver/suture-merge-driver %O %A %B %P"
git config merge.suture.recursive "binary"

# Tell Git which file types to route through Suture
echo "*.json merge=suture" >> .gitattributes
echo "*.yaml merge=suture" >> .gitattributes
echo "*.yml merge=suture" >> .gitattributes
echo "*.toml merge=suture" >> .gitattributes
echo "*.csv merge=suture" >> .gitattributes
echo "*.xml merge=suture" >> .gitattributes
echo "*.md merge=suture" >> .gitattributes
echo "*.docx merge=suture" >> .gitattributes
echo "*.xlsx merge=suture" >> .gitattributes
echo "*.pptx merge=suture" >> .gitattributes

git add .gitattributes && git commit -m "Configure suture merge driver"
```

You can also use path-based patterns:

```gitattributes
# Only specific directories
kubernetes/*.yaml merge=suture
configs/*.toml merge=suture
```

## Troubleshooting

**"suture: command not found"**

Make sure `suture` is on your `PATH`:

```bash
which suture
# or set:
export SUTURE_PATH=/usr/local/bin/suture
```

**Driver not being invoked**

Check that `.gitattributes` is committed:

```bash
git check-attr -a -- config.json
# Should show: merge: suture
```

**Conflicts still appearing on structured files**

The driver only runs when Git detects a conflict at the line level. If Git can merge the lines without conflict (even if the semantic content is wrong), it won't invoke the driver. This is normal Git behavior.

**Binary files (DOCX, XLSX, PPTX) showing as conflicts**

Make sure you have `merge.suture.recursive` set to `binary`:

```bash
git config merge.suture.recursive binary
```

This tells Git to pass binary files to the merge driver instead of treating them as unmergeable.

## Standalone Usage

You can also use `suture merge-file` directly outside of Git:

```bash
# Merge three versions of a file
suture merge-file base.json ours.json theirs.json

# Specify a driver explicitly
suture merge-file --driver json base.json ours.json theirs.json

# Write output to a file
suture merge-file --driver docx base.docx ours.docx theirs.docx -o merged.docx

# Use labels for conflict markers
suture merge-file --label-ours HEAD --label-theirs feature base.json ours.json theirs.json
```
