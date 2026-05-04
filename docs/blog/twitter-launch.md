Suture 5.1.0 is live — semantic merge for every format.

Git merges structured files line-by-line. Two people editing different keys in the same JSON? Conflict.

Suture parses files into their native representations and merges at the structural level.

18 formats: JSON, YAML, TOML, XML, CSV, SQL, HTML, Markdown, SVG, DOCX, XLSX, PPTX, PDF, PLIST, and more.

One line to install:
curl -sSL https://suture.dev/install.sh | sh

Try it in your browser (no signup, fully client-side):
https://wyattau.github.io/suture/#/merge

What's in 5.1.0:

- Git merge driver (automatic .gitattributes)
- Interactive 3-way merge demo
- CLI: init, add, commit, branch, merge, log, diff, push, pull
- TUI: dashboard, patch browser, merge view
- VS Code extension with real-time merge diagnostics
- GitHub Action for CI/CD
- FUSE filesystem mount
- WASM plugin system (wasmtime v28)
- Raft consensus (production-hardened)
- Full SaaS platform: auth, Stripe billing, merge API
- Deployed to Fly.io

Pricing:
- Free: unlimited public repos
- Pro: $9/seat/month
- Enterprise: $29/seat/month

Built in Rust. 39 crates. 1,594 tests. 0 failures. 0 known issues. Open source (AGPL-3.0). Self-hostable.

github.com/WyattAu/suture
suture.dev
