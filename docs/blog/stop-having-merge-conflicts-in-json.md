# Stop Having Merge Conflicts in package.json

You know the feeling. You're wrapping up a feature branch, you run `git merge main`, and:

```
CONFLICT (content): Merge conflict in package.json
Automatic merge failed; fix conflicts and then commit the result.
```

You open the file and find this:

```json
{
<<<<<<< HEAD
  "name": "my-app",
  "version": "2.1.0",
  "dependencies": {
    "react": "^18.2.0",
    "express": "^4.18.2",
    "zod": "^3.22.4"
  }
=======
  "name": "my-app",
  "version": "2.1.0",
  "dependencies": {
    "react": "^18.2.0",
    "express": "^4.19.0",
    "pg": "^8.11.3"
  }
>>>>>>> main
}
```

You added `zod`. Your coworker upgraded `express` and added `pg`. These changes have nothing to do with each other. There is no logical conflict here. But Git doesn't care about logic -- it cares about lines.

## Why Git Gets This Wrong

Git's merge algorithm is line-based. It computes a longest common subsequence between the base, ours, and theirs, then checks whether the changed line ranges overlap. If two people edit lines that are close together -- even if they're modifying completely different JSON keys -- Git flags a conflict.

This isn't a bug in Git. Git was designed for source code, where a changed line in the middle of a function genuinely might conflict with another changed line two lines below. The algorithm works well for `.rs`, `.py`, `.js`, and `.c` files.

The problem is that structured data formats like JSON, YAML, TOML, and XML have a different notion of identity. In a JSON file, the meaningful unit is a key-value pair, not a line. A file with the same keys rearranged is semantically identical. A file with two different keys changed on adjacent lines has no conflict at all.

Git can't do better because it has no parser. It doesn't know what JSON is. It doesn't understand that `{"a": 1, "b": 2}` and `{\n  "b": 2,\n  "a": 1\n}` are the same object. It just sees different lines.

## Semantic Merge

The fix is to merge at the structural level instead of the line level. A semantic merge driver parses the file format, computes diffs at the field level, and serializes the result back. If two people change different keys, the driver applies both changes. If they change the same key, it reports a real conflict.

The three-way merge works like this: the driver receives the base version (common ancestor), ours, and theirs. It parses all three into an intermediate representation, computes what changed on each side relative to the base, checks whether the changes overlap, and writes the merged output. Non-overlapping changes apply cleanly. Overlapping changes produce a conflict only when both sides modified the same logical element.

## Before and After

Here's the same merge scenario -- two developers editing different keys in a Kubernetes deployment config -- with Git and with Suture.

**Git (line-based):**

```yaml
<<<<<<< HEAD
apiVersion: apps/v1
kind: Deployment
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: app
        image: myapp:2.1.0
        resources:
          limits:
            cpu: "500m"
=======
apiVersion: apps/v1
kind: Deployment
spec:
  replicas: 5
  template:
    spec:
      containers:
      - name: app
        image: myapp:2.1.0
        env:
        - name: LOG_LEVEL
          value: "debug"
>>>>>>> main
```

You changed `replicas` and added a CPU limit. Your coworker changed `replicas` too and added an environment variable. Git can't tell which changes are independent.

**Suture (semantic):**

```yaml
apiVersion: apps/v1
kind: Deployment
spec:
  replicas: 5
  template:
    spec:
      containers:
      - name: app
        image: myapp:2.1.0
        env:
        - name: LOG_LEVEL
          value: "debug"
        resources:
          limits:
            cpu: "500m"
```

Suture merges at the key level. `replicas` is a genuine conflict (both sides changed it), but `resources` and `env` are independent -- both apply cleanly. For the conflicting `replicas` field, Suture falls back to standard conflict markers on just that key, leaving the rest of the file intact and valid.

## Installation

Install the merge driver with your preferred package manager:

```bash
npm install -g suture-merge-driver
```

```bash
pip install suture-merge-driver
```

```bash
cargo install suture-merge
```

All three commands install the same underlying merge engine (written in Rust). The npm and pip packages download a prebuilt platform binary automatically.

## Configuration

Once installed, tell Git to use Suture for structured files. This takes four lines:

```bash
git config merge.suture.name "Suture semantic merge"
git config merge.suture.driver "suture-merge-driver %O %A %B %P"
echo "*.json merge=suture" >> .gitattributes
echo "*.yaml merge=suture" >> .gitattributes
```

The first two lines register the merge driver. The `.gitattributes` lines tell Git which file types to route through it. Add more patterns for other formats:

```bash
echo "*.toml merge=suture" >> .gitattributes
echo "*.csv merge=suture" >> .gitattributes
echo "*.xml merge=suture" >> .gitattributes
echo "*.md merge=suture" >> .gitattributes
echo "*.docx merge=suture" >> .gitattributes
echo "*.xlsx merge=suture" >> .gitattributes
echo "*.pptx merge=suture" >> .gitattributes
```

Commit the `.gitattributes` file and you're done. Every subsequent `git merge` on supported formats will use semantic merge automatically. Files not listed in `.gitattributes` continue to use Git's default line-based merge.

For a one-command setup, run `suture git driver install` if you have the full CLI installed. It writes the config and `.gitattributes` for all 20+ supported formats at once.

## Supported Formats

| Format    | Extensions                                  | Merge Granularity              |
|-----------|---------------------------------------------|--------------------------------|
| JSON      | `.json`, `.jsonl`                           | Field-level (RFC 6901 paths)   |
| YAML      | `.yaml`, `.yml`                             | Key-level                      |
| TOML      | `.toml`                                     | Table and key-aware            |
| CSV       | `.csv`, `.tsv`                              | Row-level with header detection|
| XML       | `.xml`, `.xsl`, `.svg`                      | Element/attribute-aware        |
| Markdown  | `.md`, `.markdown`                          | Section-aware                  |
| HTML      | `.html`                                     | DOM-aware                      |
| DOCX      | `.docx`, `.docm`                            | Paragraph-level                |
| XLSX      | `.xlsx`, `.xlsm`                            | Cell-level                     |
| PPTX      | `.pptx`, `.pptm`                            | Slide-level                    |
| OTIO      | `.otio`                                     | Clip-level (video timelines)   |
| SQL       | `.sql`                                      | DDL schema diff                |
| PDF       | `.pdf`                                      | Page-level text diff           |
| Image     | `.png`, `.jpg`, `.gif`, `.bmp`, `.webp`, `.tiff`, `.ico`, `.avif` | Metadata diff |
| iCalendar | `.ics`                                      | Event-level merge              |
| RSS/Atom  | `.rss`, `.atom`                             | Feed and entry-aware           |

Files without a matching driver fall back to Git's standard line-based merge. No behavior changes for unsupported formats.

## How It Works Under the Hood

Suture uses a three-way merge at the semantic level. When Git invokes the driver, it passes three file paths: the base (common ancestor), ours (your version), and theirs (their version). Suture detects the file format from the extension, parses all three versions into an intermediate representation, and computes the diff between base and each side.

Each change is mapped to a logical address -- for JSON, that's an RFC 6901 JSON Pointer like `/spec/replicas`; for YAML, a dot-separated key path; for DOCX, a paragraph index. Two changes conflict only when their logical addresses overlap. If Alice changed `/spec/replicas` and Bob changed `/spec/template/spec/containers/0/env`, the touch sets are disjoint, so both patches apply cleanly and the merge is deterministic.

The patch algebra is formalized and proved correct in Lean 4: if the touch sets of two patches are disjoint, their composition commutes. The merge result is the same regardless of which side is applied first. This isn't a heuristic -- it's a structural guarantee.

## Try It

If you work with JSON configs, Kubernetes manifests, CI pipelines, or any structured data in Git, semantic merge eliminates a class of merge conflicts that shouldn't exist in the first place.

Install it, add the four config lines, and merge your next PR. If you hit an edge case or a format that isn't supported yet, open an issue on GitHub. The project is actively developed, and the driver list is growing.

- GitHub: https://github.com/WyattAu/suture
- Documentation: https://github.com/WyattAu/suture/tree/main/docs
- License: Apache 2.0
