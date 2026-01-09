#!/bin/bash
#
# AGIT Install Script
# https://github.com/agit-stuff/agit
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/agit-stuff/agit/main/install.sh | bash
#
# Options (via environment variables):
#   AGIT_VERSION   - Specific version to install (default: latest)
#   AGIT_INSTALL   - Installation directory (default: ~/.local/bin)
#

set -euo pipefail

# Configuration
REPO="agit-stuff/agit"
INSTALL_DIR="${AGIT_INSTALL:-$HOME/.local/bin}"
VERSION="${AGIT_VERSION:-}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    printf "${BLUE}==>${NC} %s\n" "$1"
}

success() {
    printf "${GREEN}==>${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}Warning:${NC} %s\n" "$1"
}

error() {
    printf "${RED}Error:${NC} %s\n" "$1" >&2
    exit 1
}

# Detect OS
detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        *)       error "Unsupported OS: $os. Use cargo install agit instead." ;;
    esac
}

# Detect architecture
detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)  echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *)             error "Unsupported architecture: $arch. Use cargo install agit instead." ;;
    esac
}

# Check for required commands
check_requirements() {
    local missing=()

    for cmd in curl tar; do
        if ! command -v "$cmd" &> /dev/null; then
            missing+=("$cmd")
        fi
    done

    # Need either sha256sum or shasum
    if ! command -v sha256sum &> /dev/null && ! command -v shasum &> /dev/null; then
        missing+=("sha256sum or shasum")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        error "Missing required commands: ${missing[*]}"
    fi
}

# Calculate SHA256 checksum (works on both Linux and macOS)
sha256() {
    if command -v sha256sum &> /dev/null; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

# Get latest version from GitHub
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"v([^"]+)".*/\1/'
}

# Download and install
install_agit() {
    local os="$1"
    local arch="$2"
    local version="$3"

    local asset_name="agit-${os}-${arch}.tar.gz"
    local download_url="https://github.com/${REPO}/releases/download/v${version}/${asset_name}"
    local checksum_url="${download_url}.sha256"

    local tmp_dir
    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT

    info "Downloading agit v${version} for ${os}/${arch}..."

    # Download binary
    if ! curl -fsSL -o "${tmp_dir}/${asset_name}" "$download_url"; then
        error "Failed to download ${download_url}"
    fi

    # Download checksum
    if ! curl -fsSL -o "${tmp_dir}/${asset_name}.sha256" "$checksum_url"; then
        error "Failed to download checksum"
    fi

    # Verify checksum
    info "Verifying checksum..."
    local expected_checksum actual_checksum
    expected_checksum="$(cut -d' ' -f1 < "${tmp_dir}/${asset_name}.sha256")"
    actual_checksum="$(sha256 "${tmp_dir}/${asset_name}")"

    if [ "$expected_checksum" != "$actual_checksum" ]; then
        error "Checksum verification failed!\nExpected: ${expected_checksum}\nActual:   ${actual_checksum}"
    fi
    success "Checksum verified"

    # Extract
    info "Extracting..."
    tar -xzf "${tmp_dir}/${asset_name}" -C "$tmp_dir"

    # Install
    info "Installing to ${INSTALL_DIR}..."
    mkdir -p "$INSTALL_DIR"
    mv "${tmp_dir}/agit" "${INSTALL_DIR}/agit"
    chmod +x "${INSTALL_DIR}/agit"

    success "agit v${version} installed successfully!"
}

# Check if install directory is in PATH
check_path() {
    if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
        echo ""
        warn "${INSTALL_DIR} is not in your PATH"
        echo ""
        echo "Add it to your shell configuration:"
        echo ""

        local shell_name
        shell_name="$(basename "$SHELL")"

        case "$shell_name" in
            bash)
                echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
                echo "  source ~/.bashrc"
                ;;
            zsh)
                echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
                echo "  source ~/.zshrc"
                ;;
            fish)
                echo "  fish_add_path ~/.local/bin"
                ;;
            *)
                echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
                ;;
        esac
        echo ""
    fi
}

main() {
    echo ""
    echo "  ╔═══════════════════════════════════════╗"
    echo "  ║       AGIT Installer                  ║"
    echo "  ║   AI-Native Git Wrapper               ║"
    echo "  ╚═══════════════════════════════════════╝"
    echo ""

    check_requirements

    local os arch version
    os="$(detect_os)"
    arch="$(detect_arch)"

    if [ -n "$VERSION" ]; then
        version="$VERSION"
        info "Installing specified version: v${version}"
    else
        info "Fetching latest version..."
        version="$(get_latest_version)"
        if [ -z "$version" ]; then
            error "Could not determine latest version"
        fi
        info "Latest version: v${version}"
    fi

    install_agit "$os" "$arch" "$version"
    check_path

    echo ""
    echo "Get started:"
    echo "  cd your-project"
    echo "  agit init"
    echo ""
    echo "Documentation: https://github.com/${REPO}"
    echo ""
}

main "$@"
