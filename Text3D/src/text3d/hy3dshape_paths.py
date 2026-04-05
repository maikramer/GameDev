"""
Resolve o código ``hy3dshape`` vendored em ``text3d.hy3dshape``.

O código vem de https://github.com/Tencent-Hunyuan/Hunyuan3D-2.1 (pasta ``hy3dshape/``),
integrado directamente no pacote — sem submodule.

Os **modelos** (pesos) são descarregados sob demanda via ``huggingface_hub`` a partir
de ``tencent/Hunyuan3D-2.1`` (subpasta ``hunyuan3d-dit-v2-1``).

O import principal é relativo (``from .hy3dshape.pipelines import ...``) dentro do
pacote ``text3d``. Esta função utilitária garante que ``import hy3dshape`` absoluto
também funciona (necessário para alguns módulos internos do upstream que usam
import absoluto, ex.: ``utils/trainings/mesh_log_callback.py``).
"""

from __future__ import annotations

import sys
from pathlib import Path


def resolve_hy3dshape_root() -> Path:
    """Return the vendored ``hy3dshape`` directory inside this package."""
    return Path(__file__).resolve().parent / "hy3dshape"


def ensure_hy3dshape_on_path() -> Path:
    """Make top-level ``import hy3dshape`` resolve to the vendored copy.

    Adds the parent of the vendored ``hy3dshape/`` package to ``sys.path``
    so that absolute imports like ``from hy3dshape.pipelines import ...``
    work. Idempotent — safe to call multiple times.
    """
    parent = str(resolve_hy3dshape_root().parent)
    if parent not in sys.path:
        sys.path.insert(0, parent)
    return resolve_hy3dshape_root()
