#!/usr/bin/env bash
# Conveniência para dev: venv local e `pip install -e`. Instalação oficial: ../../docs/INSTALLING.md (`./install.sh gameassets` na raiz GameDev).
#
# GameAssets — venv local e instalação do pacote (paridade com Text2D/Text3D)

set -euo pipefail

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

RECREATE=0
INSTALL_DEV=0
for arg in "$@"; do
  case "$arg" in
    --recreate) RECREATE=1 ;;
    --dev) INSTALL_DEV=1 ;;
  esac
done

echo "=========================================="
echo "  GameAssets — Setup"
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

if [ -d "$VENV_DIR" ] && [ "$RECREATE" -eq 1 ]; then
  log_warn "A remover venv existente (--recreate)..."
  rm -rf "$VENV_DIR"
fi

if [ ! -d "$VENV_DIR" ]; then
  log_info "A criar venv em $VENV_DIR..."
  $PYTHON_CMD -m venv "$VENV_DIR"
else
  log_info "A reutilizar venv existente: $VENV_DIR (usa --recreate para recriar)"
fi

# shellcheck source=/dev/null
source "$VENV_DIR/bin/activate"

log_info "A atualizar pip..."
pip install --upgrade pip "setuptools>=68,<82" wheel

cd "$PROJECT_ROOT"
if [ "$INSTALL_DEV" -eq 1 ]; then
  log_info "A instalar pacote em modo editável + extras [dev] (pytest)..."
  pip install -e ".[dev]"
else
  log_info "A instalar dependências (config/requirements.txt) e pacote em modo editável..."
  pip install -e .
fi

log_info "Verificação..."
gameassets --version

chmod +x "$PROJECT_ROOT/activate.sh" 2>/dev/null || true
chmod +x "$PROJECT_ROOT/scripts/setup.sh" 2>/dev/null || true

echo ""
echo "=========================================="
echo -e "${GREEN}  Concluído.${NC}"
echo "=========================================="
echo "  source $VENV_DIR/bin/activate"
echo "  gameassets --help"
echo ""
echo "  Ou um comando com venv já ativo: $PROJECT_ROOT/activate.sh gameassets --help"
echo ""
echo "  Batch com GPU: instala Text2D e Text3D (cada um com o seu ./scripts/setup.sh)"
echo "  e garante text2d/text3d no PATH, ou define TEXT2D_BIN / TEXT3D_BIN."
echo ""
