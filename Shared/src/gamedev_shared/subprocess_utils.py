"""Execução de ferramentas via subprocess — resolve binários e executa comandos."""

from __future__ import annotations

import os
import shutil
import subprocess
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path


@dataclass
class RunResult:
    """Resultado de um comando subprocess."""

    returncode: int
    stdout: str
    stderr: str

    @property
    def ok(self) -> bool:
        return self.returncode == 0


def merge_subprocess_output(
    r: RunResult,
    *,
    max_chars: int | None = None,
) -> str:
    """Junta stderr e stdout para diagnóstico (tracebacks podem ir só para stdout)."""
    err = (r.stderr or "").strip()
    out = (r.stdout or "").strip()
    text = f"{err}\n\n--- stdout ---\n{out}" if err and out else err or out
    if max_chars is not None and len(text) > max_chars:
        return "... (truncado no início)\n" + text[-max_chars:]
    return text


def resolve_binary(env_name: str, default_name: str) -> str:
    """Resolve executável: variável de ambiente → PATH → FileNotFoundError.

    Args:
        env_name: Nome da variável de ambiente (ex: ``TEXT2D_BIN``).
        default_name: Nome do comando no PATH (ex: ``text2d``).

    Returns:
        Caminho absoluto ou nome do executável encontrado.

    Raises:
        FileNotFoundError: Binário não encontrado.
    """
    override = os.environ.get(env_name, "").strip()
    if override:
        return override
    found = shutil.which(default_name)
    if not found:
        raise FileNotFoundError(
            f"Comando não encontrado: {default_name!r}. Instala o pacote ou define {env_name} com o caminho absoluto."
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
    """Executa um comando e devolve ``RunResult``.

    Args:
        argv: Comando e argumentos.
        cwd: Directório de trabalho.
        capture: Capturar stdout/stderr.
        timeout: Timeout em segundos.
        extra_env: Variáveis extra a juntar ao ambiente.
    """
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
    return RunResult(
        returncode=r.returncode,
        stdout=r.stdout or "",
        stderr=r.stderr or "",
    )
