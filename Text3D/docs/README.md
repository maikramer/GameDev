# Documentação Text3D

O Text3D gera meshes 3D a partir de texto em duas fases: **Text2D** (texto → imagem) e **Hunyuan3D-2mini** (imagem → mesh). Ver o [README principal](../README.md) para instalação e licença.

## Índice

- [Instalação](INSTALL.md) — pode estar desatualizado em relação ao fluxo Hunyuan; preferir o README raiz
- [API Python](API.md)
- [Troubleshooting](TROUBLESHOOTING.md) — conteúdo legado Shap-E
- [Exemplos](EXAMPLES.md) — exemplos antigos; usar API em [API.md](API.md)

## Visão geral

- **Text-to-3D:** `HunyuanTextTo3DGenerator.generate(prompt)`
- **Image-to-3D:** `generate_from_image(...)` (só Hunyuan)
- **Pouca VRAM:** `--low-vram` na CLI; descarrega Text2D antes de Hunyuan; `enable_model_cpu_offload` no Hunyuan quando CUDA
