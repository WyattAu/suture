# 5-Minute Git Merge Driver Quickstart

Stop losing work to Git merge conflicts on JSON, YAML, DOCX, XLSX, and 16 other file formats. This guide gets you from zero to conflict-free in 5 minutes.

---

## Step 1: Install (30 seconds)

**macOS (Homebrew):**
```bash
brew tap WyattAu/suture-merge-driver
brew install suture-merge-driver
```

**Linux / Windows:**
Download from [github.com/WyattAu/suture/releases](https://github.com/WyattAu/suture/releases). Unzip and add to your PATH.

**Verify:**
```bash
suture --version
# suture 5.0.1
```

---

## Step 2: Install the Merge Driver (30 seconds)

In your Git repository:
```bash
suture git driver install
```

This creates two files:
- `.gitattributes` — tells Git which file types to merge semantically (20 patterns)
- `.suture/git-merge-driver.sh` — the bridge script that Git calls on conflicts

Commit them:
```bash
git add .gitattributes .suture/git-merge-driver.sh
git commit -m "Configure suture semantic merge driver"
```

**That's it.** You're done. Every future `git merge` on supported files will use semantic merge.

---

## Step 3: See It Work (1 minute)

Here's a JSON config that two people edit simultaneously:

```bash
# Create a shared config
cat > config.json << 'EOF'
{
  "database": {"host": "localhost", "port": 5432},
  "server": {"port": 3000, "workers": 4},
  "logging": {"level": "info", "file": "app.log"}
}
EOF
git add config.json && git commit -m "initial config"

# --- Your coworker's branch ---
git checkout -b coworker/db-host
cat > config.json << 'EOF'
{
  "database": {"host": "db.example.com", "port": 5432},
  "server": {"port": 3000, "workers": 4},
  "logging": {"level": "info", "file": "app.log"}
}
EOF
git add config.json && git commit -m "point database to staging"

# --- Your branch (main) ---
git checkout main
cat > config.json << 'EOF'
{
  "database": {"host": "localhost", "port": 5432},
  "server": {"port": 8080, "workers": 8},
  "logging": {"level": "info", "file": "app.log"}
}
EOF
git add config.json && git commit -m "update server config"
```

Now merge:
```bash
git merge coworker/db-host
```

**Without Suture:** Git produces conflict markers because both sides modified the same lines.

**With Suture:** Clean merge. The result contains *both* changes:
```json
{
  "database": {"host": "db.example.com", "port": 5432},
  "server": {"port": 8080, "workers": 8},
  "logging": {"level": "info", "file": "app.log"}
}
```

---

## Step 4: Try a Word Document (1 minute)

The same thing works for `.docx` files — Suture merges at the *paragraph* level:

```bash
# Create a branch, edit paragraph 1, commit
# Switch back, edit paragraph 3, commit
# Merge — both changes preserved, no binary conflict
git merge feature/edits
```

---

## Supported File Types

| Format | Extension(s) | Merge Granularity |
|--------|-------------|-------------------|
| JSON | `.json` `.jsonl` | Field-level |
| YAML | `.yaml` `.yml` | Key-level |
| TOML | `.toml` | Table/key-aware |
| CSV/TSV | `.csv` `.tsv` | Row-level |
| XML | `.xml` `.xsl` `.svg` | Element/attribute |
| Markdown | `.md` `.markdown` | Section-aware |
| Word | `.docx` `.docm` | Paragraph-level |
| Excel | `.xlsx` `.xlsm` | Cell-level |
| PowerPoint | `.pptx` `.pptm` | Slide-level |
| SQL | `.sql` | DDL schema |
| OTIO | `.otio` | Clip-level |

Files without a driver fall back to Git's default line-based merge.

---

## Global Install (Optional)

To enable semantic merge across *all* your repos without per-repo setup:

```bash
# Copy the driver script to a fixed location
sudo cp $(which suture) /usr/local/bin/suture

# Configure Git globally
git config --global merge.suture.name "Suture Semantic Merge"
git config --global merge.suture.driver "suture merge-file %O %A %B %P"
git config --global merge.suture.recursive "binary"

# Create a global gitattributes
echo "*.json merge=suture" >> ~/.gitattributes
echo "*.yaml merge=suture" >> ~/.gitattributes
echo "*.yml merge=suture" >> ~/.gitattributes
echo "*.toml merge=suture" >> ~/.gitattributes
echo "*.csv merge=suture" >> ~/.gitattributes
echo "*.md merge=suture" >> ~/.gitattributes
echo "*.docx merge=suture" >> ~/.gitattributes
echo "*.xlsx merge=suture" >> ~/.gitattributes
echo "*.pptx merge=suture" >> ~/.gitattributes
```

---

## Uninstall

```bash
suture git driver uninstall
git add .gitattributes && git commit -m "Remove suture merge driver"
```

---

## What's Next?

- [Full CLI Reference](cli-reference.md)
- [Semantic Merge Deep Dive](semantic-merge.md)
- [Document Authors Guide](document-authors.md)
- [Install without Homebrew](getting_started.md)
