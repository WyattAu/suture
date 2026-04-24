# Building the Suture Desktop App

This guide covers building the Suture desktop application from source. The desktop app uses [Tauri](https://tauri.app/) to provide a native UI that calls `suture-core` directly — no CLI shelling.

## Prerequisites

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libssl-dev pkg-config
```

### macOS

```bash
xcode-select --install
```

Ensure Rust is installed (see [rustup.rs](https://rustup.rs/)).

### Windows

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload.
2. Install [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (pre-installed on Windows 11).

## Build from Source

```bash
cd desktop-app
cargo build --features tauri --release
```

The binary will be in `target/release/`.

## Development Mode

For iterative development with faster builds and auto-reload:

```bash
cd desktop-app
cargo build --features tauri
cargo run --features tauri
```

## Distribution

To produce a platform-specific installer:

```bash
cd desktop-app
cargo tauri build
```

Output formats:

| Platform | Format |
|----------|--------|
| Linux    | `.deb`, `.AppImage` |
| macOS    | `.dmg` |
| Windows  | `.msi` |

Installers are written to `target/release/bundle/`.

## How the UI Works

- The Tauri backend calls `suture-core` directly via Rust FFI — it does not shell out to the CLI.
- The frontend is standard HTML/CSS/JS at `ui/index.html`.
- All communication between the frontend and backend uses `window.__TAURI__.invoke()`.

## Architecture

```
┌─────────────────────────────────┐
│         Tauri WebView           │
│  ┌─────────────────────────┐   │
│  │    HTML/CSS/JS UI        │   │
│  └───────────┬─────────────┘   │
│              │ invoke()         │
│  ┌───────────▼─────────────┐   │
│  │  Rust Tauri Commands     │   │
│  └───────────┬─────────────┘   │
│              │                  │
│  ┌───────────▼─────────────┐   │
│  │    suture-core API       │   │
│  └───────────┬─────────────┘   │
│              │                  │
│  ┌───────────▼─────────────┐   │
│  │  .suture/ (SQLite + CAS)│   │
│  └─────────────────────────┘   │
└─────────────────────────────────┘
```
