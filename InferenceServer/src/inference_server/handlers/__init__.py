from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

from ..schemas import (
    JobType,
    Skymap2DParams,
    Text2DParams,
    Text3DParams,
    Texture2DParams,
)
from .skymap2d import run_skymap2d
from .text2d import run_text2d
from .text3d import run_text3d
from .texture2d import run_texture2d

HandlerFn = Callable[[str, dict[str, Any], Path], list[str]]

HANDLERS: dict[JobType, HandlerFn] = {
    "text2d": lambda jid, p, d: run_text2d(jid, Text2DParams.model_validate(p), d),
    "text3d": lambda jid, p, d: run_text3d(jid, Text3DParams.model_validate(p), d),
    "skymap2d": lambda jid, p, d: run_skymap2d(jid, Skymap2DParams.model_validate(p), d),
    "texture2d": lambda jid, p, d: run_texture2d(jid, Texture2DParams.model_validate(p), d),
}
