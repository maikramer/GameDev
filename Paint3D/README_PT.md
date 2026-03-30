# Paint3D

**Documentação:** [English (`README.md`)](README.md) · Português (esta página)

Texturização 3D: **Hunyuan3D-Paint** (textura multivista) + **Materialize PBR** (normal, AO, metallic-roughness) + **Upscale IA** (Real-ESRGAN).

## Instalação

### Oficial (monorepo)

Na raiz do repositório **GameDev**:

```bash
cd /caminho/para/GameDev
./install.sh paint3d
```

Instala o pacote no `Paint3D/.venv`, PyTorch, **nvdiffrast** e wrappers em `~/.local/bin` (ou equivalente no Windows). Ver [docs/INSTALLING_PT.md](../docs/INSTALLING_PT.md) · [EN](../docs/INSTALLING.md)

### Manual / avançado

```bash
cd Paint3D
pip install -e .              # core (paint + materialize)
pip install -e ".[upscale]"   # + upscale IA (spandrel)
```

O instalador oficial trata do **nvdiffrast** (`--no-build-isolation`); em instalação manual segue os comentários em `pyproject.toml`.

## CLI

```bash
# Texturizar mesh com imagem de referência
paint3d texture mesh.glb -i ref.png -o mesh_textured.glb

# Texturizar + PBR
paint3d texture mesh.glb -i ref.png -o mesh_pbr.glb --materialize

# Só PBR (mesh já texturizada)
paint3d materialize-pbr mesh_textured.glb -o mesh_pbr.glb

# Diagnóstico (rasterizador, GPU)
paint3d doctor

# Modelos usados
paint3d models
```

## API Python

```python
from paint3d import apply_hunyuan_paint, load_mesh_trimesh

mesh = load_mesh_trimesh("model.glb")
textured = apply_hunyuan_paint(mesh, "reference.png")
```

## Dependências

- **gamedev-shared** (monorepo GameDev — GPU, logging)
- **hy3dgen** (Hunyuan3D-2 — pipeline de textura)
- **nvdiffrast** (NVIDIA — rasterizador diferenciável)
- **spandrel** (opcional — upscale IA)

## Documentação

- [Setup do rasterizador](docs/PAINT_SETUP.md)
- [Materialize PBR](docs/PBR_MATERIALIZE.md)
