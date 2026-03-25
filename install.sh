#!/bin/bash
# =============================================================================
# GameDev Monorepo — Instalador Unificado (Linux/macOS)
# =============================================================================
#
# Instala qualquer ferramenta do monorepo GameDev.
#
# Uso:
#   ./install.sh <tool>           # Instalar uma ferramenta
#   ./install.sh all              # Instalar tudo
#   ./install.sh --list           # Listar ferramentas
#   ./install.sh materialize      # Instalar Materialize (Rust)
#   ./install.sh text2d --use-venv
#
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SHARED_SRC="$SCRIPT_DIR/Shared/src"
PYTHON_CMD="${PYTHON_CMD:-python3}"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

if ! command -v "$PYTHON_CMD" &> /dev/null; then
    echo -e "${RED}✗ Python 3 não encontrado.${NC}"
    echo "Instale Python 3.10+:"
    echo "  Ubuntu/Debian: sudo apt install python3"
    echo "  macOS: brew install python3"
    exit 1
fi

echo -e "${CYAN}GameDev Monorepo — Instalador Unificado${NC}"
echo "========================================"

PYTHONPATH="$SHARED_SRC:${PYTHONPATH:-}" \
    exec "$PYTHON_CMD" -m gamedev_shared.installer.unified "$@"
