#!/bin/bash

# Text3D - Setup Script
# Cria ambiente virtual e instala dependências otimizadas para GPUs de 6GB

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_NAME="text3d"
VENV_DIR="$SCRIPT_DIR/.venv"
PYTHON_CMD="${PYTHON_CMD:-python3}"

echo "=========================================="
echo "  Text3D - Setup de Ambiente"
echo "=========================================="
echo ""

# Cores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Função de log
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Verificar Python
log_info "Verificando Python..."
if ! command -v $PYTHON_CMD &> /dev/null; then
    log_error "Python não encontrado. Instale Python 3.8+"
    exit 1
fi

PYTHON_VERSION=$($PYTHON_CMD --version 2>&1 | cut -d' ' -f2)
log_info "Python encontrado: $PYTHON_VERSION"

# Verificar versão mínima
REQUIRED_VERSION="3.8"
VERSION_OK=$($PYTHON_CMD -c "import sys; major, minor = sys.version_info[:2]; print('OK' if (major, minor) >= (3, 8) else 'FAIL')")
if [ "$VERSION_OK" != "OK" ]; then
    log_error "Python $REQUIRED_VERSION+ necessário. Versão atual: $PYTHON_VERSION"
    exit 1
fi

# Verificar CUDA (opcional mas recomendado)
log_info "Verificando CUDA..."
if command -v nvidia-smi &> /dev/null; then
    log_info "CUDA detectado:"
    nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null | while read line; do
        echo "  - GPU: $line"
    done
else
    log_warn "CUDA não detectado. O projeto funcionará em CPU (mais lento)"
fi

# Criar ambiente virtual
if [ -d "$VENV_DIR" ]; then
    log_warn "Ambiente virtual já existe em $VENV_DIR"
    log_info "Removendo ambiente antigo..."
    rm -rf "$VENV_DIR"
fi

if [ ! -d "$VENV_DIR" ]; then
    log_info "Criando ambiente virtual em $VENV_DIR..."
    $PYTHON_CMD -m venv "$VENV_DIR"
fi

# Ativar ambiente
log_info "Ativando ambiente virtual..."
source "$VENV_DIR/bin/activate"

# Atualizar pip
log_info "Atualizando pip..."
pip install --upgrade pip setuptools wheel

# Instalar PyTorch (versão otimizada para CUDA 11.8 ou 12.1)
log_info "Instalando PyTorch otimizado..."

# Detectar versão CUDA e instalar PyTorch adequado
if command -v nvidia-smi &> /dev/null; then
    CUDA_VERSION=$(nvidia-smi | grep "CUDA Version" | sed 's/.*CUDA Version: \([0-9]*\.[0-9]*\).*/\1/')
    log_info "CUDA Version detectada: $CUDA_VERSION"
    
    if [[ "$CUDA_VERSION" == 12* ]]; then
        log_info "Instalando PyTorch para CUDA 12.1..."
        pip install torch torchvision --index-url https://download.pytorch.org/whl/cu121
    else
        log_info "Instalando PyTorch para CUDA 11.8..."
        pip install torch torchvision --index-url https://download.pytorch.org/whl/cu118
    fi
else
    log_warn "Instalando PyTorch para CPU (sem CUDA)..."
    pip install torch torchvision --index-url https://download.pytorch.org/whl/cpu
fi

# Instalar dependências principais
log_info "Instalando dependências do projeto..."
pip install -r "$PROJECT_ROOT/config/requirements.txt"

# Verificar instalação
log_info "Verificando instalação..."
python -c "import torch; print(f'PyTorch: {torch.__version__}')"
python -c "import diffusers; print(f'Diffusers: {diffusers.__version__}')"
python -c "import transformers; print(f'Transformers: {transformers.__version__}')"

# Baixar modelos com HF CLI
log_info "Verificando Hugging Face CLI..."
if command -v huggingface-cli &> /dev/null; then
    log_info "HF CLI detectado. Verificando login..."
    
    # Verificar se está logado
    if huggingface-cli whoami &> /dev/null; then
        USER_NAME=$(huggingface-cli whoami 2>/dev/null | head -1)
        log_info "Logado como: $USER_NAME"
        
        # Download dos modelos
        echo ""
        log_info "Baixando modelos (isso pode levar alguns minutos)..."
        
        # Modelo principal Shap-E
        log_info "Baixando openai/shap-e..."
        huggingface-cli download openai/shap-e \
            --local-dir "$PROJECT_ROOT/models/shap-e" \
            --local-dir-use-symlinks False \
            --resume-download || log_warn "Falha no download do shap-e (tentará novamente na primeira execução)"
        
        # Modelo img2img
        log_info "Baixando openai/shap-e-img2img..."
        huggingface-cli download openai/shap-e-img2img \
            --local-dir "$SCRIPT_DIR/models/shap-e-img2img" \
            --local-dir-use-symlinks False \
            --resume-download || log_warn "Falha no download do shap-e-img2img (tentará novamente na primeira execução)"
        
        log_info "Download dos modelos concluído!"
    else
        log_warn "Não logado no Hugging Face."
        log_info "O modelo Shap-E requer aceitação de termos em: https://huggingface.co/openai/shap-e"
        log_info "Execute 'huggingface-cli login' após o setup para baixar modelos."
        log_info "Os modelos serão baixados automaticamente na primeira execução."
    fi
else
    log_warn "HF CLI não encontrado. Instale com: pip install huggingface-hub[cli]"
    log_warn "Os modelos serão baixados automaticamente na primeira execução."
fi

# Criar diretórios de saída
log_info "Criando diretórios de saída..."
mkdir -p "$SCRIPT_DIR/outputs"/{meshes,gifs,images}

# Criar script de ativação
log_info "Criando scripts de conveniência..."
cat > "$SCRIPT_DIR/activate.sh" << 'EOF'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/.venv/bin/activate"
exec "$@"
EOF
chmod +x "$SCRIPT_DIR/activate.sh"

# Criar script text3d
pip install -e "$SCRIPT_DIR" 2>/dev/null || log_warn "Instalação em modo editable falhou (ignorando)"

echo ""
echo "=========================================="
echo -e "${GREEN}  Setup concluído com sucesso!${NC}"
echo "=========================================="
echo ""
echo "Para ativar o ambiente:"
echo "  source .venv/bin/activate"
echo ""
echo "Ou use o script de conveniência:"
echo "  ./activate.sh text3d --help"
echo ""
echo "Para gerar seu primeiro modelo 3D:"
echo "  text3d generate 'um robo futuristico' --output meu_robo.glb"
echo ""
echo "Ver VRAM disponível:"
echo "  nvidia-smi"
echo ""
