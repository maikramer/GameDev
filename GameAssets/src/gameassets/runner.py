"""Execução de text2d / text3d via subprocess — delegate para gamedev_shared."""

from __future__ import annotations

from gamedev_shared.subprocess_utils import (
    RunResult,
    merge_subprocess_output,
    resolve_binary,
    run_cmd,
)

__all__ = ["RunResult", "merge_subprocess_output", "resolve_binary", "run_cmd"]
