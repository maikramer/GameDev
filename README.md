# GameDev

[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](Text2D/LICENSE)

Monorepo com ferramentas de **texto para imagem**, **texto para 3D** e **texto para áudio**, partilhando a mesma base de scripts e documentação.

## Projetos

| Pasta | Descrição |
|-------|-----------|
| [**Shared**](Shared/) | Biblioteca partilhada (`gamedev-shared`): logging, GPU, subprocess, instaladores, CLI. |
| [**Text2D**](Text2D/) | CLI **text-to-image** com FLUX (quantização SDNQ), orientada a GPU modesta. |
| [**Text3D**](Text3D/) | Pipeline **text-to-3D**: imagem 2D (via Text2D) → mesh GLB com Hunyuan3D; pintura opcional. |
| [**GameAssets**](GameAssets/) | **Batch de prompts/assets**: perfil + CSV → `text2d` ou `texture2d` (por perfil ou por linha) + opcional `text3d` / Materialize. |
| [**Texture2D**](Texture2D/) | **Texturas 2D seamless** (tileable) via HF Inference API — sem GPU local. |
| [**Skymap2D**](Skymap2D/) | **Skymaps equirectangular 360°** via HF Inference API — skyboxes para game dev, sem GPU local. |
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
  Skymap2D/          ← skymap2d (pip) — depende de Shared; skymaps equirectangular via HF
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
./install.sh skymap2d                   # Skymap2D (skymaps equirectangular; sem GPU)
./install.sh text2sound                 # Text2Sound (requer CUDA; instala PyTorch)
./install.sh all                        # Instalar tudo

# Windows PowerShell
.\install.ps1 materialize
.\install.ps1 text2d
.\install.ps1 texture2d
.\install.ps1 skymap2d
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

# 4. GameAssets (batch; Text2D/Text3D na PATH ou TEXT2D_BIN/TEXT3D_BIN; Texture2D opcional TEXTURE2D_BIN; Materialize opcional MATERIALIZE_BIN)
cd ../GameAssets && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && gameassets --help

# 5. Texture2D (texturas seamless via HF API; sem PyTorch local)
cd ../Texture2D && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && texture2d --help

# 6. Skymap2D (skymaps equirectangular 360° via HF API; sem PyTorch local)
cd ../Skymap2D && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && skymap2d --help

# 7. Text2Sound (text-to-audio; Stable Audio Open 1.0; requer CUDA)
cd ../Text2Sound && chmod +x scripts/setup.sh && ./scripts/setup.sh && source .venv/bin/activate && text2sound --help

# 8. Materialize (Rust — requer cargo)
cd ../Materialize && ./install.sh
```

Instruções completas: [Shared/README.md](Shared/README.md), [Text2D/README.md](Text2D/README.md), [Text3D/README.md](Text3D/README.md), [GameAssets/README.md](GameAssets/README.md), [Texture2D/README.md](Texture2D/README.md), [Skymap2D/README.md](Skymap2D/README.md) e [Text2Sound/README.md](Text2Sound/README.md).

## Licenças

| Componente | Licença | Nota |
|-----------|---------|------|
| Código do monorepo (Text2D, Text3D, Texture2D, Skymap2D, GameAssets, Shared) | MIT | Ver `LICENSE` em cada pasta |
| Materialize CLI (Rust) | MIT | [Materialize/LICENSE](Materialize/LICENSE) |
| FLUX.2 Klein (pesos) | Consultar model card | [Disty0/FLUX.2-klein-4B-SDNQ](https://huggingface.co/Disty0/FLUX.2-klein-4B-SDNQ-4bit-dynamic) |
| Hunyuan3D-2mini (pesos) | Tencent Community License | [tencent/Hunyuan3D-2mini](https://huggingface.co/tencent/Hunyuan3D-2mini) |
| Stable Audio Open 1.0 (pesos) | Consultar model card | [stabilityai/stable-audio-open-1.0](https://huggingface.co/stabilityai/stable-audio-open-1.0) |
| Flux-Seamless-Texture-LoRA (pesos) | Consultar model card | [gokaygokay/Flux-Seamless-Texture-LoRA](https://huggingface.co/gokaygokay/Flux-Seamless-Texture-LoRA) |
| Flux-LoRA-Equirectangular-v3 (pesos) | Consultar model card | [MultiTrickFox/Flux-LoRA-Equirectangular-v3](https://huggingface.co/MultiTrickFox/Flux-LoRA-Equirectangular-v3) |

> **Atenção:** os pesos dos modelos pré-treinados têm licenças próprias — consulta os model cards antes de distribuir ou usar em produção.

## Variáveis de Ambiente

O monorepo usa variáveis de ambiente para localizar binários e configurar comportamento:

| Variável | Usada por | Descrição |
|----------|-----------|-----------|
| `TEXT2D_BIN` | GameAssets | Caminho para o binário `text2d` (se não estiver no `PATH`) |
| `TEXT3D_BIN` | GameAssets | Caminho para o binário `text3d` |
| `TEXTURE2D_BIN` | GameAssets | Caminho para o binário `texture2d` |
| `TEXT2SOUND_BIN` | GameAssets | Caminho para o binário `text2sound` |
| `MATERIALIZE_BIN` | GameAssets, Text3D | Caminho para o binário `materialize` |
| `TEXT2D_MODEL_ID` | Text2D | Override do modelo HF para Text2D |
| `TEXTURE2D_MODEL_ID` | Texture2D | Override do modelo HF para Texture2D |
| `SKYMAP2D_MODEL_ID` | Skymap2D | Override do modelo HF para Skymap2D |
| `HF_TOKEN` | Text2Sound, Texture2D, Skymap2D | Token Hugging Face para APIs autenticadas |
| `HF_HOME` | Todos (Python) | Diretório de cache Hugging Face (defeito: `~/.cache/huggingface`) |
| `PYTORCH_CUDA_ALLOC_CONF` | Text2D, Text3D, GameAssets | Configuração de alocação CUDA (auto-definida se vazia) |
| `TEXT3D_ALLOW_SHARED_GPU` | Text3D | Permitir GPU partilhada com outros processos |
| `TEXT3D_GPU_KILL_OTHERS` | Text3D | Controlar terminação de processos GPU concorrentes |
| `TEXT3D_EXPORT_ROTATION_X_DEG` | Text3D | Rotação X ao exportar mesh (graus) |

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
- Cada ferramenta tem o seu `pyproject.toml` com `[project.optional-dependencies] dev` — instala com `pip install -e ".[dev]"` antes de correr testes.
- **Documentação** por ferramenta: mantém o `README.md` e, quando existir, a pasta `docs/` atualizada.
