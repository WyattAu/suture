# Suture Desktop

Cross-platform desktop application for [Suture](https://github.com/WyattAu/suture), a patch-based version control system.

Built with [Tauri v2](https://tauri.app/) for a lightweight, native experience.

## Features

- **Repository management**: Open, initialize, and browse Suture repositories
- **Branch operations**: Create, checkout, merge, and delete branches
- **Staging & committing**: Stage individual files or all changes, write commit messages
- **Commit history**: Browse the full commit log with author, date, and message details
- **Sync**: Push, pull, and sync with configured remotes
- **Stash**: Push and pop working changes to/from the stash
- **Tags**: List and create tags
- **Remotes**: Add and view remote repositories
- **Merge driver configuration**: Set up the Suture merge driver for Git interop
- **Auto-update check**: Check for new Suture releases from the tray or sidebar

## Prerequisites

### System Dependencies

**Debian/Ubuntu:**
```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

**Fedora:**
```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel \
  libappindicator-gtk3-devel librsvg2-devel
```

**Nix:**
```
webkitgtk_4_1, gtk3, libappindicator-gtk3, librsvg
```

### Rust Toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Building

```bash
# From the desktop-app directory
cargo tauri dev

# Production build
cargo tauri build
```

## Version

Current version: **5.4.0**

## License

Apache-2.0
