# Hunyuan3D-Paint — Setup

O comando `paint3d texture` usa o **texgen** do `hy3dgen`, que precisa de um rasterizador GPU.

## Rasterizador: nvdiffrast (recomendado)

O Paint3D inclui um **shim** que usa **nvdiffrast** (NVIDIA) como drop-in para o `custom_rasterizer` original. Não é necessária compilação manual.

### Instalação

O `nvdiffrast` é instalado automaticamente pelo `pip install -e .` (está no `pyproject.toml`). Se falhar:

```bash
pip install git+https://github.com/NVlabs/nvdiffrast.git --no-build-isolation
```

### Verificação

```bash
paint3d doctor
```

Deve mostrar: `rasterizador OK — nvdiffrast (shim)`.

Ou em Python:

```python
from paint3d import check_paint_rasterizer_available
check_paint_rasterizer_available()
print("OK")
```

## VRAM modesta (~6 GB)

- O Paint3D carrega os pesos Paint em **CPU** e usa `enable_model_cpu_offload` (evita OOM).
- Opcional: `export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True`.

## Uso

```bash
# Pintar meshes existentes
paint3d texture mesh.glb -i ref.png -o out.glb

# Pintar + PBR
paint3d texture mesh.glb -i ref.png -o out.glb --materialize

# Sem textura (só PBR numa mesh já pintada)
paint3d materialize-pbr mesh_textured.glb -o mesh_pbr.glb
```

## Alternativa: custom_rasterizer nativo

Se preferires a extensão CUDA original (sem shim), compila manualmente:

```bash
git clone --depth 1 https://github.com/Tencent-Hunyuan/Hunyuan3D-2.git
cd Hunyuan3D-2/hy3dgen/texgen/custom_rasterizer
pip install -e . --no-build-isolation
```

O Paint3D detecta automaticamente a extensão nativa e usa-a em vez do shim.
