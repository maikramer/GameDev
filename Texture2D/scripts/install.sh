#!/bin/bash
#
# Texture2D — wrapper do instalador Python
# Uso: ./scripts/install.sh [opções]
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_CMD="${PYTHON_CMD:-python3}"

RED='\033[0;31m'
NC='\033[0m'

log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

if ! command -v "$PYTHON_CMD" &> /dev/null; then
    log_error "Python não encontrado. Instale Python 3.10+"
    exit 1
fi

INSTALLER="$SCRIPT_DIR/installer.py"
if [ ! -f "$INSTALLER" ]; then
    log_error "installer.py não encontrado: $INSTALLER"
    exit 1
fi

exec "$PYTHON_CMD" "$INSTALLER" "$@"
