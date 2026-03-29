# Hunyuan3D-Paint — Setup

O comando `text3d texture` e `text3d generate --texture` / `--final` usam o **texgen** do `hy3dgen`, que precisa de um rasterizador GPU.

## Rasterizador: nvdiffrast (recomendado)

O Text3D inclui um **shim** que usa **nvdiffrast** (NVIDIA) como drop-in para o `custom_rasterizer` original. Não é necessária compilação manual.

### Instalação

O `nvdiffrast` é instalado automaticamente pelo `pip install -e .` (está no `pyproject.toml`). Se falhar:

```bash
pip install git+https://github.com/NVlabs/nvdiffrast.git --no-build-isolation
```

### Verificação

```bash
text3d doctor
```

Deve mostrar: `rasterizador OK — nvdiffrast (shim)`.

Ou em Python:

```python
from text3d.painter import check_paint_rasterizer_available
check_paint_rasterizer_available()
print("OK")
```

## VRAM modesta (~6 GB)

- O Text3D carrega os pesos Paint em **CPU** e usa `enable_model_cpu_offload` (evita OOM).
- Opcional: `export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True` (o CLI define automaticamente).

## Uso

```bash
# Pintar meshes existentes
text3d texture mesh.glb -i ref.png -o out.glb

# Tudo num comando (texto → mesh → textura)
text3d generate "prompt" -o modelo.glb

# Sem textura (só geometria)
text3d generate "espada" --no-texture -o espada.glb
```

## Alternativa: custom_rasterizer nativo

Se preferires a extensão CUDA original (sem shim), compila manualmente:

```bash
git clone --depth 1 https://github.com/Tencent-Hunyuan/Hunyuan3D-2.git
cd Hunyuan3D-2/hy3dgen/texgen/custom_rasterizer
pip install -e . --no-build-isolation
```

O Text3D detecta automaticamente a extensão nativa e usa-a em vez do shim.
