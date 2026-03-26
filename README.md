# GameDev

Monorepo com ferramentas de **texto para imagem**, **texto para 3D** e **texto para áudio**, partilhando a mesma base de scripts e documentação.

## Projetos

| Pasta | Descrição |
|-------|-----------|
| [**Shared**](Shared/) | Biblioteca partilhada (`gamedev-shared`): logging, GPU, subprocess, instaladores, CLI. |
| [**Text2D**](Text2D/) | CLI **text-to-image** com FLUX (quantização SDNQ), orientada a GPU modesta. |
| [**Text3D**](Text3D/) | Pipeline **text-to-3D**: imagem 2D (via Text2D) → mesh GLB com Hunyuan3D; pintura opcional. |
| [**GameAssets**](GameAssets/) | **Batch de prompts/assets**: perfil + CSV → `text2d` ou `texture2d` (por perfil ou por linha) + opcional `text3d` / Materialize. |
| [**Texture2D**](Texture2D/) | **Texturas 2D seamless** (tileable) via HF Inference API — sem GPU local. |
| [**Text2Sound**](Text2Sound/) | CLI **text-to-audio** com Stable Audio Open 1.0: áudio estéreo 44.1 kHz, presets para game dev. |
| [**Materialize**](Materialize/) | CLI **PBR maps** (Rust/wgpu): gera normal, AO, metallic, smoothness a partir de textura difusa. |

Cada projeto tem o seu próprio `README`, `setup`, requisitos e licença.

## Arquitectura

```
GameDev/
  Shared/           ← gamedev-shared (pip): logging, GPU, subprocess, env, instaladores
  Text2D/           ← text2d (pip) — depende de Shared
  Text3D/           ← text3d (pip) — depende de Shared + Text2D
  GameAssets/        ← gameassets (pip) — depende de Shared; chama text2d/texture2d/text3d via subprocess
  Texture2D/         ← texture2d (pip) — depende de Shared; inferência HF na cloud
  Text2Sound/        ← text2sound (pip) — depende de Shared; Stable Audio Open 1.0
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
./install.sh text2d                     # Cria Text2D/.venv se necessário; instala no venv do projecto
./install.sh texture2d                  # Idem (Texture2D/.venv)
./install.sh text2sound                 # Text2Sound (requer CUDA; instala PyTorch)
./install.sh all                        # Instalar tudo

# Windows PowerShell
.\install.ps1 materialize
.\install.ps1 text2d
.\install.ps1 texture2d
.\install.ps1 text2sound
.\install.ps1 all

# Windows CMD
install.bat materialize
```

Opções do instalador unificado:

| Opção | Descrição |
|-------|-----------|
| `--action {install,uninstall,reinstall}` | Acção a executar (default: install) |
| `--use-venv` | Legado (opcional); o instalador **cria** sempre `projecto/.venv` se não existir e instala aí |
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

# 4. GameAssets (batch; Text2D/Text3D na PATH ou TEXT2D_BIN/TEXT3D_BIN; Texture2D opcional TEXTURE2D_BIN)
cd ../GameAssets && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && gameassets --help

# 5. Texture2D (texturas seamless via HF API; sem PyTorch local)
cd ../Texture2D && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && texture2d --help

# 6. Text2Sound (text-to-audio; Stable Audio Open 1.0; requer CUDA)
cd ../Text2Sound && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && text2sound --help

# 7. Materialize (Rust — requer cargo)
cd ../Materialize && ./install.sh
```

Instruções completas: [Shared/README.md](Shared/README.md), [Text2D/README.md](Text2D/README.md), [Text3D/README.md](Text3D/README.md), [GameAssets/README.md](GameAssets/README.md), [Texture2D/README.md](Texture2D/README.md) e [Text2Sound/README.md](Text2Sound/README.md).

## Licenças

- Código deste repositório: ver [Text2D/LICENSE](Text2D/LICENSE), [Text3D/LICENSE](Text3D/LICENSE) e [Texture2D/LICENSE](Texture2D/LICENSE) (MIT nos respetivos pacotes).
- **Modelos pré-treinados** não são necessariamente MIT; obrigação de cumprimento das licenças dos autores (BFL, Disty0, Tencent Hunyuan, etc.).

## Desenvolvimento

### Ferramentas de qualidade

O monorepo usa ferramentas centralizadas para lint, formatação, testes e type-checking:

| Ferramenta | Âmbito | Config |
|------------|--------|--------|
| [**Ruff**](https://docs.astral.sh/ruff/) | Lint + format (Python) | `ruff.toml` (raiz) |
| [**MyPy**](https://mypy.readthedocs.io/) | Type-checking (Python) | `mypy.ini` (raiz) |
| [**Pytest**](https://pytest.org/) + **pytest-cov** | Testes + cobertura | `pyproject.toml` por pacote |
| [**Cargo Clippy**](https://doc.rust-lang.org/clippy/) | Lint (Rust) | via Makefile |
| [**Pre-commit**](https://pre-commit.com/) | Hooks de pré-commit | `.pre-commit-config.yaml` |
| [**GitHub Actions**](https://github.com/features/actions) | CI (lint + test + clippy) | `.github/workflows/ci.yml` |

### Makefile (GNU Make)

```bash
make help            # Listar todos os targets
make lint            # Ruff check + Cargo clippy
make fmt             # Ruff format + Cargo fmt
make fmt-check       # Verificar formatação sem alterar
make test            # Pytest em todos os pacotes + Cargo test
make test-shared     # Pytest só no Shared
make test-text2d     # Pytest só no Text2D
make typecheck       # MyPy no Shared/src
make check           # lint + fmt-check + typecheck + test (CI completo)
make clean           # Remover __pycache__, caches, builds
make install-hooks   # Instalar pre-commit hooks
```

> **Windows:** requer GNU Make (via Git Bash, MSYS2 ou WSL).

### Setup de desenvolvimento

```bash
# 1. Instalar pre-commit hooks
pip install pre-commit
make install-hooks

# 2. Instalar dependências de dev num pacote (exemplo: Shared)
cd Shared && pip install -e ".[dev]" && cd ..

# 3. Correr testes
make test-shared

# 4. Lint e format
make lint
make fmt
```

### pyproject.toml

Cada pacote Python tem um `pyproject.toml` (PEP 621) com metadata, dependências e config do pytest.
Os ficheiros `setup.py` existentes permanecem para compatibilidade com instaladores legados.

## Contribuir

- Preferir commits pequenos e mensagens no estilo [Conventional Commits](https://www.conventionalcommits.org/).
- Ignorar ambientes virtuais e caches: o `.gitignore` na raiz alinha-se com os de cada subpasta.
- Correr `make check` antes de submeter PRs.
