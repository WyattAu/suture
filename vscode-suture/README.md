# Suture Semantic Merge — VS Code Extension

Semantic merge for structured files — JSON, YAML, TOML, XML, and more.

## Requirements

- [suture](https://github.com/WyattAu/suture) CLI must be installed and available in your `PATH`
- VS Code 1.85.0 or later

## Installation

### From VSIX

```bash
cd vscode-suture
npm install
npm run compile
npx vsce package
code --install-extension suture-0.1.0.vsix
```

### From Source

```bash
cd vscode-suture
npm install
npm run compile
```

Then launch VS Code with the extension loaded:

```bash
code --extensionDevelopmentPath $PWD
```

## Commands

| Command | Description |
|---------|-------------|
| `Suture: Semantic Merge Current File` | Run semantic merge on the active file |
| `Suture: Initialize Repository` | Initialize suture in the current workspace |
| `Suture: Show Status` | Show suture status for the workspace |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `suture.path` | `suture` | Path to the suture CLI binary |
