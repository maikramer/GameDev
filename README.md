# GameDev

Monorepo com ferramentas de **texto para imagem** e **texto para 3D**, partilhando a mesma base de scripts e documentação.

## Projetos

| Pasta | Descrição |
|-------|-----------|
| [**Shared**](Shared/) | Biblioteca partilhada (`gamedev-shared`): logging, GPU, subprocess, instaladores, CLI. |
| [**Text2D**](Text2D/) | CLI **text-to-image** com FLUX (quantização SDNQ), orientada a GPU modesta. |
| [**Text3D**](Text3D/) | Pipeline **text-to-3D**: imagem 2D (via Text2D) → mesh GLB com Hunyuan3D; pintura opcional. |
| [**GameAssets**](GameAssets/) | **Batch de prompts/assets**: perfil de jogo + estilo + CSV → `text2d` / `text3d` por subprocess. |
| [**Texture2D**](Texture2D/) | **Texturas 2D seamless** (tileable) via HF Inference API — sem GPU local. |
| [**Materialize**](Materialize/) | CLI **PBR maps** (Rust/wgpu): gera normal, AO, metallic, smoothness a partir de textura difusa. |

Cada projeto tem o seu próprio `README`, `setup`, requisitos e licença.

## Arquitectura

```
GameDev/
  Shared/           ← gamedev-shared (pip): logging, GPU, subprocess, env, instaladores
  Text2D/           ← text2d (pip) — depende de Shared
  Text3D/           ← text3d (pip) — depende de Shared + Text2D
  GameAssets/        ← gameassets (pip) — depende de Shared; chama text2d/text3d via subprocess
  Texture2D/         ← texture2d (pip) — depende de Shared; inferência HF na cloud
  Materialize/       ← materialize-cli (cargo) — instalador Python usa Shared
```

## Requisitos gerais

- **Python** 3.10 ou superior (detalhes por projeto nos READMEs das pastas).
- **GPU** opcional no Text2D; no Text3D, CUDA com VRAM suficiente é recomendado para tempos aceitáveis.
- Os **pesos dos modelos** (Hugging Face, etc.) têm licenças próprias — consulta os model cards antes de distribuir ou usar em produção.

## Arranque rápido

### Instalador unificado (recomendado)

O monorepo inclui um instalador unificado que instala qualquer ferramenta automaticamente:

```bash
# Linux/macOS
./install.sh --list                     # Listar ferramentas disponíveis
./install.sh materialize                # Instalar Materialize (Rust)
./install.sh text2d --use-venv          # Instalar Text2D no venv
./install.sh texture2d --use-venv       # Instalar Texture2D no venv
./install.sh all                        # Instalar tudo

# Windows PowerShell
.\install.ps1 materialize
.\install.ps1 text2d --use-venv
.\install.ps1 texture2d --use-venv
.\install.ps1 all

# Windows CMD
install.bat materialize
```

Opções do instalador unificado:

| Opção | Descrição |
|-------|-----------|
| `--action {install,uninstall,reinstall}` | Acção a executar (default: install) |
| `--use-venv` | Usar .venv existente (projectos Python) |
| `--skip-deps` | Não instalar dependências de sistema |
| `--skip-models` | Não configurar modelos/pesos |
| `--force` | Forçar reinstalação |
| `--prefix PATH` | Prefixo de instalação (default: ~/.local) |
| `--python CMD` | Comando Python (default: python3) |
| `--list` | Listar ferramentas disponíveis |

### Instalação manual

```bash
# 1. Instalar Shared (obrigatório para todos os projectos Python)
cd Shared && pip install -e . && cd ..

# 2. Text2D (imagem)
cd Text2D && ./scripts/setup.sh && source .venv/bin/activate && text2d --help

# 3. Text3D (3D; depende do Text2D como pacote local — ver Text3D/README)
cd ../Text3D
python -m venv .venv && source .venv/bin/activate
pip install -r config/requirements.txt && pip install -e .
text3d --help

# 4. GameAssets (batch; para GPU instala Text2D/Text3D e PATH ou TEXT2D_BIN/TEXT3D_BIN)
cd ../GameAssets && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && gameassets --help

# 5. Texture2D (texturas seamless via HF API; sem PyTorch local)
cd ../Texture2D && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && texture2d --help

# 6. Materialize (Rust — requer cargo)
cd ../Materialize && ./install.sh
```

Instruções completas: [Shared/README.md](Shared/README.md), [Text2D/README.md](Text2D/README.md), [Text3D/README.md](Text3D/README.md), [GameAssets/README.md](GameAssets/README.md) e [Texture2D/README.md](Texture2D/README.md).

## Licenças

- Código deste repositório: ver [Text2D/LICENSE](Text2D/LICENSE), [Text3D/LICENSE](Text3D/LICENSE) e [Texture2D/LICENSE](Texture2D/LICENSE) (MIT nos respetivos pacotes).
- **Modelos pré-treinados** não são necessariamente MIT; obrigação de cumprimento das licenças dos autores (BFL, Disty0, Tencent Hunyuan, etc.).

## Contribuir

- Preferir commits pequenos e mensagens no estilo [Conventional Commits](https://www.conventionalcommits.org/).
- Ignorar ambientes virtuais e caches: o `.gitignore` na raiz alinha-se com os de cada subpasta.
