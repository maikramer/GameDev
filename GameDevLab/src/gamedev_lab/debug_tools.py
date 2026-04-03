"""Helpers para comandos debug (Animator3D via subprocess)."""

from __future__ import annotations

import json
from typing import Any

from gamedev_shared.subprocess_utils import merge_subprocess_output, resolve_binary, run_cmd

__all__ = [
    "extract_json_from_output",
    "merge_subprocess_output",
    "resolve_animator3d_bin",
    "run_cmd",
]


def resolve_animator3d_bin() -> str | None:
    try:
        return resolve_binary("ANIMATOR3D_BIN", "animator3d")
    except FileNotFoundError:
        return None


def extract_json_from_output(text: str) -> dict[str, Any]:
    """Extrai o primeiro objeto JSON válido de stdout misturado com logs."""
    dec = json.JSONDecoder()
    for i, ch in enumerate(text):
        if ch != "{":
            continue
        try:
            obj, _end = dec.raw_decode(text[i:])
            if isinstance(obj, dict):
                return obj
            return {"_json_value": obj}
        except json.JSONDecodeError:
            continue
    return {
        "_parse_error": True,
        "raw_preview": text[:8000] if len(text) > 8000 else text,
    }
