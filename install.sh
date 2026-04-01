#!/usr/bin/env bash
set -euo pipefail

REPO="WyattAu/suture"
INSTALL_DIR="${PREFIX:-$HOME/.local/bin}"
GITHUB_URL="https://github.com/${REPO}/releases/latest/download"

info()  { echo "[INFO]  $*"; }
error() { echo "[ERROR] $*" >&2; exit 1; }

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"
    case "$OS" in
        Linux)  OS="linux"  ;;
        Darwin) OS="macos"  ;;
        *)      error "Unsupported OS: $OS" ;;
    esac
    case "$ARCH" in
        x86_64|amd64) ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *)             error "Unsupported architecture: $ARCH" ;;
    esac
    echo "${OS}-${ARCH}"
}

PLATFORM="$(detect_platform)"
ARCHIVE="suture-${PLATFORM}.tar.gz"
URL="${GITHUB_URL}/${ARCHIVE}"

info "Installing Suture for ${PLATFORM}"
info "Download URL: ${URL}"

mkdir -p "$INSTALL_DIR"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

info "Downloading ${ARCHIVE}..."
if command -v curl &>/dev/null; then
    curl -fSL "$URL" -o "${TMPDIR}/${ARCHIVE}"
elif command -v wget &>/dev/null; then
    wget -q "$URL" -O "${TMPDIR}/${ARCHIVE}"
else
    error "Neither curl nor wget found. Please install one."
fi

info "Extracting..."
tar -xzf "${TMPDIR}/${ARCHIVE}" -C "$INSTALL_DIR"

if [ -x "$INSTALL_DIR/suture" ]; then
    info "Successfully installed suture to ${INSTALL_DIR}/suture"
    "$INSTALL_DIR/suture" --version || true
else
    error "Installation failed: suture binary not found"
fi

if [ -x "$INSTALL_DIR/suture-hub" ]; then
    info "Successfully installed suture-hub to ${INSTALL_DIR}/suture-hub"
fi

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) info "NOTE: ${INSTALL_DIR} is not in your PATH. Add it with:"
       info "  export PATH=\"${INSTALL_DIR}:\$PATH\""
       ;;
esac
