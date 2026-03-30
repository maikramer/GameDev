#!/bin/bash
#
# Text3D — executa scripts/installer.py (implementação).
# O ficheiro install.sh nesta pasta delega para este script por compatibilidade.
# O instalador oficial do monorepo é GameDev/install.sh (na raiz do repositório).
#
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_CMD="${PYTHON_CMD:-python3}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

if ! command -v $PYTHON_CMD &> /dev/null; then
    log_error "Python não encontrado. Instale Python 3.8+"
    exit 1
fi

INSTALLER="$SCRIPT_DIR/installer.py"
if [ ! -f "$INSTALLER" ]; then
    log_error "installer.py não encontrado em: $SCRIPT_DIR"
    exit 1
fi

exec $PYTHON_CMD "$INSTALLER" "$@"
