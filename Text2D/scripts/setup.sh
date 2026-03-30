#!/bin/bash
# Conveniência para dev: cria `.venv` e instala em modo editável.
# Instalação oficial do monorepo: ../../README.md ou ../../docs/INSTALLING.md (`./install.sh text2d` na raiz GameDev).
#
# Text2D — ambiente virtual e dependências

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
echo "  Text2D — Setup"
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

PY_MINOR=$($PYTHON_CMD -c "import sys; print(sys.version_info[1])")

if command -v nvidia-smi &> /dev/null; then
    CUDA_VERSION=$(nvidia-smi 2>/dev/null | grep "CUDA Version" | sed 's/.*CUDA Version: \([0-9]*\.[0-9]*\).*/\1/' || echo "11.8")
    log_info "CUDA detectada: $CUDA_VERSION"
    # Python 3.13+: wheels cu121 do índice PyTorch podem não incluir torchvision compatível;
    # o PyPI oficial distribui torch+torchvision com CUDA (ex. cu128) alinhados.
    if [[ "$PY_MINOR" -ge 13 ]]; then
        log_info "Python 3.13+ — PyTorch+CUDA a partir do PyPI..."
        pip install torch torchvision
    elif [[ "$CUDA_VERSION" == 12* ]]; then
        pip install torch torchvision --index-url https://download.pytorch.org/whl/cu121
    else
        pip install torch torchvision --index-url https://download.pytorch.org/whl/cu118
    fi
else
    log_warn "Sem NVIDIA — PyTorch CPU"
    pip install torch torchvision --index-url https://download.pytorch.org/whl/cpu
fi

log_info "Instalando dependências do projeto..."
pip install -r "$PROJECT_ROOT/config/requirements.txt"

log_info "Instalação editable..."
pip install -e "$PROJECT_ROOT"

log_info "Verificação..."
python -c "import torch; print('PyTorch:', torch.__version__, '| CUDA:', torch.cuda.is_available())"
python -c "import diffusers; print('Diffusers:', diffusers.__version__)"

mkdir -p "$PROJECT_ROOT/outputs/images"

chmod +x "$PROJECT_ROOT/activate.sh" 2>/dev/null || true

echo ""
echo "=========================================="
echo -e "${GREEN}  Concluído.${NC}"
echo "=========================================="
echo "  source $PROJECT_ROOT/.venv/bin/activate"
echo "  text2d --help"
echo "  text2d generate \"o seu prompt\""
