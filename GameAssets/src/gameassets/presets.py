"""Carregar presets de estilo (YAML embutido + opcional presets.local.yaml)."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml

from .resources import presets_yaml_path


def _load_yaml(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        data = yaml.safe_load(f)
    return data if isinstance(data, dict) else {}


def load_presets_bundle(local_path: Path | None = None) -> dict[str, Any]:
    """Merge presets embutidos com ficheiro opcional (chaves locais sobrepõem)."""
    base = presets_yaml_path()
    merged: dict[str, Any] = _load_yaml(base)
    if local_path is not None and local_path.is_file():
        extra = _load_yaml(local_path)
        for key, val in extra.items():
            if isinstance(val, dict) and isinstance(merged.get(key), dict):
                merged[key] = {**merged[key], **val}
            else:
                merged[key] = val
    return merged


def get_preset(presets: dict[str, Any], name: str) -> dict[str, Any]:
    if name not in presets:
        known = ", ".join(sorted(presets.keys())) or "(nenhum)"
        raise KeyError(f"Preset desconhecido: {name!r}. Disponíveis: {known}")
    raw = presets[name]
    if not isinstance(raw, dict):
        raise ValueError(f"Preset {name!r} deve ser um mapa YAML")
    return raw
