# API Python — Text3D

## HunyuanTextTo3DGenerator

Fluxo: **Text2D** (texto → imagem) → `unload` → **Hunyuan3DDiTFlowMatchingPipeline** (imagem → `trimesh.Trimesh`).

### Importação

```python
from text3d import HunyuanTextTo3DGenerator, apply_hunyuan_paint, defaults
from text3d.utils import save_mesh
```

Padrões de qualidade/memória: módulo `text3d.defaults` (perfil ~6GB; `HUNYUAN_HQ_*` para GPU grande). O dicionário `defaults.PRESET_HUNYUAN` espelha o CLI `--preset fast|balanced|hq`.

### Inicialização

```python
gen = HunyuanTextTo3DGenerator(
    device=None,              # "cuda" | "cpu" | auto
    low_vram_mode=False,      # True + CUDA: Hunyuan em CPU (lento)
    verbose=False,
    cache_dir=None,
    hunyuan_model_id="tencent/Hunyuan3D-2mini",
    hunyuan_subfolder="hunyuan3d-dit-v2-mini",
)
```

### `generate(prompt, ...)`

Text-to-3D completo.

| Parâmetro | Padrão | Descrição |
|-----------|--------|-----------|
| `t2d_width`, `t2d_height` | `defaults.DEFAULT_T2D_*` (768) | Text2D; 1024 em GPU grande |
| `t2d_steps` | `defaults.DEFAULT_T2D_STEPS` (8) | Passos Text2D (8 steps melhora aderência ao prompt de iluminação) |
| `t2d_guidance` | `defaults.DEFAULT_T2D_GUIDANCE` (1.0) | Guidance Text2D (SDNQ ~1.0) |
| `text2d_model_id` | None | Override HF Text2D |
| `t2d_seed` | None | Seed Text2D |
| `num_inference_steps` | `defaults.DEFAULT_HY_STEPS` (24) | Passos Hunyuan; HQ: `HUNYUAN_HQ_STEPS` |
| `guidance_scale` | `defaults.DEFAULT_HY_GUIDANCE` (5.0) | Guidance Hunyuan |
| `octree_resolution` | `defaults.DEFAULT_OCTREE_RESOLUTION` (128) | HQ: `HUNYUAN_HQ_OCTREE` |
| `num_chunks` | `defaults.DEFAULT_NUM_CHUNKS` (4096) | HQ: `HUNYUAN_HQ_NUM_CHUNKS` |
| `hy_seed` | None | Seed Hunyuan |
| `mc_level` | `defaults.DEFAULT_MC_LEVEL` (0.0) | Nível marching cubes |
| `t2d_full_gpu` | False | Se True, FLUX inteiro na GPU (só em máquinas com muita VRAM) |
| `return_reference_image` | False | Se True, devolve `(mesh, pil_image)` para textura Hunyuan3D-Paint |

Por defeito ``t2d_full_gpu=False`` combina com ``defaults.DEFAULT_T2D_CPU_OFFLOAD`` (offload no Text2D em CUDA).

**Retorno:** `trimesh.Trimesh`, ou `(trimesh.Trimesh, PIL.Image)` se `return_reference_image=True` (malha bruta; o CLI aplica `repair_mesh` antes de gravar/pintar).

### Hunyuan3D-Paint (`apply_hunyuan_paint`)

Pesos em ``defaults.DEFAULT_PAINT_HF_REPO`` (ex.: `tencent/Hunyuan3D-2`), subpasta ``defaults.DEFAULT_PAINT_SUBFOLDER``. Descarrega na primeira execução.

```python
from text3d import apply_hunyuan_paint, defaults
from text3d.utils import save_mesh, repair_mesh

mesh = gen.generate_from_image("ref.png")
mesh = repair_mesh(mesh)
gen.unload_hunyuan()
mesh_tex = apply_hunyuan_paint(
    mesh,
    "ref.png",
    model_repo=defaults.DEFAULT_PAINT_HF_REPO,
    subfolder=defaults.DEFAULT_PAINT_SUBFOLDER,
    paint_cpu_offload=defaults.DEFAULT_PAINT_CPU_OFFLOAD,
)
save_mesh(mesh_tex, "out.glb", format="glb")
```

Atalho ficheiro: `text3d.painter.paint_file_to_file(mesh_path, image_path, output_glb)`.

**Pré-requisito:** extensão CUDA `custom_rasterizer` — ver [`PAINT_SETUP.md`](PAINT_SETUP.md).

### Materialize PBR (`apply_materialize_pbr`)

Depois do Paint, podes gerar **normal**, **oclusão** e **metallic-roughness** (glTF 2.0) a partir do albedo embutido na mesh, usando o **Materialize CLI** (binário `materialize` no `PATH`, ou variável de ambiente `MATERIALIZE_BIN`). O CLI vive no monorepo em `GameDev/Materialize` (compilar e instalar; ver README desse projeto).

**Guia completo (fluxo, flags, achados em hardware modesto):** [`PBR_MATERIALIZE.md`](PBR_MATERIALIZE.md).

```python
from text3d import apply_hunyuan_paint, apply_materialize_pbr, defaults
from text3d.utils import save_mesh, repair_mesh

mesh = repair_mesh(gen.generate_from_image("ref.png"))
gen.unload_hunyuan()
mesh_tex = apply_hunyuan_paint(mesh, "ref.png", paint_cpu_offload=defaults.DEFAULT_PAINT_CPU_OFFLOAD)
mesh_pbr = apply_materialize_pbr(
    mesh_tex,
    save_sidecar_maps_dir="./maps_out",  # opcional: PNGs no disco
    roughness_from_one_minus_smoothness=True,  # roughness = 1 − smoothness (Unity-like)
)
save_mesh(mesh_pbr, "out_pbr.glb", format="glb")
```

**CLI:** `text3d generate ... --texture --materialize` ou `text3d texture mesh.glb -i ref.png -o out.glb --materialize`. Flags: `--materialize-output-dir`, `--materialize-bin`, `--materialize-no-invert`.

**Nota:** height/edge gerados pelo Materialize não têm slots padrão no material glTF base; só os mapas acima são embutidos no GLB. Os PNGs completos do Materialize podem ser guardados com `--materialize-output-dir`.

### Pós-processo de malha (`repair_mesh`)

Em código Python, após `generate` / `generate_from_image`, podes alinhar ao CLI:

```python
from text3d.utils import repair_mesh, save_mesh

mesh = gen.generate("prompt")
mesh = repair_mesh(mesh, smooth_iterations=0)  # ou 1–2 para suavizar
save_mesh(mesh, "out.glb")
```

`repair_mesh` faz merge de vértices, mantém a **maior componente conexa** (remove ilhas pequenas) e, se `smooth_iterations > 0`, suavização Laplaciana. Isto **não** fecha buracos nem “cola” pés ao chão: geometria fina e desconexões vêm sobretudo do modelo e dos parâmetros Hunyuan (`octree_resolution`, `num_chunks`, `num_inference_steps`).

### `generate_from_image(image, ...)`

Apenas Hunyuan (caminho, `Path` ou `PIL.Image`).

### Context manager

```python
with HunyuanTextTo3DGenerator() as gen:
    mesh = gen.generate("prompt")
```

## Exportação

### `save_mesh(mesh, path, format=..., rotate=True)`

Aceita **`trimesh.Trimesh`** (Hunyuan) ou **array numpy** (legado diffusers).

```python
save_mesh(mesh, "out.glb", format="glb")
```

### `convert_mesh`, `get_mesh_info`

Ver [`src/text3d/utils/export.py`](../src/text3d/utils/export.py).

## Memória

`text3d.utils.memory`: `get_system_info`, `get_gpu_info`, `format_bytes`.

## Referências

- [Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini)
- [Código Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2)
