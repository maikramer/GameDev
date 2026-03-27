# Texture2D

CLI para geração de texturas 2D seamless (tileable) via HF Inference API.

Usa o modelo [Flux-Seamless-Texture-LoRA](https://huggingface.co/gokaygokay/Flux-Seamless-Texture-LoRA) para gerar texturas que repetem sem costuras visíveis — ideal para chão, rochas, paredes, e materiais de game dev.

No monorepo [GameDev](../README.md), o pacote depende de [**gamedev-shared**](../Shared/) (`gamedev_shared`): CLI Rich, instalação de skills Cursor e utilitários alinhados com Text2D/Text3D/GameAssets.

## Características

- **Sem GPU local** — geração 100% cloud via HF Inference API
- **Prompt seamless automático** — acrescenta instruções tileable/seamless automaticamente
- **13 presets de materiais** — Wood, Stone, Grass, Sand, Dirt, Metal, Brick, etc.
- **Batch** — gera múltiplas texturas a partir de um ficheiro de prompts
- **Metadata JSON** — cada textura acompanha ficheiro `.json` com seed, prompt final, parâmetros

## Arranque rápido

```bash
# 1. Setup (venv + deps)
./scripts/setup.sh

# 2. Ativar
source .venv/bin/activate

# 3. Gerar
texture2d generate "rough stone wall surface, medieval castle" -o stone.png

# 4. Usar preset
texture2d generate "weathered surface" --preset Stone -o wall.png
```

## Instalação

### Desenvolvimento local

```bash
./scripts/setup.sh
source .venv/bin/activate
```

O `setup.sh` instala `gamedev-shared` a partir de `../Shared` (caminho do monorepo) e o pacote `texture2d` em modo editável.

### Instalador unificado (raiz do monorepo GameDev)

A partir da raiz do repositório:

```bash
./install.sh texture2d
# Windows: .\install.ps1 texture2d
```

(O instalador unificado **cria** `Texture2D/.venv` se não existir, instala lá em modo editável e os wrappers em `~/.local/bin` apontam para esse Python. `scripts/setup.sh` continua opcional.)

Lista ferramentas: `./install.sh --list`.

### System-wide (script do projeto)

```bash
python3 scripts/installer.py --prefix ~/.local
# ou com venv:
python3 scripts/installer.py --use-venv
```

O instalador não usa PyTorch local — apenas dependências em `config/requirements.txt` e `gamedev-shared`.

## Comandos

| Comando | Descrição |
|---------|-----------|
| `texture2d generate PROMPT` | Gera uma textura seamless |
| `texture2d presets` | Lista presets de materiais |
| `texture2d batch FILE` | Batch a partir de ficheiro (um prompt por linha) |
| `texture2d info` | Configuração e ambiente |
| `texture2d skill install` | Instala Agent Skill Cursor |

## Parâmetros de `generate`

| Parâmetro | Default | Descrição |
|-----------|---------|-----------|
| `--output/-o` | auto | Ficheiro de saída (.png) |
| `--width/-W` | 1024 | Largura (256–2048, múltiplo de 8) |
| `--height/-H` | 1024 | Altura |
| `--steps/-s` | 50 | Passos de inferência (10–100) |
| `--guidance/-g` | 7.5 | Guidance scale (1.0–20.0) |
| `--seed` | aleatório | Seed para reprodutibilidade |
| `--negative-prompt/-n` | "" | Prompt negativo |
| `--preset/-p` | None | Preset de material |
| `--cfg-scale` | guidance | CFG scale |
| `--lora-strength` | 1.0 | Força do LoRA (0.0–2.0) |
| `--model/-m` | Flux-Seamless-Texture-LoRA | Modelo HF |

## Configuração

| Variável | Descrição |
|----------|-----------|
| `HF_TOKEN` | Token Hugging Face (ou `HUGGINGFACEHUB_API_TOKEN`) |
| `TEXTURE2D_MODEL_ID` | Override do modelo (default: `gokaygokay/Flux-Seamless-Texture-LoRA`) |

> **Nota:** a geração usa a HF Inference API (cloud). O tempo de resposta depende da carga dos servidores. Não há consumo de GPU local. A API pode ter rate limits — consulta a [documentação da HF Inference API](https://huggingface.co/docs/api-inference/) para detalhes.

## Integração com Materialize

Gere a textura diffuse e use o Materialize para mapas PBR:

```bash
texture2d generate "mossy stone" -o diffuse.png
materialize diffuse.png --output-dir pbr/
```

## Integração com GameAssets

O [GameAssets](../GameAssets/) pode usar `texture2d` como fonte de imagem:

- No `game.yaml`, definir `image_source: texture2d` (global) ou por linha no CSV com coluna `image_source`.
- Com `texture2d.materialize: true` no perfil, o GameAssets gera mapas PBR automaticamente via Materialize.

```bash
gameassets batch --profile game.yaml --manifest manifest.csv
```

Variável `TEXTURE2D_BIN` se o comando não estiver no `PATH`.

## Estrutura

```
Texture2D/
├── src/texture2d/
│   ├── cli.py             # CLI Click (generate, batch, presets, info)
│   ├── generator.py       # Cliente HF Inference API
│   ├── presets.py         # 13 presets de materiais
│   ├── image_processor.py # Processamento de imagem
│   └── utils.py           # Utilitários
├── config/
│   ├── requirements.txt
│   └── requirements-dev.txt
├── scripts/
│   ├── setup.sh           # Setup do venv + deps
│   ├── install.sh         # Instalador com wrappers
│   └── installer.py       # Instalador standalone
└── tests/
```

## Testes

```bash
pip install -e ".[dev]"
pytest tests/ -v
```

## Licença

- **Código:** MIT — [LICENSE](LICENSE).
- **Pesos (default):** [Flux-Seamless-Texture-LoRA](https://huggingface.co/gokaygokay/Flux-Seamless-Texture-LoRA) — metadata HF indica Apache 2.0; cumpre também os termos do **modelo base** (FLUX) e da [HF Inference API](https://huggingface.co/docs/api-inference/).
- **Tabela completa:** [GameDev/README.md](../README.md) (secção Licenças).
