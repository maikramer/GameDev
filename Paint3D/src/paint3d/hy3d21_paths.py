"""
Resolve the Hunyuan3D-2.1 ``hy3dpaint`` tree (submodule or ``HUNYUAN3D_21_ROOT``).
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

_ENV_ROOT = "HUNYUAN3D_21_ROOT"


def resolve_hy3dpaint_root() -> Path:
    """
    Return the directory that contains ``textureGenPipeline.py`` (i.e. ``hy3dpaint``).

    Resolution order:
    1. ``HUNYUAN3D_21_ROOT`` pointing to repo root (``.../Hunyuan3D-2.1``) or to ``hy3dpaint`` directly.
    2. Monorepo submodule ``<GameDev>/third_party/Hunyuan3D-2.1/hy3dpaint`` relative to this package.
    """
    raw = os.environ.get(_ENV_ROOT, "").strip()
    if raw:
        p = Path(raw).expanduser().resolve()
        if p.name == "hy3dpaint" and p.is_dir():
            return p
        hy = p / "hy3dpaint"
        if hy.is_dir():
            return hy.resolve()
        raise FileNotFoundError(
            f"{_ENV_ROOT}={raw!r} não contém a pasta hy3dpaint. "
            "Define o caminho para a raiz do clone Hunyuan3D-2.1 ou para …/hy3dpaint."
        )

    here = Path(__file__).resolve().parent  # .../Paint3D/src/paint3d
    # paint3d → src → Paint3D (project) → GameDev (monorepo)
    game_dev = here.parent.parent.parent
    sub = game_dev / "third_party" / "Hunyuan3D-2.1" / "hy3dpaint"
    if sub.is_dir() and (sub / "textureGenPipeline.py").is_file():
        return sub.resolve()

    raise FileNotFoundError(
        "Código Hunyuan3D-2.1 (hy3dpaint) não encontrado.\n"
        "  • Inicializa o submodule: git submodule update --init third_party/Hunyuan3D-2.1\n"
        f"  • Ou define {_ENV_ROOT}=/caminho/para/Hunyuan3D-2.1 (ou …/hy3dpaint)"
    )


def ensure_hy3dpaint_on_path() -> Path:
    """Insert ``hy3dpaint`` at the front of ``sys.path`` and return its path."""
    root = resolve_hy3dpaint_root()
    s = str(root)
    if s not in sys.path:
        sys.path.insert(0, s)
    return root


def default_realesrgan_ckpt(hy3dpaint_root: Path) -> Path:
    return hy3dpaint_root / "ckpt" / "RealESRGAN_x4plus.pth"
