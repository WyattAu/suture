# suture-node

Node.js native bindings for [Suture VCS](https://github.com/WyattAu/suture) — a patch-based version control system with semantic merge and format-aware drivers.

## Installation

```bash
npm install suture-node
```

## Usage

### Repository Operations

```js
const suture = require('suture-node');

// Initialize a new repository
const repo = suture.init('/path/to/project', 'Author Name');
console.log(repo.head_branch); // "main"

// Open an existing repository
const info = suture.open('/path/to/project');

// Check status
const status = suture.status('/path/to/project');
console.log(status.head_branch);
console.log(status.staged_files);

// Stage files
suture.add('/path/to/project', 'README.md');
suture.addAll('/path/to/project');

// Commit
const result = suture.commit('/path/to/project', 'Initial commit');
console.log(result.id);
console.log(result.short_id);

// View log
const entries = suture.log('/path/to/project', 10);
entries.forEach(e => console.log(`${e.short_id} ${e.message}`));

// Branches
const branches = suture.branches('/path/to/project');
suture.createBranch('/path/to/project', 'feature-x');
```

### Semantic Merge

Merge structured files (JSON, YAML, TOML, CSV) without conflicts when changes touch different keys:

```js
// JSON semantic merge
const base = '{"a": 1, "b": 2}';
const ours = '{"a": 10, "b": 2}';
const theirs = '{"a": 1, "b": 20}';
const merged = suture.mergeJson(base, ours, theirs);
// => '{"a":10,"b":20}'

// YAML semantic merge
const mergedYaml = suture.mergeYaml(baseYaml, oursYaml, theirsYaml);

// TOML semantic merge
const mergedToml = suture.mergeToml(baseToml, oursToml, theirsToml);

// CSV semantic merge
const mergedCsv = suture.mergeCsv(baseCsv, oursCsv, theirsCsv);
```

When a merge cannot be auto-resolved (both sides changed the same key to different values), an error is thrown.

## TypeScript

Full TypeScript definitions are included:

```ts
import {
  init, open, status, add, addAll, commit, log, branches, createBranch,
  mergeJson, mergeYaml, mergeToml, mergeCsv, getVersion,
  RepoInfo, StatusResult, CommitResult, LogEntry, BranchEntry
} from 'suture-node';
```

## Build from Source

Requires Rust and the napi-rs CLI:

```bash
cd packages/suture-node
npm install
npm run build
```

## License

Apache-2.0
