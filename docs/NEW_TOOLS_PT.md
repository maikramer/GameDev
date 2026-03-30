# Criar uma nova ferramenta no monorepo GameDev

**Idioma:** [English (`NEW_TOOLS.md`)](NEW_TOOLS.md) · Português (esta página)

Este guia descreve os passos para adicionar uma **ferramenta instalável** (CLI Python ou binário Rust) ao monorepo, alinhar o **instalador unificado** (`./install.sh` / `gamedev-install`), o pacote **Shared** (`gamedev-shared`), e — quando fizer sentido — a **integração com GameAssets** e documentação.

---

## 1. Decisões iniciais

| Questão | Notas |
|--------|--------|
| **Nome da pasta** | PascalCase no disco (`MeuTool/`). |
| **Nome do CLI** | Minúsculas, sem espaços (`meutool`), alinhado ao `[project.scripts]` ou `python -m meutool`. |
| **Chave no registry** | Identificador estável em minúsculas (`"meutool"`), usado em `TOOLS` e em `get_tool()`. |
| **Tipo** | `ToolKind.PYTHON` (quase sempre) ou `ToolKind.RUST` (ex.: Materialize). |
| **Python mínimo** | `min_python=(3, 10)` (ou superior se precisares de `bpy`, etc.). |
| **PyTorch / CUDA** | `needs_pytorch` / `needs_cuda` informam o `PythonProjectInstaller` (instalação de torch no venv quando aplicável). |

---

## 2. Estrutura mínima do projecto (Python)

Segue o padrão dos restantes pacotes:

- `MeuTool/pyproject.toml` — `name`, `requires-python`, `[project.scripts]` apontando para `meutool.cli:main` (ou equivalente).
- `MeuTool/src/meutool/` — código importável.
- `MeuTool/config/requirements.txt` — dependências pesadas; referência a **`gamedev-shared @ file:../Shared`** (caminho relativo ao monorepo).
- Opcional: `MeuTool/scripts/setup.sh` (conveniência dev: venv + `pip install -e .`) com cabeçalho a referir [INSTALLING.md](INSTALLING.md).
- Opcional: `MeuTool/scripts/installer.py` — wrapper fino que usa `gamedev_shared.installer.PythonProjectInstaller` (mesma lógica que `gamedev-install meutool`).

O instalador unificado faz sempre **`pip install -e .`** dentro de `MeuTool/.venv` (criado se não existir), usando `config/requirements.txt` quando presente. Garante que o `pyproject.toml` está coerente com esse fluxo.

---

## 3. Registar a ferramenta (`gamedev_shared.installer.registry`)

Editar [`Shared/src/gamedev_shared/installer/registry.py`](../Shared/src/gamedev_shared/installer/registry.py):

1. Adicionar uma entrada em `TOOLS`:

```python
"meutool": ToolSpec(
    name="MeuTool",
    kind=ToolKind.PYTHON,
    folder="MeuTool",
    cli_name="meutool",
    python_module="meutool",
    description="Uma linha clara para `gamedev-install --list`",
    min_python=(3, 10),
    extra_aliases=(),  # ou ("meutool-gen",) para wrappers extra
    needs_pytorch=False,
    needs_cuda=False,
),
```

2. Para **Rust**, usar `ToolKind.RUST`, `cargo_bin_name` (nome do binário em `target/release/`), e pasta com `Cargo.toml`.

3. `ToolSpec.exists()` exige `pyproject.toml` ou `setup.py` (Python) ou `Cargo.toml` (Rust) na pasta do projecto.

**Testes:** actualizar [`Shared/tests/test_registry.py`](../Shared/tests/test_registry.py) com asserts para a nova chave (padrão do monorepo).

---

## 4. Instalador unificado (`unified.py`)

O fluxo por defeito está em [`Shared/src/gamedev_shared/installer/unified.py`](../Shared/src/gamedev_shared/installer/unified.py) (`_ToolPythonInstaller`):

- Verificação de Python (`min_python` do `ToolSpec`).
- `ensure_project_venv` + `install_in_venv` (`pip install -e`, PyTorch se `needs_pytorch`).
- `create_cli_wrappers` + `create_activate_wrapper` + `show_summary`.

**Passos pós-instalação específicos** (como nvdiffrast no Paint3D ou extras no Rigging3D):

- Implementar funções ou classes em módulos dedicados sob `gamedev_shared/installer/` (ex.: [`text3d_extras.py`](../Shared/src/gamedev_shared/installer/text3d_extras.py), [`part3d_extras.py`](../Shared/src/gamedev_shared/installer/part3d_extras.py)).
- Chamar a partir de `_ToolPythonInstaller.run()` quando `self.spec.cli_name == "..."`.
- Se a ferramenta precisar de **novas flags CLI** do `gamedev-install` (ex.: `--skip-env-config` só para Text3D), estender `install_tool()` / `main()` em `unified.py` e documentar no [README raiz](../README.md).

**Rust:** `RustProjectInstaller` em [`rust_installer.py`](../Shared/src/gamedev_shared/installer/rust_installer.py); não misturar lógica Python pesada no mesmo passo.

---

## 5. Shared: `env.py`, subprocessos e logging

Se outras ferramentas (ou GameAssets) precisam de **descobrir o binário** por variável de ambiente:

1. Em [`Shared/src/gamedev_shared/env.py`](../Shared/src/gamedev_shared/env.py):
   - Constante `MEUTOOL_BIN = "MEUTOOL_BIN"`.
   - Entrada em `TOOL_BINS`: `"meutool": MEUTOOL_BIN`.

2. Usar [`gamedev_shared.subprocess_utils.resolve_binary`](../Shared/src/gamedev_shared/subprocess_utils.py) nos projectos que lançam subprocessos.

3. Logging / Rich: reutilizar [`gamedev_shared.logging`](../Shared/src/gamedev_shared/logging.py) e padrões de CLI (`cli_rich`) alinhados aos outros pacotes.

---

## 6. Integração com GameAssets

O GameAssets **não** invoca todas as ferramentas por defeito: só as que o perfil CSV/YAML pede (`image_source`, `generate_audio`, `generate_rig`, etc.).

Se a nova ferramenta for **orquestrada pelo batch** (como `text2d`, `texture2d`, `text3d`):

1. **Variável de ambiente** — seguir o padrão `NOMETOOL_BIN` (maiúsculas) e documentar no [GameAssets/README.md](../GameAssets/README.md) e em `gameassets info`.
2. **Código** — em [`GameAssets/src/gameassets/cli.py`](../GameAssets/src/gameassets/cli.py) (e `runner` se aplicável), usar `resolve_binary("MEUTOOL_BIN", "meutool")` antes de `run_cmd`.
3. **`env.TOOL_BINS`** — incluir `"meutool": MEUTOOL_BIN` para consistência com `get_tool_bin()`.
4. **Skill / Cursor** — actualizar [`GameAssets/.../SKILL.md`](../GameAssets/src/gameassets/cursor_skill/SKILL.md) se a skill mencionar integrações.

Se a ferramenta for **só para uso manual** (sem linha no manifest), basta estar no registry + README; não é obrigatório tocar no GameAssets.

---

## 7. Documentação e raiz do repositório

| Ficheiro | Acção |
|----------|--------|
| [`docs/INSTALLING.md`](INSTALLING.md) | Linha na tabela de ferramentas + comando `./install.sh meutool`. |
| [`README.md`](../README.md) | Tabela «Projetos» e diagrama `GameDev/`; exemplos `./install.sh meutool` se fizer sentido. |
| `MeuTool/README.md` | Secção **Instalação**: oficial (`cd` raiz + `./install.sh meutool`), manual (`venv` + `pip install -e`), atalho local se existir `scripts/installer.py`. |
| [`Shared/README.md`](../Shared/README.md) | Opcional: uma linha na tabela de módulos se adicionares API pública nova. |

---

## 8. CI (`.github/workflows/ci.yml`)

O workflow actual corre **ruff** e **pytest** só para alguns pacotes (Shared, GameAssets, Texture2D, Skymap2D, Rigging3D) e **cargo** para Materialize.

- Se o novo projecto tiver **testes leves** (sem GPU nem downloads gigantes), considera adicionar uma entrada em `matrix.package` com `install_cmd` adequado.
- Ferramentas pesadas (Text3D, Paint3D, etc.) costumam ficar **fora** da matrix por defeito (comentário no topo do `ci.yml`).

---

## 9. Qualidade e estilo

- [`ruff.toml`](../ruff.toml) na raiz: alinhar `src` e exclusões se necessário.
- Licença MIT (ou explícita) em `MeuTool/LICENSE` se ainda não existir.
- Agent Skill opcional: `MeuTool/src/meutool/cursor_skill/SKILL.md` + comando `meutool skill install` se seguires o padrão `gamedev_shared.skill_install`.

---

## 10. Checklist resumido

- [ ] Pasta `MeuTool/` com `pyproject.toml` / `Cargo.toml` válido.
- [ ] Entrada `ToolSpec` em `registry.py` + teste em `Shared/tests/test_registry.py`.
- [ ] Sem passos especiais: nada mais em `unified.py`; com passos especiais: módulo em `gamedev_shared/installer/` + ramo em `_ToolPythonInstaller.run()`.
- [ ] `docs/INSTALLING.md` e README raiz actualizados.
- [ ] `MeuTool/README.md` com instalação oficial primeiro.
- [ ] Se GameAssets: `MEUTOOL_BIN`, `TOOL_BINS`, `resolve_binary`, README GameAssets.
- [ ] `Shared/src/gamedev_shared/env.py` se existir convenção `*_BIN`.
- [ ] CI: ruff + pytest locais; matrix CI se aplicável.
- [ ] `./install.sh --list` mostra a nova ferramenta após checkout.

---

## Ver também

- [INSTALLING_PT.md](INSTALLING_PT.md) — forma oficial de instalar e tabela de ferramentas (PT). [INSTALLING.md](INSTALLING.md) (EN).
- [Shared/README_PT.md](../Shared/README_PT.md) — `gamedev-install`, módulos partilhados.
- [GameAssets/README_PT.md](../GameAssets/README_PT.md) — variáveis `*_BIN` e fluxos de batch.
