"""Resolve hy3dpaint sem importar o pacote ``paint3d`` completo (evita deps pesadas na recolha)."""

from __future__ import annotations

import importlib.util
from pathlib import Path


def _load_hy3d21_paths():
    src = Path(__file__).resolve().parents[1] / "src" / "paint3d" / "hy3d21_paths.py"
    spec = importlib.util.spec_from_file_location("_hy3d21_paths_iso", src)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_resolve_hy3dpaint_root_points_at_texture_gen_pipeline():
    mod = _load_hy3d21_paths()
    root = mod.resolve_hy3dpaint_root()
    assert root.is_dir()
    assert (root / "textureGenPipeline.py").is_file()
    assert (root / "cfgs" / "hunyuan-paint-pbr.yaml").is_file()
