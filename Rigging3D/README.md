# Rigging3D

CLI de **auto-rigging 3D** baseado no [UniRig](https://github.com/VAST-AI-Research/UniRig) (MIT).

## Instalação

### Setup automático (recomendado)

Um único comando instala tudo: venv, PyTorch+CUDA, dependências de inferência, spconv, torch-scatter/cluster e flash-attn.

```bash
cd Rigging3D
bash scripts/setup.sh
```

O script auto-detecta a versão CUDA do driver e instala as dependências correctas. Requer **Python 3.11** (para `bpy==4.2.0` e `open3d`).

```bash
bash scripts/setup.sh --python python3.11    # especificar interpretador
bash scripts/setup.sh --skip-flash           # pular flash-attn
bash scripts/setup.sh --force                # recriar venv do zero
```

### Alternativas

```bash
# Via install.sh do monorepo (só CLI base, sem deps pesadas):
./install.sh rigging3d

# Via installer.py (CLI + inferência):
python3 scripts/installer.py --use-venv --inference

# Manual (passo a passo):
cd Rigging3D && python3.11 -m venv .venv && source .venv/bin/activate
pip install -e ".[inference]"
```

### Deps CUDA-specific (se instalou manualmente)

O `setup.sh` instala tudo automaticamente, mas se precisares instalar manualmente:

```bash
# torch-scatter + torch-cluster (ajustar torch e CUDA):
pip install torch-scatter torch-cluster -f https://data.pyg.org/whl/torch-2.11.0+cu130.html

# spconv + cumm (cu121 para CUDA 12.x e 13.x):
pip install cumm-cu121 spconv-cu121
```

### flash_attn (opcional mas recomendado)

O pipeline funciona **sem** flash-attn (usa SDPA nativo do PyTorch como fallback), mas com flash-attn é mais rápido e usa menos VRAM. O `setup.sh` já o instala automaticamente.

Para instalar separadamente:

```bash
bash scripts/install_flash_attn.sh    # wheel pré-compilado (segundos)
```

O script tenta [wheels pré-compilados](https://github.com/mjun0812/flash-attention-prebuild-wheels) e só compila do source se não encontrar wheel compatível.

<details>
<summary>Compilação manual (fallback — 15-90 min, consome muita RAM)</summary>

```bash
export CUDA_HOME=/usr/local/cuda-13.2   # ajusta à tua instalação
export PATH="$CUDA_HOME/bin:$PATH"
mkdir -p "$HOME/tmp" && export TMPDIR="$HOME/tmp"
export MAX_JOBS=3                       # 1-4 conforme RAM disponível
export NVCC_THREADS=$MAX_JOBS
export CMAKE_BUILD_PARALLEL_LEVEL=$MAX_JOBS
export NINJAFLAGS=-j$MAX_JOBS

.venv/bin/pip install flash-attn --no-build-isolation --no-cache-dir
```

Cada job de `nvcc` pode usar vários GB de RAM. Com 64 GiB, `MAX_JOBS=3` costuma funcionar; com menos RAM, usa `1`.
</details>

### Pesos do modelo

Os pesos HF são descarregados automaticamente na 1.ª execução: [VAST-AI/UniRig](https://huggingface.co/VAST-AI/UniRig). Confirma termos no card (ver [GameDev/README](../README.md)).

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
