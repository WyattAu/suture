# suture-merge-driver

A Git merge driver that semantically merges structured files — JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, and more — instead of falling back to noisy line-based conflict markers.

## Install

```bash
pip install suture-merge-driver
```

Requires Python >= 3.8. On install, the appropriate platform binary is downloaded automatically from GitHub Releases.

## Configure Git

```bash
git config merge.suture.name "Suture semantic merge"
git config merge.suture.driver "suture-merge-driver %O %A %B %P"
echo "*.json merge=suture" >> .gitattributes
echo "*.yaml merge=suture" >> .gitattributes
echo "*.yml merge=suture" >> .gitattributes
```

Add more file types as needed:

```bash
echo "*.toml merge=suture" >> .gitattributes
echo "*.xml merge=suture" >> .gitattributes
echo "*.csv merge=suture" >> .gitattributes
echo "*.md merge=suture" >> .gitattributes
```

## How it works

When a merge conflict occurs on a configured file, Git invokes `suture-merge-driver %O %A %B %P`:

- `%O` — the base (ancestor) version
- `%A` — ours (your changes) — Git reads the result from here
- `%B` — theirs (their changes)
- `%P` — the original file path (used to detect format)

Suture parses both sides at the semantic level, applies a three-way merge, and writes the resolved result back. If the merge succeeds, Git uses the result. If not, Git falls back to its default conflict behavior.

### Before — line-based merge

```json
<<<<<<< ours
{
  "database": {
    "host": "db.prod.example.com",
    "port": 5432,
    "ssl": true
  },
  "logging": {
    "level": "warn"
  }
}
=======
{
  "database": {
    "host": "db.prod.example.com",
    "port": 5432,
    "max_connections": 100
  },
  "logging": {
    "level": "info",
    "format": "json"
  }
}
>>>>>>> theirs
```

### After — semantic merge

```json
{
  "database": {
    "host": "db.prod.example.com",
    "port": 5432,
    "ssl": true,
    "max_connections": 100
  },
  "logging": {
    "level": "info",
    "format": "json"
  }
}
```

Both sets of changes are merged cleanly — no conflict markers, no manual resolution.

## Supported formats

| Format    | Extensions                     |
| --------- | ------------------------------ |
| JSON      | `.json`                        |
| YAML      | `.yaml`, `.yml`                |
| TOML      | `.toml`                        |
| CSV       | `.csv`                         |
| XML       | `.xml`                         |
| Markdown  | `.md`, `.markdown`             |
| DOCX      | `.docx`                        |
| XLSX      | `.xlsx`                        |
| PPTX      | `.pptx`                        |

## Development

When developing locally, the driver automatically uses the Rust binary at `../../target/release/suture` if it exists. Build it with:

```bash
cargo build --release
```

## Links

- [Main repository](https://github.com/WyattAu/suture)
- [Documentation](https://github.com/WyattAu/suture/tree/main/docs)
- [License (Apache 2.0)](https://github.com/WyattAu/suture/blob/main/LICENSE)
