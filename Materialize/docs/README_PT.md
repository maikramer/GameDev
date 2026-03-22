# Materialize CLI

CLI minimalista em Rust que converte imagens em materiais PBR (Physically Based Rendering) usando compute shaders GPU via wgpu.

## Visão Geral

Materialize CLI é uma ferramenta de linha de comando que transforma uma imagem difusa em seis mapas PBR:

- **Height Map** - Representação de elevação da superfície
- **Normal Map** - Vetores de superfície para iluminação
- **Metallic Map** - Máscara de metalicidade
- **Smoothness Map** - Rugosidade/suavidade (base + contribuição metálica)
- **Edge Map** - Detecção de bordas a partir da normal
- **AO Map** - Ambient Occlusion (cavity-style a partir do height)

Baseado no [Materialize original](https://github.com/BoundingBoxSoftware/Materialize) (Unity/Windows), esta versão CLI é:

- **Minimalista** - Sem UI, sem Unity, apenas linha de comando
- **Rápida** - Processamento GPU via compute shaders
- **Cross-platform** - Linux, Windows, macOS via wgpu
- **Direta** - Um comando, múltiplas saídas

## Instalação

### Via instalador (recomendado, estilo denv/galaxy)

Requisitos: Python 3 (para o instalador), Rust/cargo (para compilar).

```bash
# Clone o repositório
git clone https://github.com/maikramer/Materialize-CLI
cd Materialize-CLI

# Linux/macOS
./install.sh

# Ou diretamente com Python
python3 installer/installer.py install

# Desinstalar
./install.sh uninstall
# ou
python3 installer/installer.py uninstall
```

O instalador compila com `cargo build --release` e copia o binário para `~/.local/bin/materialize`. Garanta que `~/.local/bin` está no seu PATH.

**Windows:** use `install.ps1` (PowerShell) ou `install.bat` (CMD).

### Via Cargo (manual)

```bash
cargo build --release
cargo install --path .
```

## Uso Básico

```bash
# Gerar todos os mapas a partir de uma textura
materialize texture.png

# Saída gerada:
# - texture_height.png, texture_normal.png, texture_metallic.png
# - texture_smoothness.png, texture_edge.png, texture_ao.png

# Especificar diretório de saída
materialize texture.png -o ./output/

# Escolher formato de saída
materialize texture.png --format exr
```

## Documentação

- [Arquitetura](architecture.md) - Estrutura e componentes do sistema
- [Funcionalidades](features.md) - Recursos e capacidades
- [CLI API](cli-api.md) - Interface de linha de comando
- [Algoritmos](algorithms.md) - Detalhes dos algoritmos de processamento
- [Shaders](shaders.md) - Documentação dos shaders WGSL
- [Roadmap](roadmap.md) - Desenvolvimento futuro

## Requisitos

- Rust 1.75+
- GPU com suporte a Vulkan (Linux), Metal (macOS), ou DirectX 12 (Windows)
- Drivers atualizados

## Licença

MIT - Baseado no Materialize original por Bounding Box Software
