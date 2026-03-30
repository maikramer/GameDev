# Paint3D

**Documentação:** [English (`README.md`)](README.md) · Português (esta página)

Texturização 3D: **Hunyuan3D-Paint 2.1** (multivista PBR no GLB exportado) + **Upscale IA** (Real-ESRGAN).

O código **`hy3dpaint`** está incluído no Paint3D em `Paint3D/src/paint3d/hy3dpaint/`; os pesos PBR são descarregados sob demanda do Hugging Face (`tencent/Hunyuan3D-2.1`, pasta `hunyuan3d-paintpbr-v2-1`). Ver [docs/PAINT_SETUP.md](docs/PAINT_SETUP.md).

## Instalação

### Oficial (monorepo)

Na raiz do repositório **GameDev**:

```bash
cd /caminho/para/GameDev
./install.sh paint3d
```

Instala o pacote no `Paint3D/.venv`, PyTorch, **nvdiffrast**, pesos **Real-ESRGAN** quando possível, e wrappers. Ver [docs/INSTALLING_PT.md](../docs/INSTALLING_PT.md) · [EN](../docs/INSTALLING.md)

### Manual / avançado

```bash
cd Paint3D
pip install -e .              # core (paint)
pip install -e ".[upscale]"   # + upscale IA (spandrel)
```

O instalador oficial trata do **nvdiffrast** (`--no-build-isolation`); em instalação manual segue os comentários em `pyproject.toml`.

## CLI

```bash
# Texturizar mesh (GLB com material PBR do Paint 2.1)
paint3d texture mesh.glb -i ref.png -o mesh_textured.glb

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
- **Hunyuan3D-2.1 `hy3dpaint`** (incluído em `src/paint3d/hy3dpaint/`; pesos HF sob demanda)
- **pymeshlab**, **xatlas**, **omegaconf**; Real-ESRGAN (RRDBNet + inferência) vendido em código (sem pacotes PyPI basicsr/realesrgan)
- **nvdiffrast** (NVIDIA — shim do rasterizador)
- **spandrel** (opcional — upscale IA no GLB exportado)

## Documentação

- [Setup do rasterizador](docs/PAINT_SETUP.md)
