#!/usr/bin/env bash
# install_flash_attn.sh — instala flash-attn via wheel pré-compilado (segundos)
# ou compila do source como fallback.
#
# Uso:
#   bash scripts/install_flash_attn.sh              # auto-detecta tudo
#   bash scripts/install_flash_attn.sh --pip .venv/bin/pip
#   bash scripts/install_flash_attn.sh --force-build # compilar mesmo se há wheel
#
# Fonte dos wheels: https://github.com/mjun0812/flash-attention-prebuild-wheels
set -euo pipefail

WHEELS_REPO="mjun0812/flash-attention-prebuild-wheels"
FLASH_ATTN_VERSION="2.8.3"

PIP=""
FORCE_BUILD=false
MAX_JOBS="${MAX_JOBS:-2}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --pip)       PIP="$2"; shift ;;
        --force-build) FORCE_BUILD=true ;;
        --max-jobs)  MAX_JOBS="$2"; shift ;;
        *) echo "Argumento desconhecido: $1"; exit 1 ;;
    esac
    shift
done

# --- Resolver pip ---
if [[ -z "$PIP" ]]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
    if [[ -x "$PROJECT_DIR/.venv/bin/pip" ]]; then
        PIP="$PROJECT_DIR/.venv/bin/pip"
    else
        PIP="$(command -v pip 2>/dev/null || true)"
    fi
fi
if [[ -z "$PIP" || ! -x "$PIP" ]]; then
    echo "ERRO: pip não encontrado. Passa --pip /caminho/para/pip" >&2
    exit 1
fi

PYTHON="$(dirname "$PIP")/python"
if [[ ! -x "$PYTHON" ]]; then
    PYTHON="$(command -v python3 2>/dev/null || command -v python 2>/dev/null)"
fi

echo "=== install_flash_attn.sh ==="
echo "pip:    $PIP"
echo "python: $PYTHON"

# --- Já instalado? ---
if "$PYTHON" -c "import flash_attn; print(f'flash_attn {flash_attn.__version__} já instalado')" 2>/dev/null; then
    if [[ "$FORCE_BUILD" == false ]]; then
        echo "Nada a fazer. Usa --force-build para reinstalar."
        exit 0
    fi
fi

# --- Detectar torch / CUDA / Python ---
read -r TORCH_VER CUDA_VER PYVER ABI < <("$PYTHON" -c "
import torch, sys, platform
tv = torch.__version__.split('+')[0]
# Versão major.minor do torch (ex: 2.11 → 2.10 para compatibilidade ABI de wheels)
parts = tv.split('.')
major, minor = int(parts[0]), int(parts[1])
# Wheels disponíveis: torch 2.9, 2.10. Torch 2.11+ é ABI-compatível com 2.10.
if minor >= 10:
    wheel_torch = f'{major}.10'
elif minor >= 9:
    wheel_torch = f'{major}.9'
else:
    wheel_torch = ''

cuda = torch.version.cuda or ''
cuda_short = ''
if cuda:
    cp = cuda.split('.')
    cm = int(cp[0])
    cn = int(cp[1]) if len(cp) > 1 else 0
    # Mapear para versões de CUDA disponíveis nos wheels: 12.6, 12.8, 13.0
    if cm == 13:
        cuda_short = 'cu130'
    elif cm == 12 and cn >= 7:
        cuda_short = 'cu128'
    elif cm == 12:
        cuda_short = 'cu126'

pyver = f'cp{sys.version_info.major}{sys.version_info.minor}'
abi = '1' if torch.compiled_with_cxx11_abi() else '0'
print(f'{wheel_torch} {cuda_short} {pyver} {abi}')
")

echo "torch (wheel): $TORCH_VER  CUDA: $CUDA_VER  Python: $PYVER  CXX11_ABI: $ABI"

# --- Tentar wheel pré-compilado ---
install_wheel() {
    if [[ -z "$TORCH_VER" || -z "$CUDA_VER" || -z "$PYVER" ]]; then
        echo "Não foi possível determinar torch/CUDA/Python para escolher wheel."
        return 1
    fi

    local tag="v0.9.0"
    local base="https://github.com/${WHEELS_REPO}/releases/download/${tag}"
    local whl="flash_attn-${FLASH_ATTN_VERSION}+${CUDA_VER}torch${TORCH_VER}-${PYVER}-${PYVER}-linux_x86_64.whl"
    local url="${base}/${whl}"

    echo ""
    echo "Tentando wheel pré-compilado:"
    echo "  $url"
    echo ""

    if "$PIP" install --force-reinstall "$url" 2>&1; then
        echo ""
        echo "flash-attn instalado com sucesso via wheel pré-compilado."
        return 0
    else
        echo ""
        echo "Wheel não encontrado ou falhou. Tentando manylinux..."
        local whl_ml="flash_attn-${FLASH_ATTN_VERSION}+${CUDA_VER}torch${TORCH_VER}-${PYVER}-${PYVER}-manylinux_2_24_x86_64.manylinux_2_28_x86_64.whl"
        local url_ml="${base}/${whl_ml}"
        if "$PIP" install --force-reinstall "$url_ml" 2>&1; then
            echo ""
            echo "flash-attn instalado com sucesso via wheel manylinux."
            return 0
        fi
        return 1
    fi
}

build_from_source() {
    echo ""
    echo "=== Compilando flash-attn do source (MAX_JOBS=$MAX_JOBS) ==="
    echo "Isto pode demorar 15-60 min dependendo do hardware."
    echo ""

    local cuda_home="${CUDA_HOME:-}"
    if [[ -z "$cuda_home" ]]; then
        for d in /usr/local/cuda /usr/local/cuda-13.2 /usr/local/cuda-13.0 /usr/local/cuda-12.8 /usr/local/cuda-12.6; do
            if [[ -x "$d/bin/nvcc" ]]; then
                cuda_home="$d"
                break
            fi
        done
    fi
    if [[ -n "$cuda_home" ]]; then
        export CUDA_HOME="$cuda_home"
        export PATH="$cuda_home/bin:$PATH"
        echo "CUDA_HOME=$CUDA_HOME"
    fi

    export MAX_JOBS="$MAX_JOBS"
    export NVCC_THREADS="$MAX_JOBS"
    export CMAKE_BUILD_PARALLEL_LEVEL="$MAX_JOBS"
    export NINJAFLAGS="-j${MAX_JOBS}"

    mkdir -p "${HOME}/tmp"
    export TMPDIR="${HOME}/tmp"

    "$PIP" install flash-attn --no-build-isolation --no-cache-dir
}

if [[ "$FORCE_BUILD" == true ]]; then
    build_from_source
elif ! install_wheel; then
    echo ""
    echo "Nenhum wheel pré-compilado disponível para esta combinação."
    echo "A compilar do source..."
    build_from_source
fi

echo ""
"$PYTHON" -c "import flash_attn; print(f'Verificação: flash_attn {flash_attn.__version__} OK')"
echo "Concluído."
