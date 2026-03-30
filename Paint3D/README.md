# Paint3D

**Language:** English · [Português (`README_PT.md`)](README_PT.md)

3D texturing: **Hunyuan3D-Paint** (multiview) + **Materialize PBR** (normal, AO, metallic-roughness) + **AI upscale** (Real-ESRGAN).

## Installation

### Official (monorepo)

At the **GameDev** repo root:

```bash
cd /path/to/GameDev
./install.sh paint3d
```

Installs the package in `Paint3D/.venv`, PyTorch, **nvdiffrast**, and wrappers in `~/.local/bin` (or Windows equivalent). See [docs/INSTALLING.md](../docs/INSTALLING.md).

### Manual / advanced

```bash
cd Paint3D
pip install -e .              # core (paint + materialize)
pip install -e ".[upscale]"   # + AI upscale (spandrel)
```

The official installer handles **nvdiffrast** (`--no-build-isolation`); for manual install follow comments in `pyproject.toml`.

## CLI

```bash
# Texture mesh with reference image
paint3d texture mesh.glb -i ref.png -o mesh_textured.glb

# Texture + PBR
paint3d texture mesh.glb -i ref.png -o mesh_pbr.glb --materialize

# PBR only (already textured mesh)
paint3d materialize-pbr mesh_textured.glb -o mesh_pbr.glb

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
- **hy3dgen** (Hunyuan3D-2 — texture pipeline)
- **nvdiffrast** (NVIDIA — differentiable rasterizer)
- **spandrel** (optional — AI upscale)

## Documentation

- [Rasterizer setup](docs/PAINT_SETUP.md)
- [Materialize PBR](docs/PBR_MATERIALIZE.md)
