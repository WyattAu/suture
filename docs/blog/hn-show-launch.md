# Show HN: Suture – Semantic merge for JSON, YAML, TOML, XML, CSV (17 formats)

Git merges structured files line-by-line, so two people editing different keys in the same JSON object get a conflict. Suture parses files into their native representations (trees, maps, arrays), merges at the structural level, and only flags true conflicts.

**Install as a git merge driver (one command):**

```
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash
```

After that, git automatically uses Suture for JSON, YAML, TOML, XML, and CSV files. No config editing.

**Try it in your browser:** https://wyattau.github.io/suture/#/merge (fully client-side, no signup)

**17 formats supported:** JSON, YAML, TOML, XML, CSV, SQL, HTML, Markdown, SVG, Properties/INI, DOCX, XLSX, PPTX, PDF metadata, RSS/Atom, iCalendar, OpenTimelineIO

**Other install methods:**
- Cargo: `cargo install suture-cli`
- Homebrew: `brew install WyattAu/suture-merge-driver/suture-merge-driver`
- npm: `npm install -g suture-merge-driver`
- PyPI: `pip install suture-merge-driver`
- Binary releases: https://github.com/WyattAu/suture/releases

**What's included:**
- Git merge driver with automatic `.gitattributes` configuration
- Interactive 3-way merge demo (client-side, no backend)
- CLI with init, add, commit, branch, merge, log, diff, push, pull, remote, status
- TUI dashboard, patch browser, and merge view (ctrl-c to cancel, scroll to browse)
- LSP server (diagnostics, completions, hover, go-to-symbol) for VS Code
- GitHub Action for CI/CD: `uses: WyattAu/suture/.github/actions/merge@main`
- FUSE filesystem mount (`suture mount ./repo /mnt/suture`)
- WASM plugin system (wasmtime v28, fuel-based timeouts, 16MB memory limit)
- Raft consensus for distributed hub deployment
- Hosted platform with Stripe billing (free/$9/$29 tiers)
- 39 Rust crates, 1,148 tests, 0 clippy warnings

**Performance:** JSON 10-key merge in ~10µs, 100-key in ~40µs, 1K-key in ~400µs

Source: https://github.com/WyattAu/suture
Docs: https://wyattau.github.io/suture/

Built in Rust. Dual-licensed AGPL-3.0 + Commercial.
