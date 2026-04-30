#!/usr/bin/env bash
set -euo pipefail

# Suture Git Merge Driver — One-line installer
# Usage: curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash
# Or:  brew install suture-merge-driver && suture init-merge-driver

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail()  { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }

SUTURE_VERSION="${SUTURE_VERSION:-latest}"

detect_os() {
    local os
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    case "$os" in
        linux)  echo "linux" ;;
        darwin) echo "macos" ;;
        mingw*|msys*|cygwin*) echo "windows" ;;
        *)      echo "$os" ;;
    esac
}

detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64) echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *) echo "$arch" ;;
    esac
}

install_suture() {
    if command -v suture &>/dev/null; then
        ok "Suture found: $(suture --version 2>/dev/null || echo 'installed')"
        return 0
    fi

    info "Installing Suture..."

    if command -v cargo &>/dev/null; then
        info "Installing via cargo..."
        cargo install suture-cli --locked 2>/dev/null || cargo install suture-merge-driver --locked 2>/dev/null || {
            warn "Cargo install failed. Trying fallback methods..."
            return 1
        }
        ok "Installed via cargo"
        return 0
    fi

    if command -v brew &>/dev/null; then
        info "Installing via Homebrew..."
        brew tap WyattAu/suture-merge-driver 2>/dev/null || true
        brew install suture-merge-driver 2>/dev/null || {
            warn "Homebrew install failed. Trying fallback methods..."
            return 1
        }
        ok "Installed via Homebrew"
        return 0
    fi

    if command -v npm &>/dev/null; then
        info "Installing via npm..."
        npm install -g suture-merge-driver 2>/dev/null || {
            warn "npm install failed. Trying fallback methods..."
            return 1
        }
        ok "Installed via npm"
        return 0
    fi

    if command -v pip3 &>/dev/null || command -v pip &>/dev/null; then
        info "Installing via pip..."
        (pip3 install suture-merge-driver 2>/dev/null || pip install suture-merge-driver 2>/dev/null) || {
            warn "pip install failed. Trying fallback methods..."
            return 1
        }
        ok "Installed via pip"
        return 0
    fi

    info "Downloading binary from GitHub Releases..."
    local os arch url
    os="$(detect_os)"
    arch="$(detect_arch)"
    url="https://github.com/WyattAu/suture/releases/${SUTURE_VERSION}/download/suture-${os}-${arch}"

    local bin_dir="${SUTURE_BIN_DIR:-/usr/local/bin}"
    if [ ! -d "$bin_dir" ]; then
        bin_dir="$HOME/.local/bin"
        mkdir -p "$bin_dir"
    fi

    if curl -fsSL "$url" -o "${bin_dir}/suture" 2>/dev/null; then
        chmod +x "${bin_dir}/suture"
        ok "Downloaded to ${bin_dir}/suture"
        export PATH="${bin_dir}:${PATH}"
        return 0
    fi

    if curl -fsSL "${url}.tar.gz" | tar xz -C "$bin_dir" 2>/dev/null; then
        ok "Downloaded and extracted to ${bin_dir}/"
        export PATH="${bin_dir}:${PATH}"
        return 0
    fi

    if curl -fsSL "${url}.zip" -o /tmp/suture.zip 2>/dev/null && unzip -qo /tmp/suture.zip -d "$bin_dir" 2>/dev/null; then
        rm -f /tmp/suture.zip
        ok "Downloaded and extracted to ${bin_dir}/"
        export PATH="${bin_dir}:${PATH}"
        return 0
    fi

    return 1
}

configure_drivers() {
    info "Configuring Git merge drivers..."

    local scope="${1:-global}"
    local scope_flag="--${scope}"

    if [ "$scope" = "global" ]; then
        echo ""
        echo -e "${BLUE}Configuring for all repositories (global).${NC}"
        echo -e "${BLUE}Use --local flag for per-repo configuration.${NC}"
        echo ""
    fi

    git config $scope_flag merge.json.name "Suture JSON merge driver"
    git config $scope_flag merge.json.driver "suture merge-file --driver json %O %A %B -o %A"
    ok "  JSON driver configured"

    git config $scope_flag merge.yaml.name "Suture YAML merge driver"
    git config $scope_flag merge.yaml.driver "suture merge-file --driver yaml %O %A %B -o %A"
    ok "  YAML driver configured"

    git config $scope_flag merge.toml.name "Suture TOML merge driver"
    git config $scope_flag merge.toml.driver "suture merge-file --driver toml %O %A %B -o %A"
    ok "  TOML driver configured"

    git config $scope_flag merge.xml.name "Suture XML merge driver"
    git config $scope_flag merge.xml.driver "suture merge-file --driver xml %O %A %B -o %A"
    ok "  XML driver configured"

    git config $scope_flag merge.csv.name "Suture CSV merge driver"
    git config $scope_flag merge.csv.driver "suture merge-file --driver csv %O %A %B -o %A"
    ok "  CSV driver configured"

    git config $scope_flag merge.md.name "Suture Markdown merge driver"
    git config $scope_flag merge.md.driver "suture merge-file --driver markdown %O %A %B -o %A"
    ok "  Markdown driver configured"

    git config $scope_flag merge.docx.name "Suture DOCX merge driver"
    git config $scope_flag merge.docx.driver "suture merge-file --driver docx %O %A %B -o %A"
    git config $scope_flag merge.docx.recursive "binary"
    ok "  DOCX driver configured (binary merge enabled)"

    git config $scope_flag merge.xlsx.name "Suture XLSX merge driver"
    git config $scope_flag merge.xlsx.driver "suture merge-file --driver xlsx %O %A %B -o %A"
    git config $scope_flag merge.xlsx.recursive "binary"
    ok "  XLSX driver configured (binary merge enabled)"

    git config $scope_flag merge.pptx.name "Suture PPTX merge driver"
    git config $scope_flag merge.pptx.driver "suture merge-file --driver pptx %O %A %B -o %A"
    git config $scope_flag merge.pptx.recursive "binary"
    ok "  PPTX driver configured (binary merge enabled)"
}

create_gitattributes() {
    local target="${1:-.gitattributes}"
    local patterns='# Suture semantic merge drivers
*.json merge=json
*.jsonl merge=json
*.yaml merge=yaml
*.yml merge=yaml
*.toml merge=toml
*.xml merge=xml
*.xsl merge=xml
*.svg merge=xml
*.csv merge=csv
*.tsv merge=csv
*.md merge=md
*.markdown merge=md
*.docx merge=docx
*.docm merge=docx
*.xlsx merge=xlsx
*.xlsm merge=xlsx
*.pptx merge=pptx
*.pptm merge=pptx'

    if [ -f "$target" ]; then
        if grep -q "merge=json" "$target" 2>/dev/null; then
            ok "$target already has Suture entries"
        else
            echo "" >> "$target"
            echo "$patterns" >> "$target"
            ok "$target updated"
        fi
    else
        echo "$patterns" > "$target"
        ok "$target created"
    fi
}

test_configuration() {
    info "Testing configuration..."

    local errors=0

    if ! command -v suture &>/dev/null; then
        warn "  suture not found on PATH — driver commands will fail at merge time"
        errors=$((errors + 1))
    else
        ok "  sutable binary: $(which suture)"
    fi

    local driver_val
    driver_val="$(git config --get merge.json.driver 2>/dev/null || true)"
    if [ -n "$driver_val" ]; then
        ok "  JSON driver: $driver_val"
    else
        warn "  JSON driver not configured"
        errors=$((errors + 1))
    fi

    if [ -f ".gitattributes" ]; then
        local pattern_count
        pattern_count="$(grep -c "merge=" .gitattributes 2>/dev/null || echo 0)"
        ok "  .gitattributes: $pattern_count patterns"
    else
        if [ -f "$HOME/.gitattributes" ]; then
            local pattern_count
            pattern_count="$(grep -c "merge=" "$HOME/.gitattributes" 2>/dev/null || echo 0)"
            ok "  ~/.gitattributes: $pattern_count patterns"
        else
            warn "  No .gitattributes found"
            errors=$((errors + 1))
        fi
    fi

    return $errors
}

usage() {
    cat <<'EOF'
Suture Git Merge Driver Installer

Usage:
  ./install-merge-driver.sh [options]

Options:
  --local          Configure for the current repository only (default: global)
  --uninstall      Remove all Suture merge driver configuration
  --test           Only test the current configuration
  --help           Show this help message

Environment variables:
  SUTURE_VERSION   GitHub release tag (default: latest)
  SUTURE_BIN_DIR   Directory to install binary (default: /usr/local/bin)

Examples:
  curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash
  ./install-merge-driver.sh --local
  ./install-merge-driver.sh --uninstall
EOF
}

uninstall() {
    info "Removing Suture merge driver configuration..."

    for driver in json yaml toml xml csv md docx xlsx pptx; do
        git config --global --unset "merge.${driver}.name" 2>/dev/null || true
        git config --global --unset "merge.${driver}.driver" 2>/dev/null || true
        git config --global --unset "merge.${driver}.recursive" 2>/dev/null || true
        git config --unset "merge.${driver}.name" 2>/dev/null || true
        git config --unset "merge.${driver}.driver" 2>/dev/null || true
        git config --unset "merge.${driver}.recursive" 2>/dev/null || true
    done

    git config --global --remove-section merge.suture 2>/dev/null || true
    git config --remove-section merge.suture 2>/dev/null || true

    ok "Git config cleaned"

    if [ -f ".gitattributes" ]; then
        local tmp
        tmp="$(mktemp)"
        grep -v "merge=json\|merge=yaml\|merge=toml\|merge=xml\|merge=csv\|merge=md\|merge=docx\|merge=xlsx\|merge=pptx" .gitattributes > "$tmp" 2>/dev/null || true
        if [ -s "$tmp" ]; then
            mv "$tmp" .gitattributes
        else
            rm -f "$tmp" .gitattributes
        fi
        ok ".gitattributes cleaned"
    fi

    echo ""
    ok "Suture merge driver uninstalled"
}

# --- Main ---

SCOPE="global"

case "${1:-}" in
    --local)
        SCOPE="local"
        shift
        ;;
    --uninstall)
        uninstall
        exit 0
        ;;
    --test)
        test_configuration
        exit $?
        ;;
    --help|-h)
        usage
        exit 0
        ;;
esac

echo ""
echo -e "${BLUE}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     Suture Git Merge Driver Installer       ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════╝${NC}"
echo ""

install_suture || {
    echo ""
    fail "Could not auto-install Suture. Please install manually:
  cargo install suture-cli
  brew install suture-merge-driver
  npm install -g suture-merge-driver
  pip install suture-merge-driver
  or: https://github.com/WyattAu/suture#installation"
}

configure_drivers "$SCOPE"

if [ "$SCOPE" = "local" ]; then
    create_gitattributes ".gitattributes"
else
    create_gitattributes "$HOME/.gitattributes"
    if [ -d .git ] 2>/dev/null; then
        create_gitattributes ".gitattributes"
    fi
fi

echo ""
test_configuration || true

echo ""
ok "Suture merge driver installed!"
echo ""
echo "  Git will now use Suture to automatically merge:"
echo "    JSON  YAML  TOML  XML  CSV  Markdown"
echo "    DOCX  XLSX  PPTX (binary merge enabled)"
echo ""
echo "  If inside a Git repo, commit the generated .gitattributes:"
echo "    git add .gitattributes"
echo "    git commit -m \"Configure suture merge driver\""
echo ""
echo "  Test it:"
echo "    git merge feature-branch  # structured files merge automatically"
echo ""
echo "  Uninstall:"
echo "    ./install-merge-driver.sh --uninstall"
echo ""
