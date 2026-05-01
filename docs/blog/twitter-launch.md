🧵 Suture 5.1.0 is live — semantic merge for every format.

Git merges structured files line-by-line. Two people editing different keys in the same JSON? Conflict.

Suture parses files into their native representations and merges at the structural level.

17 formats: JSON, YAML, TOML, XML, CSV, SQL, HTML, Markdown, SVG, DOCX, XLSX, PPTX, PDF, and more.

One command to install as a git merge driver:
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash

Try it in your browser (no signup, fully client-side):
https://wyattau.github.io/suture/#/merge

Built in Rust. 39 crates. 1,148 tests. Open source (AGPL-3.0).

github.com/WyattAu/suture

---

What's included in 5.1.0:

✅ Git merge driver (automatic .gitattributes)
✅ Interactive 3-way merge demo
✅ CLI: init, add, commit, branch, merge, log, diff, push, pull
✅ TUI: dashboard, patch browser, merge view
✅ LSP: diagnostics, completions, hover, symbols
✅ GitHub Action for CI/CD
✅ FUSE filesystem mount
✅ WASM plugin system (wasmtime v28)
✅ Raft consensus (production-hardened)
✅ Hosted platform with Stripe billing
✅ VS Code extension
✅ npm, PyPI, Homebrew packages

Install: cargo install suture-cli
Docs: https://wyattau.github.io/suture/
