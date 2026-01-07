#!/bin/bash
#
# RustWops Installer
# Installs the latest release of RustWops on Ubuntu systems
#

set -e

REPO="mihained/rustwops"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="rw"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${CYAN}"
echo "  ____           _   __        __"
echo " |  _ \ _   _ ___| |_ \ \      / /__  _ __  ___"
echo " | |_) | | | / __| __| \ \ /\ / / _ \| '_ \/ __|"
echo " |  _ <| |_| \__ \ |_   \ V  V / (_) | |_) \__ \\"
echo " |_| \_\\\\__,_|___/\__|   \_/\_/ \___/| .__/|___/"
echo "                                     |_|"
echo -e "${NC}"
echo "  RustWops Installer"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error: Please run as root (sudo)${NC}"
    exit 1
fi

# Check OS
if [ ! -f /etc/os-release ]; then
    echo -e "${RED}Error: Cannot detect OS. This installer is for Ubuntu only.${NC}"
    exit 1
fi

source /etc/os-release
if [ "$ID" != "ubuntu" ]; then
    echo -e "${YELLOW}Warning: This installer is designed for Ubuntu. Your OS: $ID${NC}"
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Detect architecture
ARCH=$(uname -m)
case $ARCH in
    x86_64)
        ARCH="x86_64"
        ;;
    aarch64|arm64)
        ARCH="aarch64"
        ;;
    *)
        echo -e "${RED}Error: Unsupported architecture: $ARCH${NC}"
        exit 1
        ;;
esac

echo -e "${GREEN}→${NC} Detected: Ubuntu $VERSION_ID ($ARCH)"

# Get latest release
echo -e "${GREEN}→${NC} Fetching latest release..."
LATEST_RELEASE=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
    # No releases yet, build from source
    echo -e "${YELLOW}→${NC} No releases found. Building from source..."

    # Install dependencies
    echo -e "${GREEN}→${NC} Installing build dependencies..."
    apt-get update -qq
    apt-get install -y -qq curl build-essential pkg-config libssl-dev git

    # Install Rust if not present
    if ! command -v cargo &> /dev/null; then
        echo -e "${GREEN}→${NC} Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    # Clone and build
    echo -e "${GREEN}→${NC} Cloning repository..."
    TEMP_DIR=$(mktemp -d)
    git clone --depth 1 "https://github.com/$REPO.git" "$TEMP_DIR"
    cd "$TEMP_DIR"

    echo -e "${GREEN}→${NC} Building RustWops (this may take a few minutes)..."
    cargo build --release

    # Install
    echo -e "${GREEN}→${NC} Installing binary..."
    cp target/release/rw "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    # Cleanup
    cd /
    rm -rf "$TEMP_DIR"
else
    # Download pre-built binary
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_RELEASE/rw-linux-$ARCH"

    echo -e "${GREEN}→${NC} Downloading RustWops $LATEST_RELEASE..."

    if ! curl -fsSL "$DOWNLOAD_URL" -o "$INSTALL_DIR/$BINARY_NAME"; then
        echo -e "${YELLOW}→${NC} Pre-built binary not available. Building from source..."

        # Fall back to building from source
        apt-get update -qq
        apt-get install -y -qq curl build-essential pkg-config libssl-dev git

        if ! command -v cargo &> /dev/null; then
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            source "$HOME/.cargo/env"
        fi

        TEMP_DIR=$(mktemp -d)
        git clone --depth 1 --branch "$LATEST_RELEASE" "https://github.com/$REPO.git" "$TEMP_DIR"
        cd "$TEMP_DIR"
        cargo build --release
        cp target/release/rw "$INSTALL_DIR/$BINARY_NAME"
        cd /
        rm -rf "$TEMP_DIR"
    fi

    chmod +x "$INSTALL_DIR/$BINARY_NAME"
fi

# Verify installation
if command -v rw &> /dev/null; then
    VERSION=$(rw --version 2>/dev/null | head -1)
    echo ""
    echo -e "${GREEN}✓${NC} RustWops installed successfully!"
    echo -e "  Version: $VERSION"
    echo -e "  Location: $INSTALL_DIR/$BINARY_NAME"
    echo ""
    echo -e "  Run ${CYAN}rw${NC} to start the interactive mode"
    echo -e "  Run ${CYAN}rw --help${NC} to see all commands"
    echo ""
else
    echo -e "${RED}✗${NC} Installation failed"
    exit 1
fi
