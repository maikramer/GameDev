# CLI API Reference

## Sintaxe

```
materialize [OPTIONS] [INPUT] [COMMAND]
```

**Subcomandos:** `skill install` — instala a skill do materialize-cli em `.cursor/skills/materialize-cli` do diretório atual (projeto em que você está trabalhando).

## Argumentos

### Posicionais

| Argumento | Obrigatório | Descrição |
|-----------|-------------|-----------|
| `INPUT` | Sim | Caminho para a imagem de entrada |

### Flags e Options

| Opção | Curta | Tipo | Padrão | Descrição |
|-------|-------|------|--------|-----------|
| `--output` | `-o` | String | `./` | Diretório de saída |
| `--format` | `-f` | Enum | `png` | Formato dos arquivos de saída |
| `--preset` | `-p` | Enum | `default` | Preset de material (ver seção Presets) |
| `--quality` | `-q` | Int (0-100) | `95` | Qualidade JPEG (quando aplicável) |
| `--verbose` | `-v` | Bool | `false` | Modo verbose |
| `--quiet` | | Bool | `false` | Não listar arquivos gerados no sucesso |
| `--help` | `-h` | | | Mostrar ajuda |
| `--version` | `-V` | | | Mostrar versão |

## Enums

### Preset de Material (`--preset`)

| Valor | Descrição | Uso ideal |
|-------|-----------|-----------|
| `default` | Configurações balanceadas | Qualquer textura (comportamento original) |
| `skin` | Metallic zero, smoothness médio-alto, normals suaves | Pele humana, personagens |
| `floor` | Height pronunciado, AO forte, superfície áspera | Chão, piso, azulejo, terra |
| `metal` | Metallic boost, edges nítidos, polido | Aço, ouro, cobre, alumínio |
| `fabric` | Matte, sem metallic, edges suaves | Tecido, roupa, cortina |
| `wood` | Sem metallic, detalhe de grão moderado | Madeira, móveis, piso de madeira |
| `stone` | Muito áspero, AO profundo, normals fortes | Pedra, rocha, concreto |

### Formato de Saída (`--format`)

| Valor | Extensão | Características |
|-------|----------|-----------------|
| `png` | .png | Lossless, bom geral |
| `jpg` | .jpg | Lossy, compacto |
| `jpeg` | .jpeg | Alias para jpg |
| `tga` | .tga | Uncompressed, games |
| `exr` | .exr | HDR, alta precisão |

## Convenção de Nomenclatura

### Padrão

Input: `texture.png`

Output:
- `texture_height.png`
- `texture_normal.png`
- `texture_metallic.png`
- `texture_smoothness.png`
- `texture_edge.png`
- `texture_ao.png`

## Códigos de Saída

| Código | Significado |
|--------|-------------|
| `0` | Sucesso |
| `1` | Erro genérico |
| `2` | Input file não encontrado |
| `3` | Formato de input não suportado |
| `4` | Erro de GPU (adapter não encontrado) |
| `5` | Erro de I/O (permissão, disco cheio, etc.) |
| `6` | Imagem muito grande para GPU |

## Mensagens de Erro

### Input não encontrado

```
Error: Input file 'texture.png' not found
```

### Formato não suportado

```
Error: Unsupported image format 'texture.bmp'
       Supported formats: png, jpg, tga, exr
```

### GPU não disponível

```
Error: No GPU adapter available
       Ensure you have Vulkan (Linux), Metal (macOS), or DirectX 12 (Windows) drivers installed
```

### Out of memory

```
Error: Image too large (16384x16384 requires 2GB GPU memory)
       Try using a smaller image or enabling tiled processing (--tiled)
```

## Modo Verbose

Quando `-v` ou `--verbose` é usado, o CLI imprime informações de progresso:

```bash
$ materialize texture.png -v
[1/5] Loading texture.png... 2048x2048 RGBA8 (16.7 MB)
[2/5] Initializing GPU... Vulkan adapter: NVIDIA GeForce RTX 3060
[3/5] Processing height map... done (45ms)
[4/5] Processing normal map... done (12ms)
[4/5] Processing metallic map... done (18ms)
[5/5] Saving outputs... done

Output files:
  - texture_height.png (2048x2048, 4.2 MB)
  - texture_normal.png (2048x2048, 12.5 MB)
  - texture_metallic.png (2048x2048, 1.1 MB)

Total time: 89ms
```

## Exemplos Completos

### Exemplo 1: Uso básico

```bash
materialize brick.png
```

Gera na pasta atual os seis mapas (height, normal, metallic, smoothness, edge, ao).

### Exemplo 2: Diretório de saída com preset

```bash
materialize brick.png -o ./materials/brick/ -p floor
```

Gera em `./materials/brick/` os seis mapas otimizados para textura de chão.

### Exemplo 3: Textura de personagem

```bash
materialize character_skin.png -p skin -o ./character/
```

Gera mapas otimizados para pele humana (sem metallic, normals suaves).

### Exemplo 4: Pipeline em script

```bash
#!/bin/bash

INPUT_DIR="./raw_textures"
OUTPUT_DIR="./processed"

mkdir -p "$OUTPUT_DIR"

for file in "$INPUT_DIR"/*.png; do
    name=$(basename "$file" .png)
    echo "Processing $name..."
    materialize "$file" -o "$OUTPUT_DIR/$name/" -p "$name"
done

echo "Done! Processed $(ls "$INPUT_DIR"/*.png | wc -l) textures"
```

### Exemplo 4: Formato específico por tipo de mapa

(Nota: Versão futura, não suportado em MVP)

```bash
# Height em EXR (precisão), outros em PNG
materialize texture.png --height-format=exr --normal-format=png --metallic-format=png
```

### Exemplo 5: Batch com paralelismo

```bash
# Processar 4 imagens simultaneamente
ls *.png | xargs -P 4 -I {} materialize {} -o ./output/
```

## Integração com Scripts

### Verificação de sucesso

```bash
if materialize texture.png; then
    echo "Success!"
else
    echo "Failed with exit code $?"
fi
```

### Captura de output

```bash
# Com --quiet não imprime a lista; sem --quiet imprime "Generated:" e os 6 paths
materialize texture.png -o ./out/
materialize texture.png -o ./out/ --quiet
```

## Variáveis de Ambiente

| Variável | Descrição |
|----------|-----------|
| `MATERIALIZE_GPU_BACKEND` | Forçar backend: `vulkan`, `metal`, `dx12` |
| `MATERIALIZE_LOG` | Nível de log: `error`, `warn`, `info`, `debug`, `trace` |
| `WGPU_BACKEND` | Backend wgpu (herdado da lib) |

### Exemplo

```bash
MATERIALIZE_GPU_BACKEND=vulkan materialize texture.png
MATERIALIZE_LOG=debug materialize texture.png -v
```

## Auto-completion

### Bash

```bash
materialize --generate-completions bash > /etc/bash_completion.d/materialize
```

### Zsh

```bash
materialize --generate-completions zsh > "${fpath[1]}/_materialize"
```

### Fish

```bash
materialize --generate-completions fish > ~/.config/fish/completions/materialize.fish
```
