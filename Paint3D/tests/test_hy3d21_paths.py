"""Testa resolução do hy3dpaint vendored (sem deps pesadas)."""

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


def test_resolve_hy3dpaint_root_points_at_vendored_code():
    mod = _load_hy3d21_paths()
    root = mod.resolve_hy3dpaint_root()
    assert root.is_dir()
    assert (root / "textureGenPipeline.py").is_file()
    assert (root / "cfgs" / "hunyuan-paint-pbr.yaml").is_file()


def test_default_cfg_yaml_exists():
    mod = _load_hy3d21_paths()
    cfg = mod.default_cfg_yaml()
    assert cfg.is_file()
