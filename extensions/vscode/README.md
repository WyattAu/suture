# Suture — Semantic Version Control

Version control that understands your files. Git merge driver for JSON, YAML, DOCX, XLSX, and 20+ formats.

## Features

- **Git Merge Driver Configuration** — One-click setup to use Suture as your git merge driver for structured files
- **Per-Format Enablement** — Enable semantic merge for specific formats (JSON, YAML, or all supported formats)
- **Repository Commands** — Initialize, status, commit, history, and diff directly from VS Code
- **Output Channel** — All Suture output is routed to a dedicated "Suture" output channel

## Usage

### Configure Git Merge Driver

Open the Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`) and run:

- **Suture: Configure Git Merge Driver** — Registers Suture as a git merge driver globally
- **Suture: Enable Semantic Merge for JSON** — Configures merge driver and adds `*.json` to `.gitattributes`
- **Suture: Enable Semantic Merge for YAML** — Configures merge driver and adds `*.yaml` / `*.yml` to `.gitattributes`
- **Suture: Enable Semantic Merge for All Formats** — Configures merge driver for all supported formats

### Repository Commands

- **Suture: Initialize Repository** — Run `suture init` in the workspace root
- **Suture: Show Status** — Run `suture status`
- **Suture: Quick Commit** — Stage all changes and commit with a message
- **Suture: Show History** — Show the last 20 commits
- **Suture: Show Diff** — Show unstaged changes

## Supported Formats

| Format | Extensions |
|--------|-----------|
| JSON | `.json` |
| YAML | `.yaml`, `.yml` |
| TOML | `.toml` |
| CSV | `.csv` |
| XML | `.xml` |
| Markdown | `.md` |
| HTML | `.html` |
| SVG | `.svg` |
| Word | `.docx` |
| Excel | `.xlsx` |
| PowerPoint | `.pptx` |
| SQL | `.sql` |

## Installation

1. Install the [Suture CLI](https://github.com/WyattAu/suture)
2. Install this extension from the VS Code Marketplace, or build from source:
   ```bash
   cd extensions/vscode
   npm install
   npm run compile
   ```

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `suture.executablePath` | `suture` | Path to the suture executable |
| `suture.autoConfigure` | `false` | Automatically configure suture as git merge driver when a suture repo is detected |

## Links

- [Suture Repository](https://github.com/WyattAu/suture)
- [Issues](https://github.com/WyattAu/suture/issues)
