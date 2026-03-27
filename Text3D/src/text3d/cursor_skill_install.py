"""Instala a Agent Skill Cursor — delegate para gamedev_shared."""

from __future__ import annotations

from pathlib import Path

from gamedev_shared.skill_install import (
    install_agent_skill as _install,
)
from gamedev_shared.skill_install import (
    resolve_skill_source as _resolve,
)

TOOL_NAME = "text3d"


def _package_dir() -> Path:
    return Path(__file__).resolve().parent


def resolve_skill_source() -> Path:
    return _resolve(TOOL_NAME, _package_dir())


def install_agent_skill(target_root: Path, *, force: bool = False) -> Path:
    return _install(TOOL_NAME, _package_dir(), target_root, force=force)
