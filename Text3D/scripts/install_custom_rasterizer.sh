#!/usr/bin/env bash
# Compila e instala custom_rasterizer (Hunyuan3D-Paint) no venv ativo.
# Documentação: docs/PAINT_SETUP.md
#
# Uso (a partir da raiz Text3D, monorepo GameDev):
#   source .venv/bin/activate
#   bash scripts/install_custom_rasterizer.sh
#
# Requisitos: nvcc (ex.: nvidia-cuda-toolkit), PyTorch CUDA alinhado ao nvcc
# (se o driver tiver CUDA 12.x e o torch for cu118, vê o wrapper em PAINT_SETUP.md).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${VIRTUAL_ENV:-}" ]]; then
  VENV_PY="$VIRTUAL_ENV/bin/python"
elif [[ -x "$ROOT/.venv/bin/python" ]]; then
  VENV_PY="$ROOT/.venv/bin/python"
  echo "[INFO] Usando $ROOT/.venv"
else
  echo "[ERROR] Ativa o venv do Text3D ou define VIRTUAL_ENV." >&2
  exit 1
fi

if ! command -v nvcc &>/dev/null; then
  echo "[ERROR] nvcc não encontrado. Em Ubuntu: sudo apt install nvidia-cuda-toolkit nvidia-cuda-dev" >&2
  exit 1
fi

TMP="${HUNYUAN3D_CLONE:-/tmp/Hunyuan3D-2}"
if [[ ! -d "$TMP/hy3dgen/texgen/custom_rasterizer" ]]; then
  echo "[STEP] A clonar Hunyuan3D-2 em $TMP ..."
  git clone --depth 1 https://github.com/Tencent-Hunyuan/Hunyuan3D-2.git "$TMP"
fi

CR_DIR="$TMP/hy3dgen/texgen/custom_rasterizer"

if [[ -z "${CUDA_HOME:-}" ]]; then
  if [[ -d /usr/lib/nvidia-cuda-toolkit/bin ]]; then
    export CUDA_HOME="/usr/lib/nvidia-cuda-toolkit"
    echo "[INFO] CUDA_HOME=$CUDA_HOME (toolkit do apt; se o build falhar por versão, define CUDA_HOME ou lê docs/PAINT_SETUP.md)"
  else
    echo "[WARN] CUDA_HOME não definido. Ex.: export CUDA_HOME=/usr/local/cuda" >&2
  fi
fi

"$VENV_PY" -c "import torch; print('torch', torch.__version__, 'cuda', torch.version.cuda)" 2>/dev/null || {
  echo "[ERROR] PyTorch não encontrado no Python do venv." >&2
  exit 1
}

export TORCH_CUDA_ARCH_LIST="${TORCH_CUDA_ARCH_LIST:-8.9}"
cd "$CR_DIR"
echo "[STEP] pip install -e . --no-build-isolation (CUDA_HOME=${CUDA_HOME:-})"
"$VENV_PY" -m pip install -e . --no-build-isolation

echo "[STEP] Verificação (importa torch primeiro)..."
"$VENV_PY" -c "import torch; import custom_rasterizer_kernel; print('OK: custom_rasterizer')"

echo "[done] Hunyuan3D-Paint: text3d texture / generate --final"
