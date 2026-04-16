# Suture Demo: Semantic Merge in 60 Seconds

This demo shows Suture merging a JSON config file where Git would produce a conflict.

## Setup

```bash
# Create a demo workspace
mkdir suture-demo && cd suture-demo
suture init
suture config user.name "Demo User"

# Create a base config
cat > config.json << 'EOF'
{
  "database": {
    "host": "localhost",
    "port": 5432
  },
  "server": {
    "port": 8080,
    "workers": 4
  }
}
EOF
suture add .
suture commit "initial config"
```

## Create a Conflict Scenario

```bash
# Branch 1: Database team changes the DB host
suture branch database
suture checkout database
cat > config.json << 'EOF'
{
  "database": {
    "host": "db.production.example.com",
    "port": 5432
  },
  "server": {
    "port": 8080,
    "workers": 4
  }
}
EOF
suture add .
suture commit "point to production database"

# Branch 2: Server team changes the server config
suture checkout main
cat > config.json << 'EOF'
{
  "database": {
    "host": "localhost",
    "port": 5432
  },
  "server": {
    "port": 3000,
    "workers": 8
  }
}
EOF
suture add .
suture commit "increase workers and change port"
```

## The Magic: Merge

```bash
suture merge database
cat config.json
```

Expected output:
```json
{
  "database": {
    "host": "db.production.example.com",
    "port": 5432
  },
  "server": {
    "port": 3000,
    "workers": 8
  }
}
```

Both changes applied cleanly. The database team's host change and the server team's port/workers change are combined without conflict markers.

## Compare with Git

Try the same scenario in a Git repo — you'll get conflict markers on every changed line.

## YAML Demo (Bonus)

The same works for YAML:

```bash
cat > deployment.yaml << 'EOF'
apiVersion: apps/v1
kind: Deployment
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: app
          image: myapp:1.0
          ports:
            - containerPort: 8080
EOF
```

Two people changing `replicas` and `image` in parallel? No conflict.
