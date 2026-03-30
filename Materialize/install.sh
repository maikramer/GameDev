#!/bin/bash
# =============================================================================
# Materialize CLI Installer - Linux/macOS
# =============================================================================
#
# Installs Materialize CLI (generates PBR maps from textures).
# Requires: Python 3 (for the installer), Rust/cargo (to build).
#
# Usage:
#   ./install.sh              # Install to ~/.local/bin
#   ./install.sh uninstall    # Uninstall
#   ./install.sh reinstall   # Reinstall
#
# =============================================================================

set -e

cd "$(dirname "$0")"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}Materialize CLI Installer${NC}"
echo "================================"

if ! command -v python3 &> /dev/null; then
    echo -e "${RED}✗ Python 3 not found${NC}"
    echo "Install Python 3 (required to run the installer):"
    echo "  Ubuntu/Debian: sudo apt install python3"
    echo "  macOS: brew install python3"
    exit 1
fi

echo -e "${GREEN}✓ Python 3 found${NC}"

python3 installer/installer.py "$@"
