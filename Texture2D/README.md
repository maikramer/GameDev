# Texture2D

**Language:** English · [Português (`README_PT.md`)](README_PT.md)

CLI for **seamless (tileable) 2D textures** via the HF Inference API.

Uses the [Flux-Seamless-Texture-LoRA](https://huggingface.co/gokaygokay/Flux-Seamless-Texture-LoRA) model to generate textures that repeat without visible seams — ideal for floors, rocks, walls, and game-dev materials.

In the [GameDev](../README.md) monorepo, the package depends on [**gamedev-shared**](../Shared/) (`gamedev_shared`): Rich CLI, Cursor skill install, and helpers aligned with Text2D/Text3D/GameAssets.

## Features

- **No local GPU** — 100% cloud generation via HF Inference API
- **Automatic seamless prompt** — appends tileable/seamless instructions automatically
- **13 material presets** — Wood, Stone, Grass, Sand, Dirt, Metal, Brick, etc.
- **Batch** — multiple textures from a prompt file
- **JSON metadata** — each texture has a `.json` sidecar with seed, final prompt, parameters

## Quick start

```bash
# 1. Setup (venv + deps)
./scripts/setup.sh

# 2. Activate
source .venv/bin/activate

# 3. Generate
texture2d generate "rough stone wall surface, medieval castle" -o stone.png

# 4. Use a preset
texture2d generate "weathered surface" --preset Stone -o wall.png
```

## Installation

### Official (monorepo)

At the **GameDev** repo root:

```bash
cd /path/to/GameDev
./install.sh texture2d
# Windows: .\install.ps1 texture2d
```

The installer **creates** `Texture2D/.venv` if missing, editable install, and wrappers in `~/.local/bin`. Tool list: `./install.sh --list`. General guide: [docs/INSTALLING.md](../docs/INSTALLING.md).

### Manual / development

```bash
./scripts/setup.sh
source .venv/bin/activate
```

`setup.sh` installs `gamedev-shared` from `../Shared` and the `texture2d` package in editable mode (dev convenience; does not replace the official flow above).

### Local shortcut

```bash
python3 scripts/installer.py --prefix ~/.local
# or: ./scripts/run_installer.sh / ./scripts/install.sh
python3 scripts/installer.py --use-venv
```

No local PyTorch — only `config/requirements.txt` and `gamedev-shared`.

## Commands

| Command | Description |
|---------|-------------|
| `texture2d generate PROMPT` | Generate a seamless texture |
| `texture2d presets` | List material presets |
| `texture2d batch FILE` | Batch from file (one prompt per line) |
| `texture2d info` | Config and environment |
| `texture2d skill install` | Install Cursor Agent Skill |

## `generate` parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `--output/-o` | auto | Output file (.png) |
| `--width/-W` | 1024 | Width (256–2048, multiple of 8) |
| `--height/-H` | 1024 | Height |
| `--steps/-s` | 50 | Inference steps (10–100) |
| `--guidance/-g` | 7.5 | Guidance scale (1.0–20.0) |
| `--seed` | random | Reproducibility |
| `--negative-prompt/-n` | "" | Negative prompt |
| `--preset/-p` | None | Material preset |
| `--cfg-scale` | guidance | CFG scale |
| `--lora-strength` | 1.0 | LoRA strength (0.0–2.0) |
| `--model/-m` | Flux-Seamless-Texture-LoRA | HF model |

## Configuration

| Variable | Description |
|----------|-------------|
| `HF_TOKEN` | Hugging Face token (or `HUGGINGFACEHUB_API_TOKEN`) |
| `TEXTURE2D_MODEL_ID` | Model override (default: `gokaygokay/Flux-Seamless-Texture-LoRA`) |

> **Note:** generation uses the HF Inference API (cloud). Latency depends on server load. No local GPU usage. The API may rate-limit — see [HF Inference API docs](https://huggingface.co/docs/api-inference/).

## Materialize integration

Generate the diffuse texture then use Materialize for PBR maps:

```bash
texture2d generate "mossy stone" -o diffuse.png
materialize diffuse.png --output-dir pbr/
```

## GameAssets integration

[GameAssets](../GameAssets/) can use `texture2d` as the image source:

- In `game.yaml`, set `image_source: texture2d` (global) or per CSV row with `image_source`.
- With `texture2d.materialize: true` in the profile, GameAssets generates PBR maps automatically via Materialize.

```bash
gameassets batch --profile game.yaml --manifest manifest.csv
```

Use `TEXTURE2D_BIN` if the command is not on `PATH`.

## Layout

```
Texture2D/
├── src/texture2d/
│   ├── cli.py             # Click CLI (generate, batch, presets, info)
│   ├── generator.py       # HF Inference API client
│   ├── presets.py         # 13 material presets
│   ├── image_processor.py # Image processing
│   └── utils.py           # Helpers
├── config/
│   ├── requirements.txt
│   └── requirements-dev.txt
├── scripts/
│   ├── setup.sh           # Venv + deps
│   ├── run_installer.sh   # Calls installer.py
│   ├── install.sh         # Delegates to run_installer.sh
│   └── installer.py       # Logic shared with gamedev-install
└── tests/
```

## Tests

```bash
pip install -e ".[dev]"
pytest tests/ -v
```

## License

- **Code:** MIT — [LICENSE](LICENSE).
- **Weights (default):** [Flux-Seamless-Texture-LoRA](https://huggingface.co/gokaygokay/Flux-Seamless-Texture-LoRA) — HF metadata indicates Apache 2.0; also comply with **base model** (FLUX) and [HF Inference API](https://huggingface.co/docs/api-inference/) terms.
- **Full table:** [GameDev/README.md](../README.md) (Licenses section).
