"""Variáveis de ambiente (CUDA alloc)."""

from __future__ import annotations

import os

import pytest

from text3d.utils.env import ensure_pytorch_cuda_alloc_conf


def test_sets_expandable_when_unset(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("PYTORCH_CUDA_ALLOC_CONF", raising=False)
    ensure_pytorch_cuda_alloc_conf()
    assert os.environ.get("PYTORCH_CUDA_ALLOC_CONF") == "expandable_segments:True"


def test_preserves_user_value(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("PYTORCH_CUDA_ALLOC_CONF", "max_split_size_mb:128")
    ensure_pytorch_cuda_alloc_conf()
    assert os.environ["PYTORCH_CUDA_ALLOC_CONF"] == "max_split_size_mb:128"
