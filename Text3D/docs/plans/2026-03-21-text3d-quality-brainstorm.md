# Brainstorm: qualidade Text3D (Text2D + Hunyuan3D-2mini)

Data: 2026-03-21  
Estado: proposta de design / priorização (sem implementação neste documento).

## Contexto

- Pipeline: **Text2D** (FLUX Klein) → imagem → **Hunyuan3D-2mini** (image-to-shape) → mesh GLB.
- Defeitos atuais calibrados para **~6GB VRAM**: Text2D com CPU offload, Hunyuan com `octree_resolution` / `num_chunks` baixos vs [model card HF](https://huggingface.co/tencent/Hunyuan3D-2mini) (380 / 20000 / 30 steps).
- O modelo 3D é **só geometria** (shape); textura PBR é outro estágio no ecossistema Tencent (Hunyuan3D-Paint), com requisitos de VRAM maiores.

## Porque o resultado “parece fraco”

1. **Condicionamento**: image-to-3D só “vê” uma vista; fundo ocupado, objeto pequeno ou mal enquadrado degradam o shape.
2. **Parâmetros**: octree/chunks baixos = menos detalhe e mais artefactos na superfície.
3. **Sem textura**: GLB pode ser cinzento ou sem UV rico — expectativa de “asset de jogo final” não bate só com shape.
4. **Limites do mini (0.6B)** vs variantes maiores / multiview.

## Eixos de melhoria

### A) Pré-processamento da imagem (antes do Hunyuan)

| Ideia | Fonte / notas |
|--------|----------------|
| **Remoção de fundo** (RGBA, objeto isolado) | Alinhado a tutoriais e uso comum em pipelines 3D; `rembg` já é dependência transitiva do `hy3dgen`. Alternativas HF: [briaai/RMBG-2.0](https://huggingface.co/briaai/RMBG-2.0), modelos u2net. |
| **Recorte / padding** para objeto centrado e quadrado | Reduz ruído lateral no condicionador de imagem. |
| **Evitar upscale agressivo** | Pode introduzir artefactos que o shape “copia”. Opcional: upscale leve (Real-ESRGAN) só se testes mostrarem ganho. |

### B) Parâmetros Hunyuan (mais tempo / VRAM)

- Subir `num_inference_steps`, `octree_resolution`, `num_chunks` conforme [discussão upstream](https://github.com/Tencent/Hunyuan3D-2/issues/46) (trade-off tempo e VRAM).
- Variantes **Turbo / Fast** no HF: menos passos, outro compromisso qualidade/velocidade.
- **Hunyuan3D-2mv** ([modelo multiview](https://huggingface.co/tencent/Hunyuan3D-2mv)): melhor consistência se houver **várias vistas** — fluxo de produto diferente (não só texto → uma imagem).

### C) Pós-processamento de mesh

- **pymeshlab** / **trimesh**: suavização, remoção de componentes pequenos, remeshing, reparo de buracos.
- Export GLB com normais consistentes para visualização.

### D) Text2D (melhor “briefing” para o 3D)

- Prompts estilo **produto**: “single object, centered, neutral background, studio lighting, full object in frame”.
- Resolução maior só com VRAM suficiente (`--t2d-full-gpu` ou máquina melhor).

### E) Textura (fora do mini shape só)

- **Hunyuan3D-Paint** (repositório Tencent): segunda etapa para textura; exige mais VRAM e integração separada.

## Três abordagens (trade-offs)

1. **Incremental (recomendada para já)**  
   Rembg opcional + subida moderada de `octree`/`chunks`/`steps` + flags CLI documentadas; pós-process leve com trimesh. Pouco código, ganho previsível.

2. **Média**  
   Pré-processo configurável (rembg vs outro modelo HF), presets `fast` / `balanced` / `quality`, e um passo opcional de limpeza de mesh.

3. **Completa (longo prazo)**  
   Multiview ou modelo paint; melhor qualidade final, mais dependências e VRAM.

## Próximo passo

Validar com o utilizador **o que mais incomoda** (silhueta, detalhe fino, ruído, falta de cor/textura) para priorizar A vs B vs C.
