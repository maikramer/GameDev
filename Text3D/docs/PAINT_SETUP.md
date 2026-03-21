# Hunyuan3D-Paint — `custom_rasterizer`

O comando `text3d texture` e `text3d generate --texture` / `--final` usam o **texgen** do `hy3dgen`, que depende de uma extensão CUDA **`custom_rasterizer`** (compilada localmente).

## Requisitos

- **`nvcc`** (pacote `nvidia-cuda-toolkit` no Ubuntu, ou CUDA Toolkit NVIDIA).
- **`CUDA_HOME`** com `bin/nvcc`, `include/`, `lib64/` (o PyTorch usa isto ao compilar a extensão).
- **PyTorch com CUDA** (`torch.version.cuda`); o compilador deve reportar a **mesma versão major.minor** (ex.: 11.8 para `torch` cu118), senão o build falha.

### Ubuntu: PyTorch cu118 + toolkit do sistema (CUDA 12.x)

É comum o `apt` instalar CUDA 12.x enquanto o `torch` é **cu118**. Duas opções:

1. Instalar [CUDA Toolkit 11.8](https://developer.nvidia.com/cuda-11-8-0-download-archive) em `/usr/local/cuda-11.8` e usar `export CUDA_HOME=/usr/local/cuda-11.8`, **ou**
2. Criar um **`nvcc` wrapper** que reporta `release 11.8` e reencaminha para o `nvcc` real (ex. `/usr/lib/nvidia-cuda-toolkit/bin/nvcc`), e uma árvore `CUDA_HOME` com symlinks para `include` e `lib64` (ver exemplo no histórico do projeto ou cria `~/cuda_torch118/bin/nvcc`).

Compilar:

```bash
git clone --depth 1 https://github.com/Tencent-Hunyuan/Hunyuan3D-2.git
cd Hunyuan3D-2/hy3dgen/texgen/custom_rasterizer
export CUDA_HOME=...   # árvore válida com nvcc
export TORCH_CUDA_ARCH_LIST="8.9"   # ex.: RTX 40xx; ajusta ao teu GPU
pip install -e . --no-build-isolation
```

Verifica (o kernel liga ao `libc10` do PyTorch — importa **torch** primeiro):

```bash
python -c "import torch; import custom_rasterizer; import custom_rasterizer_kernel; print('OK')"
```

## VRAM modesta (~6 GB)

- O Text3D força o carregamento inicial dos pesos Paint em **CPU** e depois usa `enable_model_cpu_offload` (evita OOM ao carregar Delight + Multiview em GPU).
- Opcional: `export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True` para reduzir fragmentação.

## Depois

- Pintar meshes existentes: `text3d texture mesh.glb -i ref.png -o out.glb`
- Tudo num comando: `text3d generate "prompt" --final -o modelo.glb`
- Script de exemplo: `scripts/paint_quality_meshes.sh` (exporta as variáveis acima se necessário)

## Nota

Sem `nvcc`/toolkit alinhado à versão do PyTorch, a compilação da extensão falha.
