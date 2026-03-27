# Rigging3D

CLI de **auto-rigging 3D** baseado no [UniRig](https://github.com/VAST-AI-Research/UniRig) (MIT).

## Instalação

```bash
./install.sh rigging3d          # monorepo
cd Rigging3D && pip install -e ".[inference,dev]"  # manual
```

Dependências pesadas (PyTorch, bpy, flash_attn, etc.) estão no grupo `[inference]`. O pacote base instala apenas o CLI leve.

Pós-pip: **spconv**, **torch_scatter**, **torch_cluster** — vê o [README do UniRig](https://github.com/VAST-AI-Research/UniRig).

Pesos: [Hugging Face — VAST-AI/UniRig](https://huggingface.co/VAST-AI/UniRig) — confirma o `LICENSE` / README do snapshot que usas (mirrors com MIT existem; ver [GameDev/README](../README.md)).

## Requisitos

- Python 3.10+ (recomendado 3.11)
- GPU NVIDIA com CUDA (≥8 GB VRAM)
- **bash** para scripts de inferência — no Windows: Git Bash ou MSYS2

## Uso

```bash
rigging3d pipeline --input mesh.glb --output rigged.glb
rigging3d skeleton --input mesh.glb --output skel.fbx
rigging3d skin    --input skel.fbx --output skin.fbx
rigging3d merge   --source skin.fbx --target mesh.glb --output rigged.glb
```

Para apontar a outra árvore de inferência:

```bash
export RIGGING3D_ROOT=/outro/caminho
```

## Comandos

| Comando | Descrição |
|---------|-----------|
| `skeleton` | Gera skeleton (FBX) |
| `skin` | Skinning weights |
| `merge` | Junta skin + mesh original |
| `pipeline` | skeleton → skin → merge |

## Licença

- Rigging3D (CLI): **MIT** — [`LICENSE`](LICENSE)
- Código UniRig: **MIT** — [`unirig/LICENSE`](src/rigging3d/unirig/LICENSE) · [`THIRD_PARTY.md`](THIRD_PARTY.md)
- **Pesos HF:** o repositório [VAST-AI/UniRig](https://huggingface.co/VAST-AI/UniRig) pode não incluir `LICENSE` na raiz; valida termos no card e em forks com ficheiro explícito se necessário. Tabela no [README do monorepo](../README.md).
