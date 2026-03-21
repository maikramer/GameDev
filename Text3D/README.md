# Text3D

**Text-to-3D** em duas fases: **[Text2D](../Text2D)** (texto → imagem) e **[Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini)** (imagem → mesh). O modelo 2D é **sempre descarregado** antes de carregar o Hunyuan3D.

Os **valores por defeito** do CLI/API estão em [`src/text3d/defaults.py`](src/text3d/defaults.py): perfil **~6 GB VRAM** (CUDA) **validado na prática** (boa qualidade text-to-3D com os mesmos números que o comando sem flags extra). O **Text2D (FLUX)** usa **CPU offload** por defeito (`DEFAULT_T2D_CPU_OFFLOAD`), senão o modelo não cabe na GPU. Em GPU grande, `--t2d-full-gpu`. `--low-vram` força o **Hunyuan** em CPU (último recurso).

**Atalhos:** `--preset fast` (menos tempo/VRAM), `balanced` (igual aos defeitos), `hq` (alta qualidade, GPU grande) — ajusta `--steps`, `--octree-resolution` e `--num-chunks` em conjunto (se usares `--preset`, não esperes que `--steps`/`--octree-resolution`/`--num-chunks` “ganhem” ao perfil — o preset tem prioridade). **`text3d doctor`** verifica PyTorch, VRAM e se o **Paint** pode carregar (`custom_rasterizer`). O CLI define `PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True` se a variável ainda não existir (menos fragmentação de VRAM).

> **Licença dos pesos Hunyuan:** [Tencent Hunyuan Community](https://huggingface.co/tencent/Hunyuan3D-2mini) — uso não comercial; lê o model card antes de uso em produção.

[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)

## Requisitos

| Configuração | Mínimo | Recomendado |
|-------------|--------|-------------|
| Python | 3.10+ | 3.11+ |
| GPU | Opcional | CUDA ~6 GB+ (defeitos já calibrados) |
| RAM | 16GB | 32GB |
| Disco | ~20GB livres | Mais (cache Hugging Face) |

## Instalação (monorepo `GameDev`)

O [`config/requirements.txt`](config/requirements.txt) referencia `text2d @ file:../Text2D` e `hy3dgen` a partir do [repositório Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2).

```bash
cd GameDev/Text3D
python -m venv .venv && source .venv/bin/activate
pip install -r config/requirements.txt
pip install -e .
```

## Uso

```bash
text3d generate "um robô futurista" --output robo.glb

# GPU com mais VRAM (equivalente ao trio HQ do model card)
text3d generate "cadeira" --preset hq -W 1024 -H 1024

# Rápido (menos passos / octree mais baixo)
text3d generate "cadeira" --preset fast -o cadeira_fast.glb

# Último recurso: Hunyuan em CPU
text3d generate "objeto" --low-vram

text3d doctor
text3d info
text3d models
text3d convert mesh.ply --output mesh.glb

# Textura (Hunyuan3D-Paint — pesos em tencent/Hunyuan3D-2, 1.ª vez: download grande)
# --final = --texture: mesh + pintura no mesmo comando
text3d generate "robô" --final -o robo_tex.glb
text3d texture outputs/meshes/robo.glb -i minha_ref.png -o robo_tex.glb
```

### Textura (`Hunyuan3D-Paint`)

O shape (**Hunyuan3D-2mini**) não inclui material; o **Paint** gera UV + textura a partir da **mesma imagem** que condiciona o 3D (no fluxo com prompt, é a imagem Text2D). Usa o repositório [`tencent/Hunyuan3D-2`](https://huggingface.co/tencent/Hunyuan3D-2) (subpastas `hunyuan3d-delight-v2-0` e `hunyuan3d-paint-v2-0-turbo`), não só o mini. Por defeito os modelos Paint usam **CPU offload**; em GPU grande experimenta `--paint-full-gpu` no `generate --texture` / `--final` ou no `text3d texture`.

**Dependência nativa:** o texgen precisa do módulo **`custom_rasterizer`** (compilar a partir do [Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2), pasta `hy3dgen/texgen/custom_rasterizer`, com `nvcc` e `CUDA_HOME`). Passo a passo: [`docs/PAINT_SETUP.md`](docs/PAINT_SETUP.md).

**Um comando (mesh + pintura):** `text3d generate "teu prompt" --final -o modelo.glb` (equivalente a `--texture`).

### Parâmetros principais (defeitos = perfil ~6 GB, validado)

Ver [`defaults.py`](src/text3d/defaults.py). Resumo:

| Flag | Padrão atual | Exemplo GPU grande (HF) |
|------|----------------|-------------------------|
| `-W` / `-H` | 768 | 1024 |
| `--steps` | 24 | 30 |
| `--guidance` | 5.0 | 5.0 |
| `--octree-resolution` | 128 | 380 |
| `--num-chunks` | 4096 | 20000 |
| `--low-vram` | off | força Hunyuan em CPU se ainda OOM |
| `--seed` | — | — |
| `--preset` | — | `fast` / `balanced` / `hq` (substitui steps+octree+chunks) |
| `--mc-level` | 0 | Iso-superfície Hunyuan (ajuste fino) |

## Python

```python
from text3d import HunyuanTextTo3DGenerator
from text3d.utils import save_mesh

with HunyuanTextTo3DGenerator(verbose=True) as gen:
    mesh = gen.generate(prompt="um carro vermelho")
    # Opcional (GPU grande): gen.generate(..., octree_resolution=380, num_chunks=20000, num_inference_steps=30)
    save_mesh(mesh, "carro.glb", format="glb")

# Só image-to-3D (Hunyuan)
# mesh = gen.generate_from_image("ref.png")
```

## Estrutura

```
Text3D/
├── src/text3d/
│   ├── defaults.py     # Padrões ~6GB vs constantes HQ + Paint HF
│   ├── generator.py    # HunyuanTextTo3DGenerator
│   ├── painter.py      # Hunyuan3D-Paint (hy3dgen.texgen)
│   ├── cli.py
│   └── utils/
│       └── env.py      # PYTORCH_CUDA_ALLOC_CONF ao iniciar o CLI
├── config/requirements.txt
```

## Limitações do image-to-3D e pós-processo

O Hunyuan3D gera **superfície a partir de uma vista**: geometria fina (pernas, espelhos) pode desaparecer, aparecer **várias ilhas** (pés separados) ou aspereza tipo “argila”. Por defeito o CLI aplica **pós-processo**: maior **componente conexa** (remove ilhas pequenas), **merge de vértices** e opcionalmente `--mesh-smooth N` (suavização Laplaciana).

```bash
# Mais detalhe geométrico (mais VRAM/tempo)
text3d generate "robô" --octree-resolution 256 --num-chunks 8000 --steps 28

# Suavizar ligeiramente a superfície
text3d generate "carro" --mesh-smooth 1

# Manter todas as ilhas (se precisares de peças separadas)
text3d generate "objeto" --no-mesh-repair
```

## Créditos

- **Tencent Hunyuan3D** — [Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2), [Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini)
- **Text2D** — FLUX.2 Klein (SDNQ) no pacote `text2d` do monorepo
