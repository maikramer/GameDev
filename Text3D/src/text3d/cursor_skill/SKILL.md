---
name: text3d
description: Gera meshes 3D a partir de texto ou imagem (Text2D + Hunyuan3D-2mini), Hunyuan3D-Paint, Materialize PBR opcional, conversão de formatos. Use quando o utilizador pedir 3D, GLB/PLY/OBJ, Hunyuan, text-to-3D, image-to-3D, --texture, --materialize, TEXT3D_BIN, MATERIALIZE_BIN, ou integração com GameAssets.
---

# Text3D — Text2D + Hunyuan3D-2mini

## Quando usar

- **Texto → 3D** ou **imagem → 3D** (mesh GLB/PLY/OBJ).
- Ativar **textura** (**Hunyuan3D-Paint**) ou **PBR completo** (**Materialize** embutido no fluxo `generate`).
- Diagnosticar GPU/PyTorch/Paint (`doctor`, `info`), converter formatos, listar modelos.

## O que é

Pipeline **text-to-3D**: geração de imagem (**Text2D** / FLUX Klein) + **Hunyuan3D-2mini** (imagem → mesh). Opcional: **Hunyuan3D-Paint** para GLB texturizado; opcional: **Materialize** para mapas PBR no GLB (`--materialize`, ver `docs/PBR_MATERIALIZE.md`). Entrada **só por imagem** (`--from-image` / `-i`) sem correr Text2D.

## Pré-requisitos

- Python e dependências (ver `docs/INSTALL.md`, `docs/PAINT_SETUP.md` se usares Paint).
- Para **Materialize** embutido: binário `materialize` no `PATH` ou `MATERIALIZE_BIN`.

## Comandos principais

| Comando | Função |
|---------|--------|
| `text3d generate "prompt" [-o mesh.glb]` | Texto → imagem → mesh |
| `text3d generate --from-image img.png -o mesh.glb` | Só Hunyuan (sem Text2D); alias `-i` |
| `text3d generate … --preset fast\|balanced\|hq` | Perfis (substituem steps/octree/chunks por defeito) |
| `text3d generate … --texture` (ou `--final`, `--with-texture`) | Mesh + Paint |
| `text3d generate … --texture --materialize [--materialize-output-dir …]` | Paint + PBR (Materialize CLI) |
| `text3d texture mesh.glb -i ref.png -o tex.glb` | Textura em mesh existente |
| `text3d doctor` | PyTorch, CUDA, Paint rasterizer |
| `text3d info` | Sistema e GPU |
| `text3d convert entrada.ply -o saida.glb` | Conversão PLY/OBJ/GLB |
| `text3d models` | Lista componentes (Text2D, Hunyuan, Paint) |
| `text3d skill install` | Instala esta skill em `.cursor/skills/text3d/` do projeto alvo |

## Exemplos

```bash
text3d generate "uma cadeira de madeira" -o cadeira.glb
text3d generate "robô" --preset hq --texture -o robo_tex.glb
text3d generate --from-image referencia.png -o so_mesh.glb
text3d texture modelo.glb -i ref.png -o modelo_tex.glb
text3d doctor
```

## Parâmetros úteis

- **VRAM baixa:** `--low-vram` (Hunyuan em CPU; **muito** mais lento), ou reduzir `--octree-resolution`, `--num-chunks`, `--steps`, ou usar `--preset fast`.
- **Qualidade:** `--preset hq` ou valores altos de steps/octree/chunks (ver `src/text3d/defaults.py`).
- **Text2D (quando aplicável):** `-W`/`-H`, `--t2d-steps`, `--t2d-guidance`, `--t2d-full-gpu`, `--model`.
- **Mesh:** `--no-mesh-repair`, `--mesh-smooth`, `--mc-level`.
- **Materialize:** `--materialize` requer `--texture`; `--materialize-output-dir`, `--materialize-bin`, `--materialize-no-invert`.

## Variáveis de ambiente (resumo)

| Variável | Função |
|----------|--------|
| `TEXT3D_BIN` | Caminho ao `text3d` se não estiver no `PATH` (útil para GameAssets) |
| `MATERIALIZE_BIN` | Caminho ao `materialize` quando não está no `PATH` |
| `HF_HOME` | Cache Hugging Face |

## Licenças e pesos

- **Hunyuan3D-2mini** e ecossistema Tencent: ver model cards e licença **Tencent Hunyuan Community** (restrições de uso).
- Primeira execução: downloads grandes para `~/.cache/huggingface/`.

## Ferramentas relacionadas

| Ferramenta | Ligação |
|------------|---------|
| **GameAssets** | Perfil `text3d` no `game.yaml` mapeia para os mesmos flags (incl. Materialize por linha do manifest). |
| **Materialize** | CLI autónoma para mapas PBR; no Text3D é invocada após Paint quando usas `--materialize`. |

## Documentação no repositório

- `docs/INSTALL.md`, `docs/TROUBLESHOOTING.md`, `docs/PAINT_SETUP.md`, `docs/API.md`, `docs/EXAMPLES.md`, `docs/PBR_MATERIALIZE.md`
- `src/text3d/defaults.py` — valores por defeito e presets HQ
