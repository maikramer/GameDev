"""Localização de ficheiros de dados do pacote."""

from __future__ import annotations

from pathlib import Path


def presets_yaml_path() -> Path:
    return Path(__file__).resolve().parent / "data" / "presets.yaml"
