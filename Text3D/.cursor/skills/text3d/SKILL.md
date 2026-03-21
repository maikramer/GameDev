---
name: text3d
description: Gera assets 3D a partir de texto usando Text2D + Hunyuan3D-2mini. Use quando o utilizador pedir modelos 3D, meshes, conversão PLY/GLB/OBJ, text-to-3D ou image-to-3D (Hunyuan).
---

# Text3D — Text2D + Hunyuan3D-2mini

## Quando usar

- Geração text-to-3D ou conversão de imagem para mesh 3D
- Formatos PLY, OBJ, GLB
- Menção a Hunyuan3D, Text2D, ou assets 3D para jogos

## Instalação (monorepo GameDev)

Na raiz `GameDev`, com venv ativo:

```bash
pip install -r Text3D/config/requirements.txt
pip install -e Text3D
```

Dependências pesadas: **text2d** (caminho `../Text2D` no requirements) e **hy3dgen** (git [Tencent-Hunyuan/Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2)). O modelo [Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini) está sob licença **Tencent Hunyuan Community** (uso não comercial).

## CLI

```bash
text3d generate "uma cadeira de madeira" --output cadeira.glb
# GPU grande (padrões HF / mais qualidade)
text3d generate "robô" -W 1024 -H 1024 --octree-resolution 380 --num-chunks 20000 --steps 30
# Textura (Hunyuan3D-Paint; pesos tencent/Hunyuan3D-2)
text3d generate "robô" --preset hq --final -o robo_tex.glb
text3d texture mesh.glb -i ref.png -o mesh_tex.glb
text3d doctor
text3d info
text3d convert entrada.ply --output saida.glb
```

Padrões do CLI em `text3d/defaults.py`: perfil ~6GB VRAM **validado na prática** (boa qualidade sem flags extra). `--low-vram` só se ainda der OOM (Hunyuan em CPU).

## API Python

```python
from text3d import HunyuanTextTo3DGenerator, defaults
from text3d.utils import save_mesh

with HunyuanTextTo3DGenerator() as gen:
    mesh = gen.generate(prompt="um carro vermelho")
    save_mesh(mesh, "carro.glb", format="glb")
# Qualidade HF: gen.generate(..., octree_resolution=defaults.HUNYUAN_HQ_OCTREE, ...)
```

## Parâmetros úteis

| Área | Notas |
|------|--------|
| `defaults.py` | `DEFAULT_*` (~6GB); `HUNYUAN_HQ_*` (model card / GPU grande) |
| `--low-vram` | Hunyuan em CPU se CUDA ainda não chegar |
| `--steps`, `--octree-resolution`, `--num-chunks` | Hunyuan shape |
| `-W`/`-H`, `--t2d-steps` | Text2D |

## Referências

- [Hunyuan3D-2mini no Hugging Face](https://huggingface.co/tencent/Hunyuan3D-2mini)
- Código upstream: [Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2)
