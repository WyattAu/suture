# Suture Onboarding Guide

## First-Time User Experience

### Step 1: Install
```bash
curl -sSL https://suture.dev/install.sh | sh
```

### Step 2: Initialize a Repository
```bash
mkdir my-project && cd my-project
suture init
```

### Step 3: Make Changes
Edit any structured files (JSON, YAML, TOML, CSV, etc.)

### Step 4: Commit
```bash
suture add .
suture commit "my first structured commit"
```

### Step 5: Branch & Merge
```bash
suture checkout -b feature
# make changes on both branches
suture merge main
# Semantic merge resolves conflicts automatically!
```

### Step 6: View History
```bash
suture log --graph
suture diff main..HEAD
```

### CI/CD Integration
Add to `.github/workflows/merge.yml`:
```yaml
steps:
  - uses: WyattAu/suture/.github/actions/merge@v5
    with:
      formats: "json,yaml,toml,csv"
```
