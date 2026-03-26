# Skymap2D

CLI para geração de skymaps equirectangular 360° via HF Inference API.

Usa o modelo [Flux-LoRA-Equirectangular-v3](https://huggingface.co/MultiTrickFox/Flux-LoRA-Equirectangular-v3) para gerar panorâmicas 360° usáveis como skybox/skymap em engines de jogo — ideal para céus, ambientes exteriores e cenários de fundo.

No monorepo [GameDev](../README.md), o pacote depende de [**gamedev-shared**](../Shared/) (`gamedev_shared`): CLI Rich, instalação de skills Cursor e utilitários alinhados com Text2D/Texture2D/GameAssets.

## Características

- **Sem GPU local** — geração 100% cloud via HF Inference API
- **Prompt equirectangular automático** — acrescenta instruções 360°/equirectangular automaticamente
- **10 presets de ambiente** — Sunset, Night Sky, Overcast, Clear Day, Storm, Space, etc.
- **Batch** — gera múltiplos skymaps a partir de um ficheiro de prompts
- **Metadata JSON** — cada skymap acompanha ficheiro `.json` com seed, prompt final, parâmetros
- **Ratio 2:1** — defaults optimizados (2048×1024) para projeção equirectangular

## Arranque rápido

```bash
# 1. Setup (venv + deps)
./scripts/setup.sh

# 2. Ativar
source .venv/bin/activate

# 3. Gerar
skymap2d generate "sunset over mountains, warm golden light" -o sky_sunset.png

# 4. Usar preset
skymap2d generate "dramatic sky" --preset Storm -o sky_storm.png
```

## Instalação

### Desenvolvimento local

```bash
./scripts/setup.sh
source .venv/bin/activate
```

O `setup.sh` instala `gamedev-shared` a partir de `../Shared` (caminho do monorepo) e o pacote `skymap2d` em modo editável.

### Instalador unificado (raiz do monorepo GameDev)

A partir da raiz do repositório:

```bash
./install.sh skymap2d
# Windows: .\install.ps1 skymap2d
```

(O instalador unificado **cria** `Skymap2D/.venv` se não existir, instala lá em modo editável e os wrappers em `~/.local/bin` apontam para esse Python. `scripts/setup.sh` continua opcional.)

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
| `skymap2d generate PROMPT` | Gera um skymap equirectangular 360° |
| `skymap2d presets` | Lista presets de ambiente |
| `skymap2d batch FILE` | Batch a partir de ficheiro (um prompt por linha) |
| `skymap2d info` | Configuração e ambiente |
| `skymap2d skill install` | Instala Agent Skill Cursor |

## Parâmetros de `generate`

| Parâmetro | Default | Descrição |
|-----------|---------|-----------|
| `--output/-o` | auto | Ficheiro de saída (.png) |
| `--width/-W` | 2048 | Largura (ratio 2:1 recomendado) |
| `--height/-H` | 1024 | Altura |
| `--steps/-s` | 40 | Passos de inferência (10–100) |
| `--guidance/-g` | 6.0 | Guidance scale (1.0–20.0) |
| `--seed` | aleatório | Seed para reprodutibilidade |
| `--negative-prompt/-n` | "" | Prompt negativo |
| `--preset/-p` | None | Preset de ambiente |
| `--cfg-scale` | guidance | CFG scale |
| `--lora-strength` | 1.0 | Força do LoRA (0.0–2.0) |
| `--model/-m` | Flux-LoRA-Equirectangular-v3 | Modelo HF |

## Presets

| Nome | Descrição |
|------|-----------|
| Sunset | Céu ao pôr do sol, nuvens douradas |
| Night Sky | Noite estrelada, Via Láctea |
| Overcast | Céu nublado, luz difusa |
| Clear Day | Céu limpo azul, poucas nuvens |
| Storm | Tempestade, nuvens escuras, relâmpagos |
| Space | Espaço exterior, nebulosa, estrelas |
| Alien World | Céu alienígena, duas luas, cores fantásticas |
| Dawn | Amanhecer, tons rosa e laranja |
| Underwater | Vista subaquática, raios de luz, água |
| Fantasy | Céu mágico, auroras, cristais flutuantes |

## Configuração

| Variável | Descrição |
|----------|-----------|
| `HF_TOKEN` | Token Hugging Face (ou `HUGGINGFACEHUB_API_TOKEN`) |
| `SKYMAP2D_MODEL_ID` | Override do modelo (default: `MultiTrickFox/Flux-LoRA-Equirectangular-v3`) |

## Uso em engines de jogo

O skymap equirectangular gerado pode ser usado directamente como:
- **Godot**: Environment → Sky → PanoramaSky → panorama texture
- **Unity**: Skybox material com shader Panoramic → assign texture
- **Unreal Engine**: Sky Sphere → equirectangular texture map

## Testes

```bash
pip install -e ".[dev]"
pytest tests/ -v
```

## Licença

MIT
