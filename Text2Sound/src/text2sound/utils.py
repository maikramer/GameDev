"""Text2Sound — utilitários comuns."""

from __future__ import annotations

import time
from pathlib import Path


def format_bytes(bytes_val: int | float) -> str:
    """Formata bytes para representação legível (ex: ``4.5 MB``)."""
    val = float(bytes_val)
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if val < 1024.0:
            return f"{val:.1f} {unit}"
        val /= 1024.0
    return f"{val:.1f} PB"


def safe_filename(text: str, max_len: int = 40) -> str:
    """Gera nome de ficheiro seguro a partir de texto."""
    return "".join(c if c.isalnum() else "_" for c in text[:max_len])


def generate_output_path(
    prompt: str,
    output_dir: Path,
    fmt: str = "wav",
) -> Path:
    """Gera caminho de saída único baseado no prompt e timestamp."""
    ts = int(time.time())
    safe = safe_filename(prompt)
    return output_dir / f"{safe}_{ts}.{fmt}"


def format_duration(seconds: float) -> str:
    """Formata segundos como mm:ss."""
    m = int(seconds) // 60
    s = int(seconds) % 60
    return f"{m}:{s:02d}"
