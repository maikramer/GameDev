#!/bin/bash
#
# Text3D System-Wide Installer Wrapper
# Este script é um wrapper que chama install.py
# Uso: ./install.sh [opções]
#

set -e

# Diretório do script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_CMD="${PYTHON_CMD:-python3}"

# Cores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Verificar Python
if ! command -v $PYTHON_CMD &> /dev/null; then
    log_error "Python não encontrado. Instale Python 3.8+"
    exit 1
fi

# Verificar installer.py existe (na mesma pasta scripts/)
INSTALLER="$SCRIPT_DIR/installer.py"
if [ ! -f "$INSTALLER" ]; then
    log_error "installer.py não encontrado em: $SCRIPT_DIR"
    exit 1
fi

# Passar todos os argumentos para installer.py
exec $PYTHON_CMD "$INSTALLER" "$@"
