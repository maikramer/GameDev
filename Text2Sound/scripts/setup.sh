#!/usr/bin/env bash
# Conveniência para dev: venv + editable install. Instalação oficial: ../../docs/INSTALLING.md (`./install.sh text2sound` na raiz GameDev).
#
# Text2Sound — setup rápido (venv + dependências + editable install)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Text2Sound — Setup ==="
echo "Projeto: $PROJECT_ROOT"

# Python
PYTHON="${PYTHON_CMD:-python3}"
echo "Python: $($PYTHON --version 2>&1)"

# Venv
VENV_DIR="$PROJECT_ROOT/.venv"
if [ ! -d "$VENV_DIR" ]; then
    echo "Criando venv em $VENV_DIR..."
    $PYTHON -m venv "$VENV_DIR"
fi

source "$VENV_DIR/bin/activate"
echo "Venv ativado: $VENV_DIR"

# Dependências base
echo "Atualizando pip/setuptools/wheel..."
pip install --upgrade pip "setuptools>=68,<82" wheel -q

# stable-audio-tools fixa pandas==2.0.2 que não compila em Python 3.13+.
# Instalamos pandas moderno primeiro, depois stable-audio-tools --no-deps,
# e por fim as restantes dependências.
echo "Instalando pandas (compatível com Python 3.13+)..."
pip install "pandas>=2.1.0" -q

echo "Instalando stable-audio-tools (sem resolver deps pinadas)..."
pip install stable-audio-tools --no-deps -q

echo "Instalando dependências de inferência..."
pip install torch torchaudio einops soundfile -q
pip install alias-free-torch auraloss descript-audio-codec einops-exts \
    ema-pytorch encodec huggingface-hub importlib-resources k-diffusion \
    laion-clap local-attention sentencepiece vector-quantize-pytorch \
    v-diffusion-pytorch -q

echo "Instalando CLI deps..."
pip install click rich rich-click -q

# Shared (editable, no-deps — deps já instaladas acima)
echo "Instalando gamedev-shared..."
pip install -e "$PROJECT_ROOT/../Shared" --no-deps -q

# Editable install
echo "Instalando Text2Sound (editable)..."
pip install -e "$PROJECT_ROOT" --no-deps -q

# Dev deps
pip install pytest pytest-cov -q

echo ""
echo "=== Text2Sound instalado com sucesso! ==="
echo "Ativar venv: source $VENV_DIR/bin/activate"
echo "Uso: text2sound --help"
echo "Testes: python -m pytest tests/ -v"
