# Suture for Data Science

## The Problem

Data science workflows involve frequent experimentation — tweaking model parameters, trying new features, modifying data pipelines. Version control for these artifacts is inadequate:

- **Jupyter notebooks** are JSON files. Git's line-based merge garbles cell structure on conflicts.
- **CSVs and datasets** change row by row. Git sees entire-file replacements.
- **Config files** (YAML, TOML, JSON) store hyperparameters, feature flags, and environment settings. Concurrent edits produce merge conflicts.
- **Experiment tracking** is often done manually in spreadsheets or not at all.

## How Suture Helps

Suture understands the structure of data science artifacts:

| Format | What Suture does |
|--------|-----------------|
| JSON (`.json`) | Merge at the key level — concurrent hyperparameter changes don't conflict |
| YAML (`.yaml`) | Merge config maps, pipeline definitions, Kubernetes manifests |
| TOML (`.toml`) | Merge `[section]` tables independently |
| CSV (`.csv`) | Row-level merge with header detection — two people adding different rows merge cleanly |
| Markdown (`.md`) | Section-aware merge for notebooks exported as `.md` |

## Example: Branching an Experiment

```bash
suture init
suture config user.name "Data Scientist"

# Base experiment
echo 'model: random_forest
n_estimators: 100
max_depth: 10
features: [age, income, zip]' > config.yaml
suture add . && suture commit "baseline model"

# Branch: try a different model architecture
suture branch experiment/xgboost
suture checkout experiment/xgboost
# Edit config.yaml:
#   model: xgboost
#   n_estimators: 500
#   learning_rate: 0.01
suture add . && suture commit "xgboost with 500 trees"

# On main: try different features
suture checkout main
# Edit config.yaml:
#   features: [age, income, zip, credit_score, tenure]
suture add . && suture commit "add credit features"

# Compare experiments
suture diff main..experiment/xgboost config.yaml

 key: model
-  "random_forest"
+  "xgboost"

 key: n_estimators
-  100
+  500

+ key: learning_rate
+  0.01

# Merge — if you want to combine features + xgboost:
suture merge experiment/xgboost
# config.yaml now has xgboost model WITH the new features
# No conflict — different keys were changed on each branch
```

## CSV Merge Example

Two team members add rows to `training_data.csv`:

```bash
# Person A adds rows 101–150
# Person B adds rows 151–175

$ suture merge person-b
# Rows 101–175 all present. No conflict.
# Only a true row-level conflict (same row ID, different values) would flag.
```

## 5-Minute Setup

### 1. Install Suture

```bash
cargo install suture-cli
```

### 2. Initialize a project

```bash
mkdir ml-experiment && cd ml-experiment
suture init
suture config user.name "Your Name"
```

### 3. Track your experiment

```bash
echo 'model: logistic_regression
C: 1.0
solver: lbfgs' > config.yaml
echo 'feature,score
age,0.82
income,0.91' > feature_importance.csv
suture add . && suture commit "experiment 1: logistic regression baseline"
```

### 4. Branch, experiment, merge

```bash
suture branch exp/svm
suture checkout exp/svm
# Edit config.yaml: model → svm, kernel → rbf
suture add . && suture commit "experiment 2: SVM with RBF kernel"

suture checkout main
suture diff main..exp/svm
suture log --oneline
```

### 5. Use the daemon for automatic tracking

```bash
suture daemon start .
# Every file save is auto-committed
# Auto-syncs to remote if configured
# `suture status` shows what changed since last commit
```

## Comparison with Alternatives

| | Suture | Git | DVC | MLflow |
|---|---|---|---|---|
| Semantic JSON/YAML merge | Yes | No (line-based) | No | N/A |
| Semantic CSV merge | Row-level | No (whole-file) | No | N/A |
| Config branching/merging | Yes | Yes (with conflicts) | Yes (with conflicts) | Experiment tracking only |
| Model artifact tracking | Via any file type | Via Git LFS | Yes (native) | Yes (native) |
| No server required | Yes | Yes | Optional | Optional |
| Experiment comparison | `suture diff` across branches | `git diff` (line noise) | `dvc diff` | UI-based |

## Recommended Workflow

1. **One branch per experiment** — easy to compare, easy to discard.
2. **Config files in YAML/TOML** — Suture merges them semantically.
3. **Results in CSV** — row-level merge means concurrent result logging works.
4. **Use `suture blame`** — see who changed each hyperparameter and when.
5. **Daemon mode** — auto-commit every save so nothing is lost.

## See Also

- [Why Suture?](why-suture.md) — how semantic understanding works
- [Suture vs. Git](comparing-with-git.md) — honest comparison
- [Quick Start](quickstart.md) — general setup guide
