# Hunyuan3D-Paint 2.1 — Setup

O comando `paint3d texture` usa o código **`hy3dpaint`** incluído no Paint3D em `Paint3D/src/paint3d/hy3dpaint/` (código espelhado a partir do repositório [Hunyuan3D-2.1](https://github.com/Tencent-Hunyuan/Hunyuan3D-2.1) no GitHub). Os **pesos PBR** são descarregados sob demanda do Hugging Face com `huggingface_hub.snapshot_download` a partir de [`tencent/Hunyuan3D-2.1`](https://huggingface.co/tencent/Hunyuan3D-2.1), apenas a subpasta `hunyuan3d-paintpbr-v2-1`.

## Código hy3dpaint (incluído no pacote)

Não é necessário submodule nem variável de ambiente para a localização do código: o módulo `hy3dpaint` está vendored em `src/paint3d/hy3dpaint/`. Basta instalar o Paint3D (`pip install -e .` ou `./install.sh paint3d` no monorepo).

## Real-ESRGAN (obrigatório para o super-resolution do pipeline)

Ficheiro: `Paint3D/src/paint3d/hy3dpaint/ckpt/RealESRGAN_x4plus.pth` (relativo à raiz do projeto Paint3D).

```bash
# Linux / macOS
wget https://github.com/xinntao/Real-ESRGAN/releases/download/v0.1.0/RealESRGAN_x4plus.pth \
  -P Paint3D/src/paint3d/hy3dpaint/ckpt
```

No Windows, coloca o mesmo ficheiro nessa pasta ou corre `./install.ps1 paint3d` / `./install.sh paint3d` (o instalador tenta descarregar).

## Rasterizador: nvdiffrast (recomendado)

O Paint3D inclui um **shim** que usa **nvdiffrast** (NVIDIA) como `custom_rasterizer` antes de carregar o renderer 2.1. Este é o caminho **recomendado**; não precisas de compilar a extensão CUDA nativa.

```bash
pip install git+https://github.com/NVlabs/nvdiffrast.git --no-build-isolation
```

### Verificação

```bash
paint3d doctor
```

## Alternativa opcional: custom_rasterizer nativo (CUDA)

A extensão CUDA **`custom_rasterizer`** do upstream **não** está vendored no repositório. Só faz sentido se quiseres substituir o shim nvdiffrast por a build nativa — nesse caso clona [Hunyuan3D-2.1](https://github.com/Tencent-Hunyuan/Hunyuan3D-2.1) à parte e compila `hy3dpaint/custom_rasterizer`, ou define `HUNYUAN3D_21_CUSTOM_RASTER` para o caminho dessa pasta e usa `scripts/install_custom_rasterizer.sh`.

## VRAM

O model card indica da ordem de **~21 GB VRAM** para textura em configuração completa. Com `--paint-full-gpu` o Paint3D usa `render_size` / `texture_size` maiores; sem essa flag usa um perfil mais económico e tenta `enable_model_cpu_offload` no diffusion pipeline (quando suportado).

## Uso

```bash
paint3d texture mesh.glb -i ref.png -o out.glb
```

O pipeline 2.1 já produz GLB com material PBR. Para **mapas PBR a partir de uma imagem difusa** (não GLB), usa o projeto [Materialize](../../Materialize) ou `texture2d.materialize` no GameAssets — ver [Text3D/docs/PBR_MATERIALIZE.md](../../Text3D/docs/PBR_MATERIALIZE.md).
