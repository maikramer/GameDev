# Text3D

**Language:** English · [Português (`README_PT.md`)](README_PT.md)

**Text-to-3D** in two phases: **[Text2D](../Text2D)** (text → image) and **[Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini)** (image → mesh). The 2D model is **always unloaded** before loading Hunyuan3D.

**Default** CLI/API values are in [`src/text3d/defaults.py`](src/text3d/defaults.py): **~6 GB VRAM** (CUDA) profile **validated in practice** (good text-to-3D quality with the same numbers as the command without extra flags). **Text2D (FLUX)** uses **CPU offload** by default (`DEFAULT_T2D_CPU_OFFLOAD`), otherwise the model does not fit on the GPU. On a large GPU, `--t2d-full-gpu`. `--low-vram` forces **Hunyuan** on CPU (last resort).

**Shortcuts:** `--preset fast` (less time/VRAM), `balanced` (same as defaults), `hq` (high quality, large GPU) — adjusts `--steps`, `--octree-resolution`, and `--num-chunks` together (if you use `--preset`, do not expect `--steps` / `--octree-resolution` / `--num-chunks` to override the preset — preset wins). **`text3d doctor`** checks PyTorch and VRAM. The CLI sets `PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True` if the variable is unset (less VRAM fragmentation).

**Texturing and PBR** are not part of this package: use **[Paint3D](../Paint3D)** (`paint3d texture` / `materialize-pbr`) or **[GameAssets](../GameAssets)** with `text3d.texture` in the profile.

> **Hunyuan weight license:** [Tencent Hunyuan 3D Community License](https://huggingface.co/tencent/Hunyuan3D-2mini) — read `LICENSE` in the repos ([2mini](https://huggingface.co/tencent/Hunyuan3D-2mini), [Hunyuan3D-2 / Paint](https://huggingface.co/tencent/Hunyuan3D-2)): territory restrictions, acceptable use, obligations. **Text2D (FLUX):** default SDNQ in the monorepo is not the same regime as BFL BF16 Apache 2.0 — see [Text2D/README](../Text2D/README.md) and [GameDev/README](../README.md).

[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)

## Requirements

| Config | Minimum | Recommended |
|--------|---------|---------------|
| Python | 3.10+ | 3.11+ |
| GPU | Optional | CUDA ~6 GB+ (defaults tuned for this) |
| RAM | 16GB | 32GB |
| Disk | ~20 GB free | More (Hugging Face cache) |

## Installation

### Official (monorepo)

At the **GameDev** repo root:

```bash
cd /path/to/GameDev
./install.sh text3d
```

Editable install in `Text3D/.venv`, config in `~/.config/text3d`, wrappers in `~/.local/bin` (Linux/macOS) or `%USERPROFILE%\bin` (Windows). Optional variable: `PYTHON_CMD`. CLI flag: `--skip-env-config` (do not write `env.sh` / `env.bat`). Texturing: install **[Paint3D](../Paint3D)** separately.

Equivalent: `gamedev-install text3d`. General guide: [docs/INSTALLING.md](../docs/INSTALLING.md).

### Manual / advanced

[`config/requirements.txt`](config/requirements.txt) references `text2d @ file:../Text2D` and `hy3dgen` from the [Hunyuan3D-2 repository](https://github.com/Tencent-Hunyuan/Hunyuan3D-2).

```bash
cd GameDev/Text3D
python -m venv .venv && source .venv/bin/activate
pip install -r config/requirements.txt
pip install -e .
```

**Windows:** `python -m venv .venv` and `.\.venv\Scripts\Activate.ps1`; or `scripts\setup.ps1`.

### Local shortcut

`python scripts/installer.py` (or `scripts/run_installer.sh` / `scripts/install.sh`) uses the same logic as `./install.sh text3d` when run from `Text3D/`.

## Usage

| Subcommand | Description |
|------------|-------------|
| `text3d generate PROMPT` | Generate 3D mesh from text (Text2D → Hunyuan3D) |
| `text3d doctor` | Check PyTorch, VRAM, native deps |
| `text3d info` | Show config, GPU, cache, environment |
| `text3d models` | List available models |
| `text3d convert FILE` | Convert mesh formats (PLY → GLB, etc.) |
| `text3d skill install` | Install Cursor Agent Skill in the project |

```bash
# Geometry-only mesh (Text2D → Hunyuan3D)
text3d generate "a futuristic robot" -o robot.glb

# More VRAM (HF HQ triple)
text3d generate "chair" --preset hq -W 1024 -H 1024

# Fast (fewer steps / lower octree)
text3d generate "chair" --preset fast -o chair_fast.glb

# Last resort: Hunyuan on CPU
text3d generate "object" --low-vram

text3d doctor
text3d info
text3d models
text3d convert mesh.ply --output mesh.glb

# Texture an existing mesh (Paint3D project)
paint3d texture outputs/meshes/robot.glb -i my_ref.png -o robot_tex.glb
```

### Texture and PBR

Full text → mesh → texture → PBR maps: **[GameAssets](../GameAssets)** (`gameassets batch` with `text3d.texture` / `materialize`) or chain manually `text3d generate` → `paint3d texture` → `paint3d materialize-pbr`. Materialize details: **[docs/PBR_MATERIALIZE.md](docs/PBR_MATERIALIZE.md)** and **[Paint3D/docs/PAINT_SETUP.md](../Paint3D/docs/PAINT_SETUP.md)**.

### Main parameters (defaults = ~6 GB profile, validated)

See [`defaults.py`](src/text3d/defaults.py). Summary:

| Flag | Current default | Large GPU example (HF) |
|------|-----------------|-------------------------|
| `-W` / `-H` | 768 | 1024 |
| `--steps` | 24 | 30 |
| `--guidance` | 5.0 | 5.0 |
| `--octree-resolution` | 256 | 380 |
| `--num-chunks` | 8000 | 20000 |
| `--low-vram` | off | forces Hunyuan on CPU if still OOM |
| `--seed` | — | — |
| `--preset` | — | `fast` / `balanced` / `hq` (replaces steps+octree+chunks) |
| `--mc-level` | 0 | Hunyuan iso-surface (fine tuning) |

## Python

```python
from text3d import HunyuanTextTo3DGenerator
from text3d.utils import save_mesh

with HunyuanTextTo3DGenerator(verbose=True) as gen:
    mesh = gen.generate(prompt="a red car")
    # Optional (large GPU): gen.generate(..., octree_resolution=380, num_chunks=20000, num_inference_steps=30)
    save_mesh(mesh, "car.glb", format="glb")

# Image-to-3D only (Hunyuan)
# mesh = gen.generate_from_image("ref.png")
```

## Layout

```
Text3D/
├── src/text3d/
│   ├── defaults.py        # ~6GB defaults vs HQ constants
│   ├── generator.py       # HunyuanTextTo3DGenerator
│   ├── cli.py
│   └── utils/
│       └── env.py         # PYTORCH_CUDA_ALLOC_CONF at CLI startup
├── docs/
│   └── PBR_MATERIALIZE.md # → Paint3D
├── config/requirements.txt

# Texture, Materialize PBR, AI upscale → Paint3D package (../Paint3D)
```

## Image-to-3D limitations and post-processing

Hunyuan3D generates **surface from one view**: fine geometry (legs, mirrors) may disappear, **multiple islands** (separate feet) or clay-like roughness may appear. By default the CLI applies **post-processing**: largest **connected component** (removes small islands), **vertex merge**, and optionally `--mesh-smooth N` (Laplacian smoothing).

```bash
# More geometric detail (more VRAM/time)
text3d generate "robot" --octree-resolution 256 --num-chunks 8000 --steps 28

# Slightly smooth the surface
text3d generate "car" --mesh-smooth 1

# Keep all islands (if you need separate pieces)
text3d generate "object" --no-mesh-repair
```

## Additional documentation

| File | Description |
|------|-------------|
| [docs/INSTALL.md](docs/INSTALL.md) | Detailed install guide |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | Troubleshooting |
| [docs/EXAMPLES.md](docs/EXAMPLES.md) | Advanced examples |
| [docs/API.md](docs/API.md) | Python API reference |
| [docs/PAINT_SETUP.md](docs/PAINT_SETUP.md) | Points to Paint3D (Hunyuan texture) |
| [docs/PBR_MATERIALIZE.md](docs/PBR_MATERIALIZE.md) | Points to Paint3D + Materialize |

## Environment variables

| Variable | Description |
|----------|-------------|
| `TEXT2D_MODEL_ID` | HF model override for Text2D phase |
| `MATERIALIZE_BIN` | Used by **paint3d** (materialize-pbr), not by `text3d` |
| `HF_HOME` | Hugging Face cache directory (default: `~/.cache/huggingface`) |
| `PYTORCH_CUDA_ALLOC_CONF` | CUDA config (auto-set to `expandable_segments:True` if empty) |
| `TEXT3D_ALLOW_SHARED_GPU` | Allow GPU sharing with other processes (`1` = yes) |
| `TEXT3D_GPU_KILL_OTHERS` | Control termination of competing GPU processes (`0` = off, `1` = force) |
| `TEXT3D_EXPORT_ROTATION_X_DEG` | X rotation in degrees when exporting mesh (default: 90°, Hunyuan→Y-up) |
| `TEXT3D_EXPORT_ROTATION_X_RAD` | Alternative in radians |

## Credits

- **Tencent Hunyuan3D** — [Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2), [Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini)
- **Text2D** — FLUX.2 Klein (SDNQ Disty0 by default; optional BFL BF16 via `TEXT2D_MODEL_ID`) in the monorepo `text2d` package — licenses: [GameDev/README](../README.md)
