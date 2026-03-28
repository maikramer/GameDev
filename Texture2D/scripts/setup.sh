#!/bin/bash
# Texture2D — ambiente virtual e dependências

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENV_DIR="$PROJECT_ROOT/.venv"
PYTHON_CMD="${PYTHON_CMD:-python3}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo "=========================================="
echo "  Texture2D — Setup"
echo "=========================================="

if ! command -v "$PYTHON_CMD" &> /dev/null; then
    log_error "Python não encontrado. Instale Python 3.10+"
    exit 1
fi

VERSION_OK=$($PYTHON_CMD -c "import sys; print('OK' if sys.version_info >= (3, 10) else 'FAIL')")
if [ "$VERSION_OK" != "OK" ]; then
    log_error "Python 3.10+ necessário."
    exit 1
fi

if [ -d "$VENV_DIR" ]; then
    log_warn "Removendo venv existente..."
    rm -rf "$VENV_DIR"
fi

log_info "Criando venv em $VENV_DIR..."
$PYTHON_CMD -m venv "$VENV_DIR"
# shellcheck source=/dev/null
source "$VENV_DIR/bin/activate"

log_info "Atualizando pip..."
pip install --upgrade pip "setuptools>=68,<82" wheel

log_info "Instalando dependências do projeto..."
pip install -r "$PROJECT_ROOT/config/requirements.txt"

log_info "Instalação editable..."
pip install -e "$PROJECT_ROOT"

log_info "Verificação..."
python -c "import gamedev_shared; print('gamedev-shared OK')"
python -c "from huggingface_hub import InferenceClient; print('huggingface_hub:', InferenceClient.__module__)"
python -c "from PIL import Image; print('Pillow OK')"

mkdir -p "$PROJECT_ROOT/outputs/textures"

chmod +x "$PROJECT_ROOT/activate.sh" 2>/dev/null || true

echo ""
echo "=========================================="
echo -e "${GREEN}  Concluído.${NC}"
echo "=========================================="
echo "  source $PROJECT_ROOT/.venv/bin/activate"
echo "  texture2d --help"
echo '  texture2d generate "rough stone wall surface"'
