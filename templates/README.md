# Suture `.gitattributes` Templates

Pre-built `.gitattributes` snippets for common project types. Append the relevant template to your project's `.gitattributes` file to enable semantic merging with Suture.

## Quick Start

```bash
# Download and append a template
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/templates/gitattributes-node.md >> .gitattributes
```

Or copy-paste the relevant lines from any template below.

## Templates

| Template | For | Files Covered |
|----------|-----|---------------|
| [Node.js](gitattributes-node.md) | JavaScript/TypeScript | package.json, tsconfig, eslint, prettier, jest |
| [Rust](gitattributes-rust.md) | Rust/Cargo | Cargo.toml, clippy, rustfmt |
| [Python](gitattributes-python.md) | Python/Pip/Poetry | pyproject.toml, setup.cfg, tox.ini |
| [Kubernetes](gitattributes-kubernetes.md) | K8s/Helm | YAML manifests, values files, Chart.yaml |
| [Java](gitattributes-java.md) | Maven/Gradle | pom.xml, application.yml, logback.xml |
| [CI/CD](gitattributes-ci.md) | GitHub/GitLab/CircleCI | workflow YAML files |
| [Web](gitattributes-web.md) | Frontend | package.json, tailwind, postcss, SVG |
| [Go](gitattributes-go.md) | Go modules | go.mod, golangci |
| [Combo](gitattributes-combo.md) | Everything | All supported formats by extension |

## Notes

- Use `-merge` for files that should **not** use semantic merge (binary, generated, or lock files).
- Wildcard patterns (e.g. `*.json merge=json`) apply to all matching files recursively.
- Combine multiple templates by appending each to your `.gitattributes`.
