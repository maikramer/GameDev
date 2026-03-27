# Text3D

**Text-to-3D** em duas fases: **[Text2D](../Text2D)** (texto → imagem) e **[Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini)** (imagem → mesh). O modelo 2D é **sempre descarregado** antes de carregar o Hunyuan3D.

Os **valores por defeito** do CLI/API estão em [`src/text3d/defaults.py`](src/text3d/defaults.py): perfil **~6 GB VRAM** (CUDA) **validado na prática** (boa qualidade text-to-3D com os mesmos números que o comando sem flags extra). O **Text2D (FLUX)** usa **CPU offload** por defeito (`DEFAULT_T2D_CPU_OFFLOAD`), senão o modelo não cabe na GPU. Em GPU grande, `--t2d-full-gpu`. `--low-vram` força o **Hunyuan** em CPU (último recurso).

**Atalhos:** `--preset fast` (menos tempo/VRAM), `balanced` (igual aos defeitos), `hq` (alta qualidade, GPU grande) — ajusta `--steps`, `--octree-resolution` e `--num-chunks` em conjunto (se usares `--preset`, não esperes que `--steps`/`--octree-resolution`/`--num-chunks` “ganhem” ao perfil — o preset tem prioridade). **`text3d doctor`** verifica PyTorch, VRAM e se o **Paint** pode carregar (`custom_rasterizer`). O CLI define `PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True` se a variável ainda não existir (menos fragmentação de VRAM).

> **Licença dos pesos Hunyuan:** [Tencent Hunyuan 3D Community License](https://huggingface.co/tencent/Hunyuan3D-2mini) — lê o ficheiro `LICENSE` nos repositórios ([2mini](https://huggingface.co/tencent/Hunyuan3D-2mini), [Hunyuan3D-2 / Paint](https://huggingface.co/tencent/Hunyuan3D-2)): restrições de território, política de uso aceitável e obrigações. **Text2D (FLUX):** o default SDNQ no monorepo não é o mesmo regime que o BF16 Apache 2.0 da BFL — ver [Text2D/README](../Text2D/README.md) e [GameDev/README](../README.md).

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

| Subcomando | Descrição |
|-----------|-----------|
| `text3d generate PROMPT` | Gera mesh 3D a partir de texto (Text2D → Hunyuan3D) |
| `text3d doctor` | Verifica PyTorch, VRAM e dependências nativas |
| `text3d info` | Mostra configuração, GPU, cache e ambiente |
| `text3d models` | Lista modelos disponíveis |
| `text3d convert FILE` | Converte mesh entre formatos (PLY → GLB, etc.) |
| `text3d texture FILE` | Aplica textura Paint a um mesh existente |
| `text3d skill install` | Instala Agent Skill Cursor no projeto |

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

### PBR completo no GLB (Materialize)

Depois do Paint, o **Materialize CLI** (projeto [`Materialize`](../Materialize) no monorepo) gera **normal**, **oclusão** e **metallic-roughness** a partir do albedo embutido; o Text3D empacota tudo num **glTF 2.0** e grava o GLB.

**Um comando (texto → mesh → textura → PBR):**

```bash
text3d generate "a wooden crate" --texture --materialize --preset fast -o caixa_pbr.glb
```

**Guardar mapas PNG para inspeção:**

```bash
text3d generate "..." --texture --materialize -o out.glb --materialize-output-dir ./maps
```

**Requisito extra:** binário `materialize` no `PATH` (ou `MATERIALIZE_BIN`). Guia completo, achados em hardware modesto (~6 GB), tabelas de flags e referências: **[docs/PBR_MATERIALIZE.md](docs/PBR_MATERIALIZE.md)**.

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
│   ├── defaults.py        # Padrões ~6GB vs constantes HQ + Paint HF
│   ├── generator.py       # HunyuanTextTo3DGenerator
│   ├── painter.py         # Hunyuan3D-Paint (hy3dgen.texgen)
│   ├── materialize_pbr.py # Paint → Materialize CLI → GLB PBR (glTF)
│   ├── cli.py
│   └── utils/
│       └── env.py         # PYTORCH_CUDA_ALLOC_CONF ao iniciar o CLI
├── docs/
│   └── PBR_MATERIALIZE.md # Fluxo PBR, requisitos, flags, achados
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

## Documentação adicional

| Ficheiro | Descrição |
|----------|-----------|
| [docs/INSTALL.md](docs/INSTALL.md) | Guia de instalação detalhado |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | Resolução de problemas |
| [docs/EXAMPLES.md](docs/EXAMPLES.md) | Exemplos de uso avançado |
| [docs/API.md](docs/API.md) | Referência da API Python |
| [docs/PAINT_SETUP.md](docs/PAINT_SETUP.md) | Setup do Hunyuan3D-Paint + custom_rasterizer |
| [docs/PBR_MATERIALIZE.md](docs/PBR_MATERIALIZE.md) | Fluxo PBR com Materialize CLI |

## Variáveis de Ambiente

| Variável | Descrição |
|----------|-----------|
| `TEXT2D_MODEL_ID` | Override do modelo HF para a fase Text2D |
| `MATERIALIZE_BIN` | Caminho para o binário `materialize` (se não estiver no `PATH`) |
| `HF_HOME` | Diretório de cache Hugging Face (defeito: `~/.cache/huggingface`) |
| `PYTORCH_CUDA_ALLOC_CONF` | Configuração CUDA (auto-definida como `expandable_segments:True` se vazia) |
| `TEXT3D_ALLOW_SHARED_GPU` | Permitir GPU partilhada com outros processos (`1` = sim) |
| `TEXT3D_GPU_KILL_OTHERS` | Controlar terminação de processos GPU concorrentes (`0` = desligar, `1` = forçar) |
| `TEXT3D_EXPORT_ROTATION_X_DEG` | Rotação X em graus ao exportar mesh (defeito: 90°, Hunyuan→Y-up) |
| `TEXT3D_EXPORT_ROTATION_X_RAD` | Alternativa em radianos |

## Créditos

- **Tencent Hunyuan3D** — [Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2), [Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini)
- **Text2D** — FLUX.2 Klein (SDNQ Disty0 por defeito; opcional BF16 BFL via `TEXT2D_MODEL_ID`) no pacote `text2d` do monorepo — licenças: [GameDev/README](../README.md)
