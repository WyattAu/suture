# Git Merge Driver

Suture can act as a **Git merge driver**, giving Git the ability to perform semantic merges on structured files instead of its default line-based three-way merge. When both sides of a merge edit different parts of a JSON config, YAML manifest, or TOML table, Suture merges them cleanly — no conflict markers.

## Prerequisites

- **suture-cli** installed and on your `PATH`, or set the `SUTURE_PATH` environment variable.

  ```bash
  cargo install suture-cli
  ```

- Git 2.0+ (any reasonably modern version).

## Quick Setup

Three commands in your repository:

```bash
git config merge.suture.name "Suture semantic merge"
git config merge.suture.driver "$(pwd)/contrib/git-merge-driver/suture-merge-driver %O %A %B"
echo -e "*.json merge=suture\n*.yaml merge=suture\n*.yml merge=suture\n*.toml merge=suture\n*.csv merge=suture\n*.xml merge=suture\n*.md merge=suture" >> .gitattributes
```

Commit `.gitattributes` and you're done. Future merges on those file types will use Suture.

## Per-Repository Setup

The `git config` commands above set the driver for the current repository only. To configure it globally for every repo:

```bash
git config --global merge.suture.name "Suture semantic merge"
git config --global merge.suture.driver "/absolute/path/to/suture-merge-driver %O %A %B"
```

Then add `.gitattributes` entries in each repo where you want semantic merging.

## Per-File-Type Configuration

Only the file types listed in `.gitattributes` are routed to Suture. Add or remove lines as needed:

```gitattributes
# Use Suture for structured formats
*.json merge=suture
*.yaml merge=suture
*.yml  merge=suture
*.toml merge=suture
*.csv  merge=suture
*.xml  merge=suture
*.md   merge=suture

# Everything else uses Git's default merge (no entry needed)
```

You can also use path-based patterns:

```gitattributes
# Only specific directories
kubernetes/*.yaml merge=suture
configs/*.toml merge=suture
```

## How It Works

1. When `git merge` encounters a conflict on a file matching `.gitattributes`, Git writes three temporary files — base (`%O`), ours (`%A`), and theirs (`%B`) — and invokes the merge driver script.
2. The script detects the file extension and runs `suture merge-file --driver <ext>` with the three files.
3. **Clean merge**: Suture writes the merged result to the ours file and exits 0. Git accepts it as resolved.
4. **Semantic conflict**: If Suture's semantic merge reports a conflict, the script falls back to line-based merge. If that also fails, it exits 1 and Git falls back to inserting standard conflict markers.
5. **Unknown extension**: The script copies theirs over ours and exits 0 (standard Git behavior for text files).

## Example Workflow

```bash
# 1. Set up the driver
git config merge.suture.name "Suture semantic merge"
git config merge.suture.driver "$(pwd)/contrib/git-merge-driver/suture-merge-driver %O %A %B"
echo "*.json merge=suture" >> .gitattributes
git add .gitattributes && git commit -m "configure suture merge driver"

# 2. Create a base config and commit it
cat > config.json << 'EOF'
{
  "database": {"host": "localhost", "port": 5432},
  "server": {"port": 3000, "workers": 4}
}
EOF
git add config.json && git commit -m "initial config"

# 3. Branch: coworker changes database host
git checkout -b feature/db-config
cat > config.json << 'EOF'
{
  "database": {"host": "db.example.com", "port": 5432},
  "server": {"port": 3000, "workers": 4}
}
EOF
git add config.json && git commit -m "point database to staging"

# 4. Main: you change server port
git checkout main
cat > config.json << 'EOF'
{
  "database": {"host": "localhost", "port": 5432},
  "server": {"port": 8080, "workers": 4}
}
EOF
git add config.json && git commit -m "change server port"

# 5. Merge — no conflict!
git merge feature/db-config
cat config.json
# {"database":{"host":"db.example.com","port":5432},"server":{"port":8080,"workers":4}}
```

Without the merge driver, Git would produce conflict markers because both sides modified the same file. With Suture, it merges at the field level and combines both changes cleanly.

## Troubleshooting

**"suture: command not found"**

Make sure `suture` is on your `PATH`, or set `SUTURE_PATH` to the binary location:

```bash
export SUTURE_PATH=/usr/local/bin/suture
```

**Driver not being invoked**

Check that `.gitattributes` is committed and tracked. Git ignores uncommitted `.gitattributes` entries. Verify with:

```bash
git check-attr -a -- config.json
```

You should see `merge: suture`.

**Conflicts still appearing on structured files**

The driver only runs when Git detects a conflict. If the changes don't overlap at the line level, Git merges them without invoking the driver. This is normal — the driver is a fallback for cases Git can't handle.

**Script permission denied**

Make the script executable:

```bash
chmod +x contrib/git-merge-driver/suture-merge-driver
```
