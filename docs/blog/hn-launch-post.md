Show HN: Suture – Semantic merge for 18 file formats (JSON, YAML, XML, CSV, Word, Excel, PDF...)

I built Suture because I was tired of manually resolving merge conflicts in
YAML configs, JSON manifests, and XML layouts at work. Git's line-based merge
doesn't understand file structure, so it creates false conflicts when two
people edit different keys in the same JSON file.

Suture does structural 3-way merge. It parses files into their AST,
identifies what actually changed, and merges intelligently:

- JSON/YAML/TOML: merge different keys/sections automatically
- XML: merge different elements and attributes  
- CSV: merge different rows and columns
- Word/Excel/PDF: binary-aware merge
- SQL: merge DDL statements
- HTML/Markdown: merge different sections
- SVG: merge different elements
- Images: pixel-level merge
- And 8 more formats...

It works as:
1. Git merge driver (`git merge-file`) — drop-in replacement
2. CLI tool (`suture merge`)
3. REST API (merge-as-a-service)
4. Web UI (try it live)
5. VS Code extension
6. Rust library

The core merge engine is ~131K lines of Rust, has 1,550+ tests, and is verified
via extensive property-based testing using patch theory. The self-hosted hub is
always free (AGPL-3.0). We also have a hosted platform with a free tier
(100 merges/month).

Tech: Rust, Axum, SQLite, Wasmtime, Raft consensus, 32 crates on crates.io

GitHub: https://github.com/WyattAu/suture
Try the merge tool: https://suture.dev
Docs: https://wyattau.github.io/suture/
