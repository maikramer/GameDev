#!/bin/bash
# Conveniência para dev: cria `.venv` e dependências. Instalação oficial: ../../docs/INSTALLING.md (`./install.sh text3d` na raiz GameDev).
#
# Text3D — Setup completo (venv + deps; textura/PBR no projeto Paint3D)
#
# Pipeline: Text2D → Hunyuan3D shape → repair/remesh (sem Paint neste pacote)
#
# Uso:
#   bash scripts/setup.sh                   # setup padrão
#   PYTHON_CMD=python3.11 bash scripts/setup.sh  # forçar versão Python

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENV_DIR="$PROJECT_ROOT/.venv"
PYTHON_CMD="${PYTHON_CMD:-python3}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step()  { echo -e "\n${GREEN}[STEP]${NC} $1"; }

echo "=========================================="
echo "  Text3D — Setup de Ambiente"
echo "=========================================="
echo ""

# ── Python ────────────────────────────────────────────────────────────
log_step "Verificando Python..."
if ! command -v $PYTHON_CMD &>/dev/null; then
  log_error "Python não encontrado. Instale Python 3.10+"
  exit 1
fi

PYTHON_VERSION=$($PYTHON_CMD --version 2>&1 | cut -d' ' -f2)
log_info "Python: $PYTHON_VERSION"

VERSION_OK=$($PYTHON_CMD -c "import sys; print('OK' if sys.version_info >= (3, 10) else 'FAIL')")
if [ "$VERSION_OK" != "OK" ]; then
  log_error "Python 3.10+ necessário. Versão atual: $PYTHON_VERSION"
  exit 1
fi

# ── CUDA ──────────────────────────────────────────────────────────────
log_step "Verificando CUDA..."
if command -v nvidia-smi &>/dev/null; then
  log_info "GPU detectada:"
  nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null | while read line; do
    echo "  - $line"
  done
  CUDA_VERSION=$(nvidia-smi 2>/dev/null | grep "CUDA Version" | sed 's/.*CUDA Version: \([0-9]*\.[0-9]*\).*/\1/' || echo "")
  if [ -n "$CUDA_VERSION" ]; then
    log_info "CUDA (driver): $CUDA_VERSION"
  fi
else
  log_warn "CUDA não detectado. Text3D shape em CPU é possível; Paint3D (textura) em geral requer CUDA."
fi

# Autodetectar CUDA_HOME
if [ -z "${CUDA_HOME:-}" ]; then
  for candidate in /usr/local/cuda-* /usr/local/cuda /usr/lib/cuda; do
    if [ -x "$candidate/bin/nvcc" ]; then
      export CUDA_HOME="$candidate"
      break
    fi
  done
fi
if [ -n "${CUDA_HOME:-}" ]; then
  log_info "CUDA_HOME=$CUDA_HOME"
fi

# ── venv ──────────────────────────────────────────────────────────────
log_step "Ambiente virtual..."
if [ -d "$VENV_DIR" ]; then
  log_info "Venv existente: $VENV_DIR"
else
  log_info "Criando venv em $VENV_DIR..."
  $PYTHON_CMD -m venv "$VENV_DIR"
fi

source "$VENV_DIR/bin/activate"
log_info "Ativado: $(which python)"

# ── pip bootstrap ─────────────────────────────────────────────────────
log_step "Atualizando pip..."
pip install --upgrade pip "setuptools>=68,<82" wheel

# ── PyTorch ───────────────────────────────────────────────────────────
log_step "PyTorch..."
if python -c "import torch; print(f'PyTorch {torch.__version__} CUDA={torch.cuda.is_available()}')" 2>/dev/null; then
  log_info "PyTorch já instalado."
else
  if command -v nvidia-smi &>/dev/null; then
    CUDA_MAJOR=$(echo "${CUDA_VERSION:-0}" | cut -d. -f1)
    PY_MINOR=$(python -c "import sys; print(sys.version_info[1])")
    if [ "$CUDA_MAJOR" -ge 13 ] || [ "$PY_MINOR" -ge 13 ]; then
      log_info "PyTorch via PyPI (CUDA $CUDA_VERSION / Python 3.$PY_MINOR)..."
      pip install torch torchvision
    elif [ "$CUDA_MAJOR" -eq 12 ]; then
      log_info "PyTorch para CUDA 12..."
      pip install torch torchvision --index-url https://download.pytorch.org/whl/cu121
    else
      log_info "PyTorch para CUDA 11.8..."
      pip install torch torchvision --index-url https://download.pytorch.org/whl/cu118
    fi
  else
    log_warn "Instalando PyTorch CPU..."
    pip install torch torchvision --index-url https://download.pytorch.org/whl/cpu
  fi
fi

# ── Text3D (editable) ────────────────────────────────────────────────
log_step "Instalando Text3D (editable)..."
pip install -e "$PROJECT_ROOT"

# ── Paint3D (textura Hunyuan) ─────────────────────────────────────────
log_step "Paint3D (opcional)..."
log_info "Textura/PBR: instala o pacote ../Paint3D (./install.sh paint3d na raiz GameDev). Não faz parte deste venv."

# ── Verificação final ─────────────────────────────────────────────────
log_step "Verificando instalação..."
python -c "import torch; print(f'  PyTorch: {torch.__version__} (CUDA={torch.cuda.is_available()})')"
python -c "import pymeshlab; print(f'  PyMeshLab: OK')" 2>/dev/null || log_warn "pymeshlab não disponível"
python -c "import text3d; print('  Text3D: OK')" 2>/dev/null || log_warn "text3d não importável"

echo ""
echo "=========================================="
echo -e "${GREEN}  Setup concluído!${NC}"
echo "=========================================="
echo ""
echo "Para ativar o ambiente:"
echo "  source $VENV_DIR/bin/activate"
echo ""
echo "Shape (Text2D → Hunyuan):"
echo "  text3d generate 'um guerreiro medieval' -o guerreiro.glb"
echo ""
echo "Depois, textura com Paint3D (outro venv ou PATH):"
echo "  paint3d texture guerreiro.glb -i guerreiro_text2d.png -o guerreiro_tex.glb"
echo ""
echo "Diagnóstico:"
echo "  text3d doctor"
echo ""
