# Installing in the GameDev monorepo

**Language:** English · [Português (`INSTALLING_PT.md`)](INSTALLING_PT.md)

## Official method

At the **repository root** (folder containing `Shared/`, `install.sh`, `.git`):

| Platform | Command |
|----------|---------|
| Linux / macOS | `./install.sh <tool>` |
| Windows PowerShell | `.\install.ps1 <tool>` |
| Windows CMD | `install.bat <tool>` |

With `gamedev-shared` installed (or `PYTHONPATH` pointing to `Shared/src`):

```bash
gamedev-install --list
gamedev-install text2d
```

**Installer prerequisites:** Python **3.10+**, `pip`, and dependencies in [`Shared/config/requirements.txt`](../Shared/config/requirements.txt) (e.g. Rich), installed automatically by [`install.sh`](../install.sh) before loading the unified module.

Useful variable: `PYTHON_CMD` — interpreter to use (default `python3`, or `python` on Windows in the scripts).

---

## Registered tools

| `./install.sh …` command | Folder | Type | Min Python | Notes |
|--------------------------|--------|------|------------|-------|
| `text2d` | Text2D | Python | 3.10 | PyTorch/CUDA recommended |
| `text3d` | Text3D | Python | 3.8 | Depends on Text2D; nvdiffrast after venv |
| `gameassets` | GameAssets | Python | 3.10 | No PyTorch in package; `batch` orchestrates CLIs on PATH (e.g. Part3D with `--with-parts`) |
| `text2sound` | Text2Sound | Python | 3.10 | PyTorch/CUDA |
| `texture2d` | Texture2D | Python | 3.10 | HF API; local GPU optional |
| `skymap2d` | Skymap2D | Python | 3.10 | HF API |
| `rigging3d` | Rigging3D | Python | 3.11 | UniRig; inference extras **always** via unified installer |
| `animator3d` | Animator3D | Python | 3.13 | `bpy` 5.1 |
| `part3d` | Part3D | Python | 3.10 | torch-scatter/cluster after venv |
| `paint3d` | Paint3D | Python | 3.10 | nvdiffrast after venv |
| `materialize` | Materialize | Rust | — | Needs `cargo`; binary in `~/.local/bin` by default |

Install everything present in the checkout: `./install.sh all`.

Technical details: [`Shared/src/gamedev_shared/installer/registry.py`](../Shared/src/gamedev_shared/installer/registry.py).

---

## Do not confuse two `install.sh` files

| File | Role |
|------|------|
| **`GameDev/install.sh`** (root) | **Unified** installer for any tool (`gamedev_shared.installer.unified`). |
| **`<Project>/scripts/install.sh`** | Local shortcut that only calls **that** project’s `scripts/installer.py` (same logic as unified when equivalent). **Not** the root script. |

Prefer `./install.sh <name>` **from the repo root**. The `scripts/` wrapper exists for people already inside the project folder.

Text2D, Text3D, and Texture2D also expose `scripts/run_installer.sh` (implementation); `scripts/install.sh` delegates to it for compatibility.

---

## Manual install / CI

For pipelines or debugging, you can create a `venv` and `pip install -e` in each folder; see per-project READMEs and “Manual” sections or `scripts/setup.sh` (dev convenience: creates `.venv` and editable install — **does not** replace the “official install” contract documented above).

---

## Documentation per tool

- **[Adding a new tool to the monorepo](NEW_TOOLS.md)** — registry, unified installer, Shared, GameAssets, CI, checklist.
- [Shared/README.md](../Shared/README.md) — `gamedev-shared`, `gamedev-install`
- [Text2D/README.md](../Text2D/README.md), [Text3D/README.md](../Text3D/README.md), [GameAssets/README.md](../GameAssets/README.md), [Texture2D/README.md](../Texture2D/README.md), [Skymap2D/README.md](../Skymap2D/README.md), [Text2Sound/README.md](../Text2Sound/README.md), [Rigging3D/README.md](../Rigging3D/README.md), [Animator3D/README.md](../Animator3D/README.md), [Part3D/README.md](../Part3D/README.md), [Paint3D/README.md](../Paint3D/README.md), [Materialize/README.md](../Materialize/README.md)
