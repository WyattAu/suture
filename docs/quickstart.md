# Quickstart

Get started with Suture in under 5 minutes.

## Install

**From source (requires Rust 1.85+):**

```bash
cargo install --path crates/suture-cli
```

**Build from repo:**

```bash
git clone https://github.com/WyattAu/suture.git
cd suture
cargo build --release --bin suture
# Binary at target/release/suture
```

## First Repo

```bash
suture init my-project
cd my-project
suture config user.name "Your Name"

echo "Hello, Suture" > README.md
suture add .
suture commit "initial commit"
suture log
```

## Branching and Merging

```bash
suture branch feature
suture checkout feature

echo "new feature" >> README.md
suture add .
suture commit "add feature"

suture checkout main
suture merge feature
```

## Remote and Hub

```bash
suture remote add origin http://localhost:50051
suture push
suture pull
```

Start a hub:

```bash
suture-hub --db hub.db
# Web UI at http://localhost:50051
```

Clone from a hub:

```bash
suture clone http://localhost:50051/my-project
```

## Semantic Merge Demo

Two developers edit different keys in the same JSON file -- no conflict.

```bash
# Base: create config.json and commit on main
echo '{"database": {"host": "localhost", "port": 5432}}' > config.json
suture add .
suture commit "base config"

# Developer A: change host on a branch
suture branch dev-a
suture checkout dev-a
echo '{"database": {"host": "db.prod.internal", "port": 5432}}' > config.json
suture add . && suture commit "point to production db"

# Developer B: change port on main
suture checkout main
echo '{"database": {"host": "localhost", "port": 3306}}' > config.json
suture add . && suture commit "switch to mysql port"

# Merge -- both changes applied cleanly
suture merge dev-a
cat config.json
# {"database": {"host": "db.prod.internal", "port": 3306}}
```

Suture parsed the JSON, detected that `host` and `port` are independent keys, and merged both changes. No conflict markers.

## Next Steps

- [Semantic Merge Deep Dive](semantic-merge.md)
- [CLI Reference](cli-reference.md)
- [Hub Guide](hub.md)
