# Roadmap

## Versão 1.0 (MVP) - Atual

**Status:** Estável

### Features

- [x] Height map generation via multi-level blur
- [x] Normal map from height via Sobel operator
- [x] Metallic map via HSL analysis
- [x] Smoothness map (base + metallic contribution)
- [x] Edge map from normal gradient
- [x] AO map (cavity-style from height)
- [x] CLI interface (clap: input, -o, -f, -q, -v, --quiet)
- [x] wgpu compute shaders (incl. pipeline 2 inputs para smoothness)
- [x] PNG/JPG/TGA/EXR support

### Limitações Conhecidas

- Parâmetros hardcoded (sem ajuste fino)
- Sem configuração via arquivo
- Um formato de saída por execução
- Resolução limitada por GPU memory
- Sem alpha handling

---

## Versão 1.1 - Parâmetros Inline

**Timeline:** 1-2 semanas após MVP

### Novos Features

#### Parâmetros Inline

- **Override de defaults:**
  ```bash
  materialize texture.png --height-blur=5.0 --normal-intensity=2.0
  ```

- **Configuração por mapa:**
  ```bash
  materialize texture.png \
    --height-contrast=1.8 \
    --height-levels=5 \
    --normal-flip-y \
    --metallic-saturation-threshold=0.2
  ```

### API Preview

```bash
materialize texture.png --help

Options:
  --height-blur <FLOAT>         Sigma máximo do blur [default: 64.0]
  --height-contrast <FLOAT>     Fator de contraste [default: 1.5]
  --height-levels <INT>         Número de níveis de blur [default: 7]
  --normal-intensity <FLOAT>    Escala dos gradientes [default: 1.0]
  --normal-flip-y               Flip Y para OpenGL
  --metallic-saturation <FLOAT>  Threshold de saturação [default: 0.15]
  --metallic-luminance <FLOAT>  Threshold de luminância [default: 0.4]
  --smoothness                  Incluir smoothness map
```

---

## Versão 1.2 - Batch Processing

**Timeline:** 2-3 semanas após v1.1

### Features

#### Processamento de Diretórios

```bash
# Processar pasta inteira
materialize ./textures/ -o ./output/

# Padrão glob
materialize "./textures/**/*.png" -o ./output/

# Estrutura preservada
# Input:  ./textures/bricks/red_brick.png
# Output: ./output/bricks/red_brick_height.png, etc.
```

#### Paralelização

- Processa N imagens simultaneamente (configurável)
- Reusa GPU context entre imagens
- Progress bar com indicador de fila

```bash
materialize ./textures/ --jobs=4 --progress

# Output:
# [1/50] processing stone.png... done (120ms)
# [2/50] processing brick.png... done (98ms)
# [=====>              ] 2/50 (ETA: 4.8s)
```

#### Resume/Continue

```bash
# Se interrompido, resume do ponto onde parou
materialize ./textures/ --resume

# Skip existentes (útil para atualizações)
materialize ./textures/ --skip-existing
```

---

## Versão 2.0 - AO Avançado (opcional)

**Timeline:** Após v1.2 (opcional)

AO cavity-style já está no MVP. Esta versão adicionaria AO por ray marching (normal+height) como no Materialize original, com parâmetros (ray count, spread, depth). Curvature map também considerado.

---

## Versão 2.1 - Configuração Avançada

**Timeline:** 2-3 semanas após v2.0

### Config Files (TOML)

**Arquivo padrão:** `materialize.toml` no diretório do input

```toml
# materialize.toml
[global]
output_format = "exr"
output_dir = "./processed"

[height]
blur_levels = 7
max_sigma = 64.0
contrast = 1.5

[normal]
intensity = 1.0
flip_y = false

[metallic]
saturation_threshold = 0.15
luminance_threshold = 0.4

[ao]
enabled = true
ray_count = 64
max_distance = 0.5
```

**Uso:**
```bash
# Auto-detecta materialize.toml
materialize texture.png

# Especifica arquivo
materialize texture.png --config=./my-config.toml

# Override inline
materialize texture.png --config=./base.toml --height-contrast=2.0
```

### Profiles

Profiles pré-definidos para casos comuns:

```bash
# Profile para tijolos
materialize brick.png --profile=brick

# Profile para metal
materialize metal.png --profile=metal

# Profile para pele/orgânico
materialize skin.png --profile=organic

# Lista profiles
materialize --list-profiles
```

**Built-in profiles:**
- `default`: Configurações equilibradas
- `brick`: Blur mais forte para padrões de tijolo
- `metal`: Thresholds ajustados para metais
- `organic`: Suavização extra para superfícies naturais
- `tile`: Configurado para texturas tileáveis

---

## Versão 3.0 - Preview Window

**Timeline:** 1-2 meses após v2.1

### Feature Principal: Preview 3D

**Descrição:** Janela SDL2/GLFW para preview rápido do material PBR completo

**Comando:**
```bash
materialize texture.png --preview

# Preview apenas após processamento
materialize texture.png && materialize texture.png --preview-only
```

**Controles:**
- `Left click + drag`: Rotacionar
- `Right click + drag`: Pan
- `Scroll`: Zoom
- `H/N/M/S`: Toggle mapas (Height/Normal/Metallic/Smoothness)
- `Space`: Toggle wireframe
- `Esc`: Fechar

### Shader de Preview

Pipeline de preview simples:
```
Diffuse + Normal + Metallic + Smoothness + AO → PBR shading
```

**Iluminação:**
- 3 point lights (key, fill, rim)
- Cubemap environment (opcional)
- Rotation automática

---

## Versão 3.1 - Seamless/Tiling

**Timeline:** 1 mês após v3.0

### Features

#### Seamless Texture Maker

Converte textura não-tileável em tileável:

```bash
materialize texture.png --make-seamless --output=seamless.png
```

**Algoritmo:**
1. Wrap edges com blending
2. Frequency analysis para patterns
3. Poisson blending nas junções

#### Tiling Preview

```bash
materialize texture.png --preview --tiling=2x2  # Mostra 2x2 tiles
materialize texture.png --preview --tiling=4x4  # Mostra 4x4 tiles
```

#### Seamless Maps

Todos os mapas gerados são automaticamente seamless se input for:
- Height: Wrapping com derivadas consistentes
- Normal: Wrapping com continuidade
- Metallic: Simples wrap (não afeta vizinhança)

---

## Versão 4.0 - Advanced Algorithms

**Timeline:** 2-3 meses após v3.1

### Machine Learning

#### ML-Based Metallic Detection

- Modelo treinado em dataset de materiais PBR
- Melhor detecção que heurísticas HSL
- Suporte para metais pintados/oxidados

```bash
materialize texture.png --metallic-ml --model=./my-model.onnx
```

#### Super-Resolution

Upscale + geração de mapas simultâneo:

```bash
materialize lowres.png --upscale=2x  # 1K → 2K
materialize lowres.png --upscale=4x  # 1K → 4K
```

### Advanced Height

#### Machine Learning Height

Extrai height com ML (melhor que luminância):

```bash
materialize texture.png --height-ml
```

#### Depth-from-Defocus (DfD)

Se múltiplas imagens com diferentes focos disponíveis:

```bash
materialize --dfd ./focus_stack/ --output=height.png
```

---

## Versão 5.0 - Plugin System

**Timeline:** 3-6 meses após v4.0

### Plugin Architecture

Plugins em Rust (dynamic libs) ou Lua/Python scripts:

```bash
# Carregar plugin
materialize texture.png --plugin=./my-plugin.so

# Plugins podem:
# - Adicionar novos mapas
# - Modificar pipeline existente
# - Adicionar novos algoritmos
```

### Marketplace de Plugins

```bash
# Instalar plugin do registry
materialize plugin install normal-enhancer

# Listar plugins
materialize plugin list

# Desenvolver plugin
materialize plugin new my-plugin  # Gera template
```

---

## Versões Futuras (Sem Timeline)

### Features Consideradas

- [ ] **CLI Server Mode:** `materialize serve` para processamento via API HTTP
- [ ] **Watch Mode:** `materialize --watch ./textures/` - re-processa em mudanças
- [ ] **GUI Mode:** Interface gráfica opcional (egui/iced)
- [ ] **Cloud Processing:** Offload para GPUs cloud
- [ ] **Batch Config:** Processar múltiplas configs em uma execução
- [ ] **Image Sequence:** Processar vídeos/texturas animadas
- [ ] **Normal Map Combine:** Combinar múltiplas normais (detail mapping)
- [ ] **Curvature-Driven:** Ajustar parâmetros baseado em curvatura local

---

## Prioridades

### Prioridade Alta (Must Have)

1. MVP funcional e estável
2. Batch processing (essencial para pipelines)
3. Config files (usabilidade)

### Prioridade Média (Should Have)

4. AO (diferencial do Materialize original)
5. Preview window (UX)
6. Smoothness (completa o set PBR básico)

### Prioridade Baixa (Nice to Have)

7. Seamless maker
8. ML features
9. Plugin system
10. GUI mode

---

## Contribuições

Features da comunidade são bem-vindas! Abra uma issue para discutir:

- Novos mapas/algoritmos
- Integrações com engines
- Performance improvements
- Bug fixes

**Label `good-first-issue`:** Issues ideais para novos contribuidores
