# GameDev

Monorepo com ferramentas de **texto para imagem** e **texto para 3D**, partilhando a mesma base de scripts e documentação.

## Projetos

| Pasta | Descrição |
|-------|-----------|
| [**Text2D**](Text2D/) | CLI **text-to-image** com FLUX (quantização SDNQ), orientada a GPU modesta. |
| [**Text3D**](Text3D/) | Pipeline **text-to-3D**: imagem 2D (via Text2D) → mesh GLB com Hunyuan3D; pintura opcional. |

Cada projeto tem o seu próprio `README`, `setup`, requisitos e licença.

## Requisitos gerais

- **Python** 3.10 ou superior (detalhes por projeto nos READMEs das pastas).
- **GPU** opcional no Text2D; no Text3D, CUDA com VRAM suficiente é recomendado para tempos aceitáveis.
- Os **pesos dos modelos** (Hugging Face, etc.) têm licenças próprias — consulta os model cards antes de distribuir ou usar em produção.

## Arranque rápido

```bash
# Text2D (imagem)
cd Text2D && ./scripts/setup.sh && source .venv/bin/activate && text2d --help

# Text3D (3D; depende do Text2D como pacote local — ver Text3D/README)
cd ../Text3D
python -m venv .venv && source .venv/bin/activate
pip install -r config/requirements.txt && pip install -e .
text3d --help
```

Instruções completas: [Text2D/README.md](Text2D/README.md) e [Text3D/README.md](Text3D/README.md).

## Licenças

- Código deste repositório: ver [Text2D/LICENSE](Text2D/LICENSE) e [Text3D/LICENSE](Text3D/LICENSE) (ambos MIT nos respetivos pacotes).
- **Modelos pré-treinados** não são necessariamente MIT; obrigação de cumprimento das licenças dos autores (BFL, Disty0, Tencent Hunyuan, etc.).

## Contribuir

- Preferir commits pequenos e mensagens no estilo [Conventional Commits](https://www.conventionalcommits.org/).
- Ignorar ambientes virtuais e caches: o `.gitignore` na raiz alinha-se com os de cada subpasta.
