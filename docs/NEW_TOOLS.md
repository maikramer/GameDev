# Adding a new tool to the GameDev monorepo

**Language:** English · [Português (`NEW_TOOLS_PT.md`)](NEW_TOOLS_PT.md)

This guide describes how to add an **installable tool** (Python CLI or Rust binary) to the monorepo, align the **unified installer** (`./install.sh` / `gamedev-install`), the **Shared** package (`gamedev-shared`), and — when appropriate — **GameAssets integration** and documentation.

---

## 1. Initial decisions

| Question | Notes |
|----------|-------|
| **Folder name** | PascalCase on disk (`MyTool/`). |
| **CLI name** | Lowercase, no spaces (`mytool`), aligned with `[project.scripts]` or `python -m mytool`. |
| **Registry key** | Stable lowercase id (`"mytool"`), used in `TOOLS` and `get_tool()`. |
| **Kind** | `ToolKind.PYTHON` (almost always) or `ToolKind.RUST` (e.g. Materialize). |
| **Minimum Python** | `min_python=(3, 10)` (or higher if you need `bpy`, etc.). |
| **PyTorch / CUDA** | `needs_pytorch` / `needs_cuda` inform `PythonProjectInstaller` (installs torch in the venv when applicable). |

---

## 2. Minimum project layout (Python)

Follow the pattern of other packages:

- `MyTool/pyproject.toml` — `name`, `requires-python`, `[project.scripts]` pointing to `mytool.cli:main` (or equivalent).
- `MyTool/src/mytool/` — importable code.
- `MyTool/config/requirements.txt` — heavy dependencies; reference **`gamedev-shared @ file:../Shared`** (path relative to monorepo).
- Optional: `MyTool/scripts/setup.sh` (dev convenience: venv + `pip install -e .`) with a header pointing to [INSTALLING.md](INSTALLING.md).
- Optional: `MyTool/scripts/installer.py` — thin wrapper using `gamedev_shared.installer.PythonProjectInstaller` (same logic as `gamedev-install mytool`).

The unified installer always runs **`pip install -e .`** inside `MyTool/.venv` (created if missing), using `config/requirements.txt` when present. Ensure `pyproject.toml` matches that flow.

---

## 3. Register the tool (`gamedev_shared.installer.registry`)

Edit [`Shared/src/gamedev_shared/installer/registry.py`](../Shared/src/gamedev_shared/installer/registry.py):

1. Add an entry to `TOOLS`:

```python
"mytool": ToolSpec(
    name="MyTool",
    kind=ToolKind.PYTHON,
    folder="MyTool",
    cli_name="mytool",
    python_module="mytool",
    description="One line for `gamedev-install --list`",
    min_python=(3, 10),
    extra_aliases=(),  # or ("mytool-gen",) for extra wrappers
    needs_pytorch=False,
    needs_cuda=False,
),
```

2. For **Rust**, use `ToolKind.RUST`, `cargo_bin_name` (binary name in `target/release/`), and a folder with `Cargo.toml`.

3. `ToolSpec.exists()` requires `pyproject.toml` or `setup.py` (Python) or `Cargo.toml` (Rust) in the project folder.

**Tests:** update [`Shared/tests/test_registry.py`](../Shared/tests/test_registry.py) with asserts for the new key (monorepo convention).

---

## 4. Unified installer (`unified.py`)

Default flow is in [`Shared/src/gamedev_shared/installer/unified.py`](../Shared/src/gamedev_shared/installer/unified.py) (`_ToolPythonInstaller`):

- Python version check (`min_python` from `ToolSpec`).
- `ensure_project_venv` + `install_in_venv` (`pip install -e`, PyTorch if `needs_pytorch`).
- `create_cli_wrappers` + `create_activate_wrapper` + `show_summary`.

**Post-install steps** (like nvdiffrast in Paint3D or extras in Rigging3D):

- Implement functions or classes in dedicated modules under `gamedev_shared/installer/` (e.g. [`text3d_extras.py`](../Shared/src/gamedev_shared/installer/text3d_extras.py), [`part3d_extras.py`](../Shared/src/gamedev_shared/installer/part3d_extras.py)).
- Call from `_ToolPythonInstaller.run()` when `self.spec.cli_name == "..."`.
- If the tool needs **new `gamedev-install` flags** (e.g. `--skip-env-config` only for Text3D), extend `install_tool()` / `main()` in `unified.py` and document in the [root README](../README.md).

**Rust:** `RustProjectInstaller` in [`rust_installer.py`](../Shared/src/gamedev_shared/installer/rust_installer.py); keep heavy Python logic out of that step.

---

## 5. Shared: `env.py`, subprocess, logging

If other tools (or GameAssets) need to **discover the binary** via environment:

1. In [`Shared/src/gamedev_shared/env.py`](../Shared/src/gamedev_shared/env.py):
   - Constant `MEUTOOL_BIN = "MEUTOOL_BIN"`.
   - Entry in `TOOL_BINS`: `"mytool": MEUTOOL_BIN`.

2. Use [`gamedev_shared.subprocess_utils.resolve_binary`](../Shared/src/gamedev_shared/subprocess_utils.py) in projects that spawn subprocesses.

3. Logging / Rich: reuse [`gamedev_shared.logging`](../Shared/src/gamedev_shared/logging.py) and CLI patterns (`cli_rich`) aligned with other packages.

---

## 6. GameAssets integration

GameAssets does **not** invoke every tool by default — only those requested by the CSV/YAML profile (`image_source`, `generate_audio`, `generate_rig`, etc.).

If the new tool is **orchestrated by the batch** (like `text2d`, `texture2d`, `text3d`):

1. **Environment variable** — follow `TOOLNAME_BIN` (uppercase) and document in [GameAssets/README.md](../GameAssets/README.md) and `gameassets info`.
2. **Code** — in [`GameAssets/src/gameassets/cli.py`](../GameAssets/src/gameassets/cli.py) (and `runner` if needed), use `resolve_binary("MEUTOOL_BIN", "mytool")` before `run_cmd`.
3. **`env.TOOL_BINS`** — include `"mytool": MEUTOOL_BIN` for consistency with `get_tool_bin()`.
4. **Skill / Cursor** — update [`GameAssets/.../SKILL.md`](../GameAssets/src/gameassets/cursor_skill/SKILL.md) if the skill mentions integrations.

If the tool is **manual only** (no manifest line), registry + README is enough; GameAssets changes are optional.

---

## 7. Documentation and repo root

| File | Action |
|------|--------|
| [`docs/INSTALLING.md`](INSTALLING.md) | Row in tool table + `./install.sh mytool` example. |
| [`README.md`](../README.md) | “Projects” table and `GameDev/` diagram; `./install.sh mytool` examples if useful. |
| `MyTool/README.md` | **Install** section: official (`cd` to root + `./install.sh mytool`), manual (`venv` + `pip install -e`), local shortcut if `scripts/installer.py` exists. |
| [`Shared/README.md`](../Shared/README.md) | Optional: one line in the module table if you add new public API. |

---

## 8. CI (`.github/workflows/ci.yml`)

The workflow runs **ruff** and **pytest** for selected packages (Shared, GameAssets, Texture2D, Skymap2D, Rigging3D) and **cargo** for Materialize.

- If the new project has **light tests** (no GPU or huge downloads), consider adding a `matrix.package` entry with a suitable `install_cmd`.
- Heavy tools (Text3D, Paint3D, etc.) usually stay **out** of the matrix by default (see comment at top of `ci.yml`).

---

## 9. Quality and style

- [`ruff.toml`](../ruff.toml) at root: align `src` paths and exclusions if needed.
- MIT license (or explicit) in `MyTool/LICENSE` if missing.
- Optional Agent Skill: `MyTool/src/mytool/cursor_skill/SKILL.md` + `mytool skill install` if you follow `gamedev_shared.skill_install`.

---

## 10. Short checklist

- [ ] `MyTool/` folder with valid `pyproject.toml` / `Cargo.toml`.
- [ ] `ToolSpec` entry in `registry.py` + test in `Shared/tests/test_registry.py`.
- [ ] No special steps: nothing else in `unified.py`; with special steps: module under `gamedev_shared/installer/` + branch in `_ToolPythonInstaller.run()`.
- [ ] `docs/INSTALLING.md` and root README updated.
- [ ] `MyTool/README.md` with official install first.
- [ ] If GameAssets: `MEUTOOL_BIN`, `TOOL_BINS`, `resolve_binary`, GameAssets README.
- [ ] `Shared/src/gamedev_shared/env.py` if using `*_BIN` convention.
- [ ] CI: local ruff + pytest; CI matrix if applicable.
- [ ] `./install.sh --list` shows the new tool after checkout.

---

## See also

- [INSTALLING.md](INSTALLING.md) — official install method and tool table.
- [Shared/README.md](../Shared/README.md) — `gamedev-install`, shared modules.
- [GameAssets/README.md](../GameAssets/README.md) — `*_BIN` variables and batch flows.
