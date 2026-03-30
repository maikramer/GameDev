#!/usr/bin/env bash
# Compila e instala custom_rasterizer (Hunyuan3D-Paint 2.1 / hy3dpaint) no venv ativo.
#
# Uso (a partir da raiz Paint3D):
#   source .venv/bin/activate
#   bash scripts/install_custom_rasterizer.sh
#
# Requisitos: nvcc (CUDA Toolkit), PyTorch CUDA no venv.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -n "${VIRTUAL_ENV:-}" ]]; then
  VENV_PY="$VIRTUAL_ENV/bin/python"
elif [[ -x "$ROOT/.venv/bin/python" ]]; then
  VENV_PY="$ROOT/.venv/bin/python"
  echo "[INFO] Usando $ROOT/.venv"
else
  echo "[ERROR] Ativa o venv do Paint3D ou define VIRTUAL_ENV." >&2
  exit 1
fi

# Autodetectar CUDA_HOME
if [[ -z "${CUDA_HOME:-}" ]]; then
  for candidate in /usr/local/cuda-* /usr/local/cuda /usr/lib/cuda; do
    if [[ -x "$candidate/bin/nvcc" ]]; then
      export CUDA_HOME="$candidate"
      echo "[INFO] CUDA_HOME autodetectado: $CUDA_HOME"
      break
    fi
  done
fi

if ! command -v nvcc &>/dev/null; then
  if [[ -n "${CUDA_HOME:-}" && -x "$CUDA_HOME/bin/nvcc" ]]; then
    export PATH="$CUDA_HOME/bin:$PATH"
  else
    echo "[ERROR] nvcc não encontrado. Em Ubuntu: sudo apt install nvidia-cuda-toolkit nvidia-cuda-dev" >&2
    exit 1
  fi
fi

MONOREPO="$(cd "$ROOT/.." && pwd)"
CR_DIR="${HUNYUAN3D_21_CUSTOM_RASTER:-$MONOREPO/third_party/Hunyuan3D-2.1/hy3dpaint/custom_rasterizer}"

if [[ ! -d "$CR_DIR" ]]; then
  echo "[ERROR] custom_rasterizer não encontrado: $CR_DIR" >&2
  echo "  Na raiz do monorepo: git submodule update --init third_party/Hunyuan3D-2.1" >&2
  echo "  Ou define HUNYUAN3D_21_CUSTOM_RASTER para hy3dpaint/custom_rasterizer" >&2
  exit 1
fi

"$VENV_PY" -c "import torch; print('torch', torch.__version__, 'cuda', torch.version.cuda)" 2>/dev/null || {
  echo "[ERROR] PyTorch não encontrado no Python do venv." >&2
  exit 1
}

cd "$CR_DIR"
echo "[STEP] pip install -e . --no-build-isolation (CUDA_HOME=${CUDA_HOME:-})"
"$VENV_PY" -m pip install -e . --no-build-isolation

echo "[STEP] Verificação..."
"$VENV_PY" -c "import torch; import custom_rasterizer; print('OK: custom_rasterizer')"

echo "[done] Pipeline padrão com textura: paint3d texture mesh.glb -i ref.png -o out.glb"
