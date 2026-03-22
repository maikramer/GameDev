---
name: text2d
description: Gera imagens 2D a partir de texto com FLUX.2 Klein (SDNQ via Disty0). Use quando o utilizador pedir text-to-image, FLUX, imagens por prompt, TEXT2D_MODEL_ID, HF_HOME, low-vram, ou integração com GameAssets/Text3D.
---

# Text2D — text-to-image (FLUX.2 Klein)

## Quando usar

- Gerar **uma ou várias** imagens a partir de **prompt** em texto.
- Afinar **resolução**, **steps**, **seed**, ou correr em **CPU / pouca VRAM**.
- O utilizador menciona **FLUX Klein**, **Disty0**, **SDNQ**, ou pipelines que alimentam **Text3D** / **GameAssets**.

## O que é

CLI **text-to-2D** com [FLUX.2 Klein 4B](https://huggingface.co/black-forest-labs/FLUX.2-klein-4B) em quantização **SDNQ** ([Disty0](https://huggingface.co/Disty0/FLUX.2-klein-4B-SDNQ-4bit-dynamic)), pensado para GPUs modestas (**CPU offload**, `--low-vram`).

## Pré-requisitos

- Python e dependências do pacote (ver `docs/INSTALL.md`).
- Espaço em disco e rede para **primeiro download** dos pesos (vários GB).

## Comandos principais

| Comando | Função |
|---------|--------|
| `text2d generate PROMPT [-o ficheiro.png]` | Gera uma imagem |
| `text2d info` | Sistema, CUDA, VRAM, cache HF |
| `text2d models` | Modelos suportados / notas |
| `text2d skill install` | Instala esta skill em `.cursor/skills/text2d/` do projeto alvo |

**Opções frequentes em `generate`:** `--width` / `--height`, `--steps`, `--guidance` (SDNQ tipicamente **~1.0**), `--seed`, `--cpu`, `--low-vram`, `--model` / `TEXT2D_MODEL_ID`, `-v` (verbose).

## Exemplos

```bash
text2d generate "um gato com um cartaz" -o saida.png
text2d generate "paisagem" --width 768 --height 768 --steps 4 --guidance 1.0
text2d generate "retrato" --low-vram --seed 42 -o retrato.png
text2d -v generate "teste"
```

## Variáveis de ambiente

| Variável | Função |
|----------|--------|
| `TEXT2D_MODEL_ID` | Repo Hugging Face alternativo compatível com o pipeline Klein |
| `HF_HOME` | Raiz do cache Hugging Face |

## Notas importantes

- **Primeira execução:** download de pesos — pode parecer “parado” durante rede/disco.
- Pesos **GGUF** são para fluxos tipo ComfyUI-GGUF, **não** este CLI Diffusers.
- **Guidance** padrão **1.0** para o checkpoint SDNQ Disty0.
- Em **GameAssets**, resolução 2D elevada + outras apps na mesma GPU (ex.: **Godot** + editor 3D) aumenta risco de **OOM**; reduzir `width`/`height` no bloco `text2d` do `game.yaml` ou libertar VRAM.

## Ferramentas relacionadas

| Ferramenta | Ligação |
|------------|---------|
| **GameAssets** | Chama `text2d generate` por linha do manifest com prompts compostos. |
| **Text3D** | Pode usar Text2D no fluxo texto→imagem→3D, ou só imagem com `--from-image`. |

## Referências no repositório

- `src/text2d/cli.py` — CLI
- `src/text2d/generator.py` — `KleinFluxGenerator`
- `docs/INSTALL.md`, `docs/TROUBLESHOOTING.md`
