"""Execução de text2d / text3d via subprocess."""

from __future__ import annotations

import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


@dataclass
class RunResult:
    returncode: int
    stdout: str
    stderr: str


def merge_subprocess_output(
    r: RunResult,
    *,
    max_chars: int | None = None,
) -> str:
    """Junta stderr e stdout para diagnóstico (tracebacks podem ir só para stdout)."""
    err = (r.stderr or "").strip()
    out = (r.stdout or "").strip()
    if err and out:
        text = f"{err}\n\n--- stdout ---\n{out}"
    else:
        text = err or out
    if max_chars is not None and len(text) > max_chars:
        return "... (truncado no início)\n" + text[-max_chars:]
    return text


def resolve_binary(env_name: str, default_name: str) -> str:
    override = os.environ.get(env_name, "").strip()
    if override:
        return override
    found = shutil.which(default_name)
    if not found:
        raise FileNotFoundError(
            f"Comando não encontrado: {default_name!r}. "
            f"Instala o pacote ou define {env_name} com o caminho absoluto."
        )
    return found


def run_cmd(
    argv: Sequence[str],
    *,
    cwd: Path | None = None,
    capture: bool = True,
    timeout: float | None = None,
    extra_env: dict[str, str] | None = None,
) -> RunResult:
    env = os.environ.copy()
    if extra_env:
        env.update(extra_env)
    r = subprocess.run(
        list(argv),
        cwd=cwd,
        capture_output=capture,
        text=True,
        timeout=timeout,
        env=env,
    )
    out = r.stdout or ""
    err = r.stderr or ""
    return RunResult(returncode=r.returncode, stdout=out, stderr=err)
