"""Testes unitários do módulo generator (sem carregar modelo)."""

from __future__ import annotations

import pytest

from text2d.generator import default_model_id


def test_default_model_id_respects_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("TEXT2D_MODEL_ID", "org/custom-model")
    assert default_model_id() == "org/custom-model"


def test_default_model_id_fallback(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("TEXT2D_MODEL_ID", raising=False)
    mid = default_model_id()
    assert "FLUX" in mid or "klein" in mid.lower() or "Disty0" in mid


def test_torch_dtype_cpu() -> None:
    from text2d.generator import _torch_dtype_for

    assert _torch_dtype_for("cpu") == __import__("torch").float32
