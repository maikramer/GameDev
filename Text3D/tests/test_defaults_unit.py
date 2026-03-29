"""Contratos dos defaults e presets Hunyuan (sem GPU)."""

from __future__ import annotations

import text3d.defaults as d


def test_default_texture_constant_is_true() -> None:
    assert d.DEFAULT_TEXTURE is True


def test_get_default_texture_respects_env(monkeypatch) -> None:
    monkeypatch.setenv("TEXT3D_DEFAULT_TEXTURE", "0")
    assert d.get_default_texture() is False
    monkeypatch.setenv("TEXT3D_DEFAULT_TEXTURE", "1")
    assert d.get_default_texture() is True
    monkeypatch.delenv("TEXT3D_DEFAULT_TEXTURE", raising=False)
    assert d.get_default_texture() is True


def test_preset_hunyuan_keys() -> None:
    assert set(d.PRESET_HUNYUAN.keys()) == {"fast", "balanced", "hq"}


def test_balanced_matches_defaults() -> None:
    b = d.PRESET_HUNYUAN["balanced"]
    assert b["steps"] == d.DEFAULT_HY_STEPS
    assert b["octree"] == d.DEFAULT_OCTREE_RESOLUTION
    assert b["chunks"] == d.DEFAULT_NUM_CHUNKS


def test_fast_is_lighter_than_balanced() -> None:
    f = d.PRESET_HUNYUAN["fast"]
    bal = d.PRESET_HUNYUAN["balanced"]
    assert f["octree"] <= bal["octree"]
    assert f["chunks"] <= bal["chunks"]


def test_hq_reference_constants() -> None:
    hq = d.PRESET_HUNYUAN["hq"]
    assert hq["steps"] == d.HUNYUAN_HQ_STEPS
    assert hq["octree"] == d.HUNYUAN_HQ_OCTREE
    assert hq["chunks"] == d.HUNYUAN_HQ_NUM_CHUNKS
