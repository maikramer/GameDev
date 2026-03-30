# Text3D documentation

**Language:** English · [Português (`README_PT.md`)](README_PT.md)

Text3D generates 3D meshes from text in two phases: **Text2D** (text → image) and **Hunyuan3D-2mini** (image → mesh). See the [main README](../README.md) for installation and licensing.

## Index

- [Installation](INSTALL.md) — may lag behind the Hunyuan flow; prefer the root README
- [PBR + Materialize in GLB](PBR_MATERIALIZE.md) — full flow, requirements, CLI flags, findings on modest hardware
- [Python API](API.md)
- [Paint (custom_rasterizer)](PAINT_SETUP.md)
- [Troubleshooting](TROUBLESHOOTING.md) — legacy Shap-E content
- [Examples](EXAMPLES.md) — older examples; use API in [API.md](API.md)

## Overview

- **Text-to-3D:** `HunyuanTextTo3DGenerator.generate(prompt)`
- **Image-to-3D:** `generate_from_image(...)` (Hunyuan only)
- **Texture (Paint):** mesh + UV + albedo — see [PAINT_SETUP.md](PAINT_SETUP.md)
- **PBR in GLB (Materialize):** embedded normal, AO, metallic-roughness — see [PBR_MATERIALIZE.md](PBR_MATERIALIZE.md)
- **Low VRAM:** `--low-vram` on CLI; unload Text2D before Hunyuan; `enable_model_cpu_offload` on Hunyuan when using CUDA
