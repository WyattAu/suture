Title: r/programming: I built a semantic merge engine that understands file structure — works with JSON, YAML, XML, Word, Excel, and 13 more formats

Body:

After spending years manually resolving merge conflicts in Kubernetes YAML, package.json, and XML configs, I built a tool that does structural 3-way merge — it parses files into their AST, identifies what actually changed, and only conflicts when two people edit the same key/element.

The core idea: Git's line-based merge treats files as bags of lines. If two people add a key to the same JSON object, Git sees a conflict even though the changes are unrelated. Suture parses the JSON, sees that different keys were modified, and merges them automatically.

**What formats it supports:**

- Structured: JSON, YAML, TOML, XML, CSV, SQL, HTML, Markdown, Properties, INI, ENV
- Binary: PNG, JPEG, WebP, GIF, SVG, Word (DOCX), Excel (XLSX), PowerPoint (PPTX), PDF
- Domain: iCalendar, RSS/Atom feeds, OpenTimelineIO

**How to use it:**

1. As a Git merge driver (drop-in):
   ```
   git config merge.driver.suture.name "Suture semantic merge"
   git config merge.driver.suture.driver "suture merge-file %O %A %B %P"
   ```
   Then add to `.gitattributes`:
   ```
   *.json merge=suture
   *.yaml merge=suture
   *.toml merge=suture
   ```

2. As a REST API:
   ```bash
   curl -X POST https://suture.dev/api/merge \
     -d '{"driver":"json","base":"...","ours":"...","theirs":"..."}'
   ```

3. As a Rust library:
   ```rust
   let driver = JsonDriver::default();
   let merged = driver.merge(&base, &ours, &theirs)?;
   ```

**Tech details:**
- Written in Rust (32 crates, 1,300+ tests)
- Proven by Lean 4 formal verification using patch theory
- Uses BLAKE3 for content hashing
- Raft consensus for distributed hub
- Wasm plugin system for custom merge strategies
- Self-hosted hub is free (AGPL-3.0)

GitHub: https://github.com/WyattAu/suture
Docs: https://wyattau.github.io/suture/

Happy to answer questions about the merge algorithm, the patch theory formalization, or the architecture.
