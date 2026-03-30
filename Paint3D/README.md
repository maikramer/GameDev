# Paint3D

**Language:** English · [Português (`README_PT.md`)](README_PT.md)

3D texturing: **Hunyuan3D-Paint 2.1** (multiview PBR in the exported GLB) + optional **AI upscale** (Real-ESRGAN).

Requires the **Hunyuan3D-2.1** tree: submodule `third_party/Hunyuan3D-2.1` or env `HUNYUAN3D_21_ROOT`. See [docs/PAINT_SETUP.md](docs/PAINT_SETUP.md).

## Installation

### Official (monorepo)

At the **GameDev** repo root:

```bash
cd /path/to/GameDev
./install.sh paint3d
```

Installs the package in `Paint3D/.venv`, PyTorch, **nvdiffrast**, initializes submodule **Hunyuan3D-2.1**, downloads **Real-ESRGAN** weights when possible, and adds wrappers. See [docs/INSTALLING.md](../docs/INSTALLING.md).

### Manual / advanced

```bash
cd Paint3D
pip install -e .              # core (paint)
pip install -e ".[upscale]"   # + AI upscale (spandrel)
```

The official installer handles **nvdiffrast** (`--no-build-isolation`); for manual install follow comments in `pyproject.toml`.

## CLI

```bash
# Texture mesh with reference image (GLB includes PBR material from Paint 2.1)
paint3d texture mesh.glb -i ref.png -o mesh_textured.glb

# Diagnostics (rasterizer, GPU)
paint3d doctor

# Models in use
paint3d models
```

## Python API

```python
from paint3d import apply_hunyuan_paint, load_mesh_trimesh

mesh = load_mesh_trimesh("model.glb")
textured = apply_hunyuan_paint(mesh, "reference.png")
```

## Dependencies

- **gamedev-shared** (GameDev monorepo — GPU, logging)
- **Hunyuan3D-2.1 `hy3dpaint`** (submodule or `HUNYUAN3D_21_ROOT`) — texture pipeline
- **pymeshlab**, **xatlas**, **omegaconf**, **basicsr**, **realesrgan** (super-resolution inside paint)
- **nvdiffrast** (NVIDIA — differentiable rasterizer shim)
- **spandrel** (optional — AI upscale on exported GLB)

## Documentation

- [Rasterizer setup](docs/PAINT_SETUP.md)
