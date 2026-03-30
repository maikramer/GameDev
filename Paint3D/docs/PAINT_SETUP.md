# Hunyuan3D-Paint 2.1 — Setup

O comando `paint3d texture` usa o código **`hy3dpaint`** do repositório [Hunyuan3D-2.1](https://github.com/Tencent-Hunyuan/Hunyuan3D-2.1) e descarrega os pesos **PBR** do Hugging Face ([`tencent/Hunyuan3D-2.1`](https://huggingface.co/tencent/Hunyuan3D-2.1), pasta `hunyuan3d-paintpbr-v2-1`).

## Patches no hy3dpaint (após submodule)

Depois de `git submodule update --init`, corre (na raiz do monorepo):

```bash
python Paint3D/scripts/apply_hunyuan21_patches.py
```

O instalador `./install.sh paint3d` / `install.ps1 paint3d` tenta aplicar estes patches automaticamente. Corrige o loop de resize multivista e torna a subpasta de pesos HF configurável (`--paint-subfolder`).

## Código hy3dpaint (obrigatório)

1. **Submodule (recomendado no monorepo GameDev)**

   ```bash
   git submodule update --init third_party/Hunyuan3D-2.1
   python Paint3D/scripts/apply_hunyuan21_patches.py
   ```

2. **Ou variável de ambiente** `HUNYUAN3D_21_ROOT` — caminho para a raiz do clone `Hunyuan3D-2.1` (ou directamente para a pasta `hy3dpaint`).

## Real-ESRGAN (obrigatório para o super-resolution do pipeline)

Ficheiro: `hy3dpaint/ckpt/RealESRGAN_x4plus.pth` (relativo ao clone).

```bash
# Linux / macOS
wget https://github.com/xinntao/Real-ESRGAN/releases/download/v0.1.0/RealESRGAN_x4plus.pth \
  -P third_party/Hunyuan3D-2.1/hy3dpaint/ckpt
```

No Windows, coloca o mesmo ficheiro nessa pasta ou corre `./install.ps1 paint3d` / `./install.sh paint3d` (o instalador tenta descarregar).

## Rasterizador: nvdiffrast (recomendado)

O Paint3D inclui um **shim** que usa **nvdiffrast** (NVIDIA) como `custom_rasterizer` antes de carregar o renderer 2.1.

```bash
pip install git+https://github.com/NVlabs/nvdiffrast.git --no-build-isolation
```

### Verificação

```bash
paint3d doctor
```

## Alternativa: custom_rasterizer nativo (Hunyuan3D-2.1)

```bash
cd third_party/Hunyuan3D-2.1/hy3dpaint/custom_rasterizer
pip install -e . --no-build-isolation
```

## VRAM

O model card indica da ordem de **~21 GB VRAM** para textura em configuração completa. Com `--paint-full-gpu` o Paint3D usa `render_size` / `texture_size` maiores; sem essa flag usa um perfil mais económico e tenta `enable_model_cpu_offload` no diffusion pipeline (quando suportado).

## Uso

```bash
paint3d texture mesh.glb -i ref.png -o out.glb
```

O pipeline 2.1 já produz GLB com material PBR. Para **mapas PBR a partir de uma imagem difusa** (não GLB), usa o projeto [Materialize](../../Materialize) ou `texture2d.materialize` no GameAssets — ver [Text3D/docs/PBR_MATERIALIZE.md](../../Text3D/docs/PBR_MATERIALIZE.md).
