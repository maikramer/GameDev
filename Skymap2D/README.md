# Skymap2D

**Language:** English · [Português (`README_PT.md`)](README_PT.md)

CLI for **equirectangular 360° skymaps** via the HF Inference API.

Uses the [Flux-LoRA-Equirectangular-v3](https://huggingface.co/MultiTrickFox/Flux-LoRA-Equirectangular-v3) model to generate 360° panoramas usable as skybox/skymaps in game engines — ideal for skies, outdoor environments, and backgrounds.

In the [GameDev](../README.md) monorepo, the package depends on [**gamedev-shared**](../Shared/) (`gamedev_shared`): Rich CLI, Cursor skill install, and helpers aligned with Text2D/Texture2D/GameAssets.

## Features

- **No local GPU** — 100% cloud via HF Inference API
- **Automatic equirectangular prompt** — appends 360°/equirectangular instructions automatically
- **10 environment presets** — Sunset, Night Sky, Overcast, Clear Day, Storm, Space, etc.
- **Batch** — multiple skymaps from a prompt file
- **JSON metadata** — each skymap has a `.json` sidecar with seed, final prompt, parameters
- **2:1 ratio** — defaults tuned (2048×1024) for equirectangular projection
- **EXR output (optional)** — RGB float32 in **linear** space (OpenEXR), for engines that prefer `.exr`. The model still returns LDR; EXR packs the same content without a second sRGB curve. We do **not** use [Materialize](../Materialize/) here: that flow builds PBR maps (normal, height, …) from textures; for panoramas `skymap2d` with `--format exr` is enough.

## Quick start

```bash
# 1. Setup (venv + deps)
./scripts/setup.sh

# 2. Activate
source .venv/bin/activate

# 3. Generate
skymap2d generate "sunset over mountains, warm golden light" -o sky_sunset.png

# 4. Use a preset
skymap2d generate "dramatic sky" --preset Storm -o sky_storm.png

# 5. EXR (linear RGB) instead of PNG
skymap2d generate "clear blue sky" --format exr -o sky_clear.exr
# or: -o sky_clear.exr  (.exr extension sets format)
```

## Installation

### Official (monorepo)

At the **GameDev** repo root:

```bash
cd /path/to/GameDev
./install.sh skymap2d
# Windows: .\install.ps1 skymap2d
```

Creates `Skymap2D/.venv` if needed, editable install, and wrappers. `./install.sh --list`. Guide: [docs/INSTALLING.md](../docs/INSTALLING.md).

### Manual / development

```bash
./scripts/setup.sh
source .venv/bin/activate
```

`setup.sh` installs `gamedev-shared` from `../Shared` and the `skymap2d` package in editable mode.

### Local shortcut

```bash
python3 scripts/installer.py --prefix ~/.local
python3 scripts/installer.py --use-venv
```

No local PyTorch — only `config/requirements.txt` and `gamedev-shared`.

## Commands

| Command | Description |
|---------|-------------|
| `skymap2d generate PROMPT` | Generate a 360° equirectangular skymap |
| `skymap2d presets` | List environment presets |
| `skymap2d batch FILE` | Batch from file (one prompt per line) |
| `skymap2d info` | Config and environment |
| `skymap2d skill install` | Install Cursor Agent Skill |

## `generate` parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `--output/-o` | auto | Output file (`.png` or `.exr`) |
| `--format` | png | `png` or `exr` (if `-o` has no extension, this applies) |
| `--exr-scale` | 1.0 | Multiply linear values when writing EXR |
| `--width/-W` | 2048 | Width (2:1 ratio recommended) |
| `--height/-H` | 1024 | Height |
| `--steps/-s` | 40 | Inference steps (10–100) |
| `--guidance/-g` | 6.0 | Guidance scale (1.0–20.0) |
| `--seed` | random | Reproducibility |
| `--negative-prompt/-n` | "" | Negative prompt |
| `--preset/-p` | None | Environment preset |
| `--cfg-scale` | guidance | CFG scale |
| `--lora-strength` | 1.0 | LoRA strength (0.0–2.0) |
| `--model/-m` | Flux-LoRA-Equirectangular-v3 | HF model |

## Presets

| Name | Description |
|------|-------------|
| Sunset | Golden sunset, warm clouds |
| Night Sky | Starry night, Milky Way |
| Overcast | Cloudy sky, diffuse light |
| Clear Day | Clear blue sky, few clouds |
| Storm | Storm, dark clouds, lightning |
| Space | Outer space, nebula, stars |
| Alien World | Alien sky, two moons, fantasy colors |
| Dawn | Dawn, pink and orange tones |
| Underwater | Underwater view, light rays, water |
| Fantasy | Magic sky, auroras, floating crystals |

## Configuration

| Variable | Description |
|----------|-------------|
| `HF_TOKEN` | Hugging Face token (or `HUGGINGFACEHUB_API_TOKEN`) |
| `SKYMAP2D_MODEL_ID` | Model override (default: `MultiTrickFox/Flux-LoRA-Equirectangular-v3`) |

## Using in game engines

The generated equirectangular skymap can be used directly as:
- **Godot**: Environment → Sky → PanoramaSky → panorama texture
- **Unity**: Skybox material with Panoramic shader → assign texture
- **Unreal Engine**: Sky Sphere → equirectangular texture map

## Tests

```bash
pip install -e ".[dev]"
pytest tests/ -v
```

## License

- **Code:** MIT — [LICENSE](LICENSE).
- **Weights (default):** [Flux-LoRA-Equirectangular-v3](https://huggingface.co/MultiTrickFox/Flux-LoRA-Equirectangular-v3) — LoRA on [FLUX.1-dev](https://huggingface.co/black-forest-labs/FLUX.1-dev) (**non-commercial** BFL license); inference via [HF Inference API](https://huggingface.co/docs/api-inference/) — also [HF terms](https://huggingface.co/terms-of-service).
- **Full table:** [GameDev/README.md](../README.md) (Licenses section).
