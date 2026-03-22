# Funcionalidades

## Geração de Mapas PBR

### Height Map

Converte uma imagem colorida em um mapa de altura em escala de cinza.

**Entrada:** Imagem difusa (RGBA)
**Saída:** Height map (grayscale, R32Float internamente)

**Algoritmo:**
1. Converter para luminância (grayscale)
2. Aplicar Gaussian blur em múltiplos níveis
3. Combinar com pesos ajustáveis
4. Aplicar contraste e spread

**Uso típico:**
- Displacement mapping
- Parallax occlusion mapping
- Base para normal map generation

### Normal Map

Gera vetores de superfície a partir do height map.

**Entrada:** Height map (gerado internamente)
**Saída:** Normal map (RGB, formato DirectX/OpenGL)

**Algoritmo:**
1. Calcular gradientes via operador Sobel
2. Construir vetores normais (x, y, z)
3. Normalizar e codificar para [0, 255]

**Formato de saída:**
- Red channel: X component (-1 to +1) → 0 to 255
- Green channel: Y component (-1 to +1) → 0 to 255  
- Blue channel: Z component (0 to +1) → 128 to 255

**Uso típico:**
- Iluminação em tempo real
- Bump mapping
- Detalhes de superfície sem geometria adicional

### Metallic Map

Detecta áreas metálicas por análise de cor.

**Entrada:** Imagem difusa original (RGBA)
**Saída:** Metallic map (grayscale, 0 = dieletric, 1 = metal)

**Algoritmo:**
1. Converter RGB para espaço HSL
2. Analisar saturação e luminância
3. Aplicar heurística de detecção metálica

**Heurística:**
- Metais puros têm saturação baixa e luminância alta
- Ouro/cobre têm matiz específico
- Thresholds adaptativos baseados na imagem

**Uso típico:**
- Physically based rendering (PBR)
- Diferenciação metal/não-metal
- Reflexos específicos por material

### Smoothness Map

Define rugosidade/suavidade da superfície para PBR.

**Entrada:** Imagem difusa + mapa metallic (gerados internamente)
**Saída:** Smoothness map (grayscale, 0 = rugoso, 1 = liso)

**Algoritmo:** Base smoothness (0.25) + contribuição do metallic (0.65 × metallic). Metais tendem a ser mais lisos.

**Uso típico:** Roughness/smoothness em shaders PBR (Unity, Unreal, etc.).

### Edge Map

Destaca bordas e vincos a partir do mapa de normal.

**Entrada:** Normal map (gerado internamente)
**Saída:** Edge map (grayscale)

**Algoritmo:** Gradiente da normal (amostras ±1 pixel em X e Y), combinado com contraste. Inspirado no Materialize original (Blit_Edge_From_Normal).

**Uso típico:** Outline, cavity, ou máscaras para pós-processamento.

### AO Map (Ambient Occlusion)

Oclusão ambiente no estilo cavity, a partir do height map.

**Entrada:** Height map (gerado internamente)
**Saída:** AO map (grayscale, 0 = ocluído, 1 = aberto)

**Algoritmo:** Amostras em 8 direções (raios 1 e 2 pixels); oclusão quando altura da amostra > centro; resultado invertido e escalado.

**Uso típico:** Sombreamento em frestas e cantos em pipelines PBR.

## Formatos Suportados

### Entrada (Leitura)

| Formato | Extensões | Observações |
|---------|-----------|-------------|
| PNG | .png | Recomendado, lossless |
| JPEG | .jpg, .jpeg | Lossy, bom para fotos |
| TGA | .tga | Legacy, games |
| BMP | .bmp | Limitado, suportado |
| EXR | .exr | HDR, linear |

### Saída (Escrita)

| Formato | Extensões | Melhor uso |
|---------|-----------|------------|
| PNG | .png | Geral (lossless) |
| JPEG | .jpg | Compacto (lossy) |
| TGA | .tga | Games engines |
| EXR | .exr | Normal maps (float precision) |

**Configurações de qualidade:**
- PNG: Compression level 6 (padrão)
- JPEG: Qualidade 95% (padrão), configurável 0-100
- EXR: P-tiles, zip compression

## Interface CLI

### Comando Básico

```bash
materialize <INPUT> [OPTIONS]
```

### Opções

| Opção | Curta | Descrição | Padrão |
|-------|-------|-----------|--------|
| `--output` | `-o` | Diretório de saída | `.` |
| `--format` | `-f` | Formato de saída (png, jpg, tga, exr) | png |
| `--quality` | `-q` | Qualidade JPEG (0-100) | 95 |
| `--verbose` | `-v` | Modo verbose | false |
| `--quiet` | | Não listar arquivos gerados no sucesso | false |
| `--help` | `-h` | Mostrar ajuda | - |
| `--version` | `-V` | Mostrar versão | - |

### Exemplos de Uso

```bash
# Básico - gera na mesma pasta
materialize texture.png
# Resultado: texture_height.png, texture_normal.png, texture_metallic.png,
#           texture_smoothness.png, texture_edge.png, texture_ao.png

# Diretório de saída específico
materialize texture.png -o ./materials/

# Formato diferente
materialize texture.png -f exr

# JPEG com qualidade baixa (mais compacto)
materialize texture.jpg -f jpg -q 80

# Verbose - mostra progresso; --quiet suprime a lista de arquivos
materialize texture.png -v
materialize texture.png --quiet

# Instalar a skill do Cursor no projeto atual
materialize skill install
```

## Features Futuras (Roadmap)

### Versão 1.1 - Batch Processing

- **Diretório como input:** `materialize ./textures/`
- **Paralelização:** Processa múltiplas imagens simultaneamente
- **Progress bar:** Indicação visual de progresso

### Versão 2.0 - Configuração Avançada

- **Arquivo de configuração:** `materialize.toml` para parâmetros customizados
- **Override por mapa:** `--height-blur=5.0 --normal-intensity=2.5`

### Versão 3.0 - Preview

- **Janela de preview:** SDL2-based, visualização 3D rápida
- **Rotação/Pan/Zoom:** Navegação básica

## Casos de Uso

### Game Development

```bash
# Pipeline de assets
for texture in assets/textures/raw/*.png; do
    materialize "$texture" -o assets/textures/processed/
done
```

### 3D Art / Blender

```bash
# Preparar textura para importação
materialize photo.jpg -f exr -o ~/blender_project/textures/
# Importar height para displacement, normal para bump, metallic para shader
```

### Web Development (Three.js/Babylon)

```bash
# Otimizar para web (JPEG compacto)
materialize texture.png -f jpg -q 85 -o ./public/textures/
```

### Archviz

```bash
# Materiais de alta qualidade
materialize marble_scan.png -f exr -o ./materials/marble/
# EXR preserva precisão de cor para materiais PBR
```

## Limitações Conhecidas

### MVP (Versão 1.0)

1. **Parâmetros fixos:** Defaults hardcoded (smoothness base/metal, AO depth scale, etc.), sem ajuste fino
2. **Resolução máxima:** Limitada por GPU memory (tipicamente 8K+)
3. **Um formato por execução:** Todos os seis mapas no mesmo formato
4. **Sem alpha handling:** Canal alpha ignorado no processamento
5. **AO simplificado:** Cavity-style a partir do height; o Materialize original usa ray marching com normal+height

### Futuro

Limitações serão endereçadas conforme roadmap (configuração, tiled processing, etc.)
