# Materialize CLI

*Versão em português. [English README](README.md).*

Uma **CLI em Rust** que gera mapas PBR (renderização fisicamente baseada) a partir de texturas difusas, usando *compute shaders* na GPU via [wgpu](https://wgpu.rs/). Sem interface gráfica, sem Unity — um comando e seis mapas.

Inspirada no [Materialize](https://github.com/BoundingBoxSoftware/Materialize) original (Unity/Windows).

**Para quem é?** Desenvolvedores de jogos, artistas 3D e quem precisa de mapas PBR a partir de texturas difusas — em motores como Unity, Unreal, Godot ou no Blender, sem abrir uma GUI nem depender do original só para Windows.

**Monorepo GameDev:** o **Hunyuan3D-Paint 2.1** (`paint3d texture`) já produz **GLB PBR**. Esta CLI serve para **mapas PBR a partir de uma imagem difusa** (ex.: **Texture2D** + `texture2d.materialize` no GameAssets). Ver [`Text3D/docs/PBR_MATERIALIZE.md`](../Text3D/docs/PBR_MATERIALIZE.md).

---

## Mapas gerados

A partir de uma única imagem difusa/albedo, a ferramenta produz seis mapas:

| Mapa | Descrição |
|------|-----------|
| **Height** | Elevação da superfície para parallax/deslocamento |
| **Normal** | Normais da superfície para iluminação |
| **Metallic** | Máscara metálico vs dielétrico |
| **Smoothness** | Rugosidade/suavidade (base + contribuição metálica) |
| **Edge** | Deteção de arestas derivada das normais |
| **AO** | Oclusão ambiental (estilo cavidade a partir da height) |

---

## Funcionalidades

- **Minimalista** — Só linha de comando; fácil de automatizar e integrar em scripts
- **Rápida** — *Compute shaders* na GPU (wgpu); sem processamento pesado de imagem só na CPU
- **Multiplataforma** — Linux, macOS, Windows (Vulkan, Metal, DirectX 12)
- **Flexível** — Formatos de saída: PNG, JPG, TGA, EXR; qualidade JPEG configurável

---

## Arranque rápido

### Instalação (recomendado)

Requer **Python 3** (instalador) e **Rust** (cargo) para compilar. O instalador compila e coloca o binário em `~/.local/bin/materialize` (confirme que está no seu `PATH`).

```bash
git clone https://github.com/maikramer/Materialize-CLI.git
cd Materialize-CLI
./install.sh
```

- **Linux/macOS:** `./install.sh` | `./install.sh uninstall` | `./install.sh reinstall`
- **Windows:** `.\install.ps1` ou `install.bat`

### Execução

```bash
materialize texture.png
# Escreve no diretório atual:
#   texture_height.png, texture_normal.png, texture_metallic.png,
#   texture_smoothness.png, texture_edge.png, texture_ao.png

materialize texture.png -o ./out/ -v
materialize diffuse.png --format png --quiet
```

### Compilação manual (Cargo)

```bash
cargo build --release
cargo install --path .
```

---

## Uso

### Sintaxe

```text
materialize [OPTIONS] [INPUT] [COMMAND]
```

### Opções

| Opção | Curta | Predefinição | Descrição |
|-------|-------|--------------|-----------|
| `--output` | `-o` | `./` | Diretório de saída |
| `--format` | `-f` | `png` | Formato de saída: `png`, `jpg`, `jpeg`, `tga`, `exr` |
| `--quality` | `-q` | `95` | Qualidade JPEG (0–100) com `-f jpg` |
| `--verbose` | `-v` | — | Mostrar progresso e tempos |
| `--quiet` | — | — | Não listar ficheiros gerados em caso de sucesso |
| `--help` | `-h` | — | Mostrar ajuda |
| `--version` | `-V` | — | Mostrar versão |

### Subcomandos

- **`materialize skill install`** — Instala a [skill do Cursor](.cursor/skills/materialize-cli/) do Materialize CLI no projeto atual, em `.cursor/skills/materialize-cli/`.

### Nomes dos ficheiros de saída

Para o input `texture.png`, as saídas são:

- `texture_height.png`
- `texture_normal.png`
- `texture_metallic.png`
- `texture_smoothness.png`
- `texture_edge.png`
- `texture_ao.png`

(A extensão segue `--format`.)

### Códigos de saída

| Código | Significado |
|--------|-------------|
| `0` | Sucesso |
| `1` | Erro genérico |
| `2` | Ficheiro de entrada não encontrado |
| `3` | Formato de entrada não suportado |
| `4` | Erro de GPU (sem *adapter*) |
| `5` | Erro de I/O (permissões, disco cheio, etc.) |
| `6` | Imagem demasiado grande para a GPU |

---

## Exemplos

```bash
# Predefinição: diretório atual, PNG
materialize brick.png

# Diretório de saída e modo verboso
materialize brick.png -o ./materials/brick/ -v

# EXR para HDR / precisão
materialize texture.png -f exr -o ./out/

# Lote (paralelo com xargs)
ls *.png | xargs -P 4 -I {} materialize {} -o ./output/

# Adequado a scripts: silencioso, verificar código de saída
materialize texture.png -o ./out/ --quiet
if [ $? -eq 0 ]; then echo "OK"; fi
```

---

## Obter ajuda

- **Bugs e funcionalidades** — [Abrir um *issue*](https://github.com/maikramer/Materialize-CLI/issues) (há modelos para relatórios de bug e pedidos de funcionalidade).
- **Dúvidas** — [GitHub Discussions](https://github.com/maikramer/Materialize-CLI/discussions).
- **Contribuir** — Ver [CONTRIBUTING.md](CONTRIBUTING.md). Seguimos um [Código de conduta](CODE_OF_CONDUCT.md).

---

## Requisitos

- **Rust** 1.75+
- **GPU** com Vulkan (Linux), Metal (macOS) ou DirectX 12 (Windows); controladores atualizados

---

## Documentação

- [docs/README.md](docs/README.md) — Visão geral, detalhes de instalação e índice da documentação
- [docs/cli-api.md](docs/cli-api.md) — Referência completa da CLI, variáveis de ambiente, conclusão na shell
- [docs/architecture.md](docs/architecture.md) — Estrutura do sistema
- [docs/features.md](docs/features.md) — Capacidades
- [docs/algorithms.md](docs/algorithms.md) — Algoritmos de processamento
- [docs/shaders.md](docs/shaders.md) — *Shaders* WGSL
- [docs/roadmap.md](docs/roadmap.md) — Planos futuros

---

## Licença

[MIT](LICENSE). Baseado no Materialize original da Bounding Box Software.
