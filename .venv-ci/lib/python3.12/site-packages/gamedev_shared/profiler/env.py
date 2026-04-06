"""Variáveis de ambiente para ativar profiling em subprocessos (ex.: gameassets batch)."""

from __future__ import annotations

import os

ENV_PROFILE = "GAMEDEV_PROFILE"
ENV_LOG = "GAMEDEV_PROFILE_LOG"
ENV_TOOL = "GAMEDEV_PROFILE_TOOL"


def env_profile_enabled() -> bool:
    """True se ``GAMEDEV_PROFILE`` está definido como valor truthy (1, true, yes, on)."""
    v = (os.environ.get(ENV_PROFILE) or "").strip().lower()
    return v in ("1", "true", "yes", "on")


def env_profile_log_path() -> str | None:
    """Caminho opcional para JSONL (uma linha por span)."""
    p = (os.environ.get(ENV_LOG) or "").strip()
    return p if p else None


def env_profile_tool() -> str | None:
    """Nome da ferramenta (ex.: ``gameassets``) para metadados nos eventos."""
    t = (os.environ.get(ENV_TOOL) or "").strip()
    return t if t else None


def is_profiling_enabled(cli_flag: bool | None = None) -> bool:
    """
    Profiling ativo se a flag CLI for True **ou** se o ambiente pedir profiling.

    Args:
        cli_flag: Pass ``True`` quando o utilizador usa ``--profile`` na CLI actual.
    """
    if cli_flag is True:
        return True
    return env_profile_enabled()
