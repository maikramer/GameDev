#!/bin/bash
# Rigging3D — setup completo de inferência
# Cria .venv, instala PyTorch+CUDA, extras inference, spconv, torch-scatter/cluster, flash-attn.
#
# Uso:
#   bash scripts/setup.sh                  # auto-detecta tudo
#   bash scripts/setup.sh --python python3.11
#   bash scripts/setup.sh --skip-flash     # pula flash-attn
#   bash scripts/setup.sh --force          # recria venv do zero

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENV_DIR="$PROJECT_ROOT/.venv"

PYTHON_CMD="${PYTHON_CMD:-python3.11}"
SKIP_FLASH=false
FORCE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --python)     PYTHON_CMD="$2"; shift ;;
        --skip-flash) SKIP_FLASH=true ;;
        --force)      FORCE=true ;;
        -h|--help)
            echo "Uso: bash scripts/setup.sh [--python CMD] [--skip-flash] [--force]"
            echo ""
            echo "  --python CMD    Interpretador Python (default: python3.11)"
            echo "  --skip-flash    Não instalar flash-attn"
            echo "  --force         Recriar .venv do zero"
            exit 0
            ;;
        *) echo "Argumento desconhecido: $1"; exit 1 ;;
    esac
    shift
done

# ---------------------------------------------------------------------------
# Cores e logging
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log_step()  { echo -e "\n${BOLD}${CYAN}▸ $1${NC}"; }
log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERRO]${NC} $1"; }
log_ok()    { echo -e "${GREEN}  ✓${NC} $1"; }

# ---------------------------------------------------------------------------
# 1. Verificar Python
# ---------------------------------------------------------------------------
log_step "Verificando Python"

if ! command -v "$PYTHON_CMD" &>/dev/null; then
    log_error "Python não encontrado: $PYTHON_CMD"
    log_info "Instala Python 3.11: uv python install 3.11  OU  sudo apt install python3.11"
    exit 1
fi

PY_VERSION=$("$PYTHON_CMD" --version 2>&1)
PY_MINOR=$("$PYTHON_CMD" -c "import sys; print(sys.version_info[1])")
log_info "$PY_VERSION"

if [[ "$PY_MINOR" -lt 10 ]]; then
    log_error "Python 3.10+ necessário (recomendado 3.11)"
    exit 1
fi

if [[ "$PY_MINOR" -ne 11 ]]; then
    log_warn "Python 3.11 é recomendado (bpy==4.2.0 e open3d requerem cp311)."
    log_warn "Com Python 3.$PY_MINOR, alguns pacotes podem não ter wheels."
fi

# ---------------------------------------------------------------------------
# 2. Verificar CUDA
# ---------------------------------------------------------------------------
log_step "Verificando GPU/CUDA"

CUDA_MAJOR=""
CUDA_MINOR=""
CUDA_VERSION=""

if command -v nvidia-smi &>/dev/null; then
    CUDA_VERSION=$(nvidia-smi 2>/dev/null | grep -oP "CUDA Version: \K[0-9]+\.[0-9]+" || true)
    if [[ -n "$CUDA_VERSION" ]]; then
        CUDA_MAJOR="${CUDA_VERSION%%.*}"
        CUDA_MINOR="${CUDA_VERSION##*.}"
        log_info "GPU detectada:"
        nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null | while IFS= read -r line; do
            echo "    $line"
        done
        log_info "CUDA Driver: $CUDA_VERSION"
    fi
fi

if [[ -z "$CUDA_VERSION" ]]; then
    log_warn "CUDA não detectado. O Rigging3D requer GPU NVIDIA."
    log_warn "A instalação continua mas a inferência não funcionará sem GPU."
fi

# ---------------------------------------------------------------------------
# 3. Criar/recriar .venv
# ---------------------------------------------------------------------------
log_step "Ambiente virtual"

if [[ "$FORCE" == true && -d "$VENV_DIR" ]]; then
    log_warn "Removendo venv existente (--force)..."
    rm -rf "$VENV_DIR"
fi

if [[ ! -d "$VENV_DIR" ]]; then
    log_info "Criando $VENV_DIR..."
    "$PYTHON_CMD" -m venv "$VENV_DIR"
    log_ok "Venv criado"
else
    log_info "Venv existente: $VENV_DIR"
fi

PIP="$VENV_DIR/bin/pip"
PYTHON="$VENV_DIR/bin/python"
source "$VENV_DIR/bin/activate"

log_info "Atualizando pip/setuptools/wheel..."
"$PIP" install --upgrade pip "setuptools>=68,<82" wheel --quiet

# ---------------------------------------------------------------------------
# 4. Instalar PyTorch
# ---------------------------------------------------------------------------
log_step "PyTorch"

TORCH_INSTALLED=false
if "$PYTHON" -c "import torch; print(f'torch {torch.__version__}')" 2>/dev/null; then
    EXISTING_TORCH=$("$PYTHON" -c "import torch; print(torch.__version__)")
    log_info "PyTorch já instalado: $EXISTING_TORCH"
    if [[ "$FORCE" != true ]]; then
        TORCH_INSTALLED=true
    fi
fi

if [[ "$TORCH_INSTALLED" != true ]]; then
    if [[ -n "$CUDA_VERSION" ]]; then
        if [[ "$CUDA_MAJOR" -ge 13 ]]; then
            log_info "CUDA $CUDA_VERSION — instalando PyTorch via PyPI (inclui cu130)..."
            "$PIP" install torch torchvision
        elif [[ "$CUDA_MAJOR" -eq 12 && "$CUDA_MINOR" -ge 6 ]]; then
            log_info "CUDA $CUDA_VERSION — instalando PyTorch cu126..."
            "$PIP" install torch torchvision --index-url https://download.pytorch.org/whl/cu126
        elif [[ "$CUDA_MAJOR" -eq 12 ]]; then
            log_info "CUDA $CUDA_VERSION — instalando PyTorch cu121..."
            "$PIP" install torch torchvision --index-url https://download.pytorch.org/whl/cu121
        else
            log_info "CUDA $CUDA_VERSION — instalando PyTorch cu118..."
            "$PIP" install torch torchvision --index-url https://download.pytorch.org/whl/cu118
        fi
    else
        log_warn "Sem CUDA — instalando PyTorch CPU..."
        "$PIP" install torch torchvision --index-url https://download.pytorch.org/whl/cpu
    fi
    log_ok "PyTorch instalado"
fi

TORCH_VER=$("$PYTHON" -c "import torch; print(torch.__version__)")
TORCH_CUDA=$("$PYTHON" -c "import torch; print(torch.version.cuda or 'cpu')")
log_info "torch $TORCH_VER (CUDA runtime: $TORCH_CUDA)"

# ---------------------------------------------------------------------------
# 5. Instalar pacote Rigging3D + extras inference
# ---------------------------------------------------------------------------
log_step "Rigging3D + extras inference"

log_info "Instalando gamedev-shared + rigging3d[inference]..."
"$PIP" install -e "$PROJECT_ROOT/../Shared" --quiet
"$PIP" install -e "$PROJECT_ROOT[inference]" --quiet
log_ok "rigging3d[inference] instalado"

# ---------------------------------------------------------------------------
# 6. Dependências CUDA-specific (torch-scatter, torch-cluster, spconv, cumm)
# ---------------------------------------------------------------------------
log_step "Dependências CUDA (torch-scatter, torch-cluster, spconv, cumm)"

TORCH_SHORT=$("$PYTHON" -c "
import torch
v = torch.__version__.split('+')[0]
parts = v.split('.')
print(f'{parts[0]}.{parts[1]}.0')
")
TORCH_CUDA_TAG=$("$PYTHON" -c "
import torch
c = torch.version.cuda or ''
if c:
    parts = c.split('.')
    print(f'cu{parts[0]}{parts[1]}')
else:
    print('cpu')
")

log_info "torch=$TORCH_SHORT  cuda_tag=$TORCH_CUDA_TAG"

# torch-scatter e torch-cluster
SCATTER_URL="https://data.pyg.org/whl/torch-${TORCH_SHORT}+${TORCH_CUDA_TAG}.html"
log_info "Instalando torch-scatter, torch-cluster de $SCATTER_URL ..."
"$PIP" install torch-scatter torch-cluster -f "$SCATTER_URL" --quiet 2>&1 || {
    log_warn "Wheels torch-scatter/cluster não disponíveis para torch $TORCH_SHORT+$TORCH_CUDA_TAG."
    log_warn "Tentando compilação pip (pode demorar)..."
    "$PIP" install torch-scatter torch-cluster --quiet || log_warn "Falha na instalação de torch-scatter/cluster"
}
log_ok "torch-scatter/cluster"

# spconv + cumm (versão depende do CUDA)
SPCONV_PKG=""
CUMM_PKG=""
if [[ "$TORCH_CUDA_TAG" == cu130 || "$TORCH_CUDA_TAG" == cu126 || "$TORCH_CUDA_TAG" == cu128 ]]; then
    SPCONV_PKG="spconv-cu121"
    CUMM_PKG="cumm-cu121"
elif [[ "$TORCH_CUDA_TAG" == cu121 || "$TORCH_CUDA_TAG" == cu124 ]]; then
    SPCONV_PKG="spconv-cu121"
    CUMM_PKG="cumm-cu121"
elif [[ "$TORCH_CUDA_TAG" == cu118 ]]; then
    SPCONV_PKG="spconv-cu118"
    CUMM_PKG="cumm-cu118"
fi

if [[ -n "$SPCONV_PKG" ]]; then
    log_info "Instalando $SPCONV_PKG + $CUMM_PKG ..."
    "$PIP" install "$CUMM_PKG" "$SPCONV_PKG" --quiet
    log_ok "$SPCONV_PKG + $CUMM_PKG"
else
    log_warn "spconv: sem pacote CUDA para $TORCH_CUDA_TAG (instala manualmente se necessário)"
fi

# ---------------------------------------------------------------------------
# 7. flash-attn (opcional)
# ---------------------------------------------------------------------------
if [[ "$SKIP_FLASH" != true ]]; then
    log_step "flash-attn"

    if "$PYTHON" -c "import flash_attn" 2>/dev/null; then
        FA_VER=$("$PYTHON" -c "import flash_attn; print(flash_attn.__version__)")
        log_info "flash-attn $FA_VER já instalado"
    else
        log_info "Executando scripts/install_flash_attn.sh ..."
        bash "$SCRIPT_DIR/install_flash_attn.sh" --pip "$PIP" || {
            log_warn "flash-attn não instalado (o pipeline usa SDPA como fallback)."
            log_warn "Para instalar depois: bash scripts/install_flash_attn.sh"
        }
    fi
else
    log_info "flash-attn ignorado (--skip-flash)"
fi

# ---------------------------------------------------------------------------
# 8. Verificação
# ---------------------------------------------------------------------------
log_step "Verificação final"

"$PYTHON" -c "
import torch
print(f'  torch:       {torch.__version__}  (CUDA: {torch.version.cuda or \"cpu\"})')
print(f'  GPU:         {torch.cuda.get_device_name(0) if torch.cuda.is_available() else \"N/A\"}')" 2>/dev/null || true

"$PYTHON" -c "import spconv; print(f'  spconv:      {spconv.__version__}')" 2>/dev/null || log_warn "  spconv: não instalado"
"$PYTHON" -c "import torch_scatter; print('  torch-scatter: OK')" 2>/dev/null || log_warn "  torch-scatter: não instalado"
"$PYTHON" -c "import torch_cluster; print('  torch-cluster: OK')" 2>/dev/null || log_warn "  torch-cluster: não instalado"
"$PYTHON" -c "import flash_attn; print(f'  flash-attn:  {flash_attn.__version__}')" 2>/dev/null || log_info "  flash-attn:  não instalado (fallback SDPA)"
"$PYTHON" -c "import bpy; print(f'  bpy:         {bpy.app.version_string}')" 2>/dev/null || log_warn "  bpy: não instalado"
"$PYTHON" -c "import lightning; print(f'  lightning:   {lightning.__version__}')" 2>/dev/null || log_warn "  lightning: não instalado"
"$PYTHON" -c "import rigging3d; print(f'  rigging3d:   {rigging3d.__version__}')" 2>/dev/null || log_warn "  rigging3d: não instalado"

echo ""
echo -e "${BOLD}${GREEN}══════════════════════════════════════════${NC}"
echo -e "${BOLD}${GREEN}  Rigging3D — setup concluído!${NC}"
echo -e "${BOLD}${GREEN}══════════════════════════════════════════${NC}"
echo ""
echo "  Ativar venv:    source .venv/bin/activate"
echo "  Testar:         rigging3d --help"
echo "  Pipeline:       rigging3d pipeline -i mesh.glb -o rigged.glb"
echo ""
echo "  Os pesos HF são descarregados automaticamente na 1ª execução."
echo ""
