"""Resumo textual e exportação JSONL."""

from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, TextIO


def write_jsonl_event(fp: TextIO, event: dict[str, Any]) -> None:
    """Escreve uma linha JSON (sem pretty-print)."""
    fp.write(json.dumps(event, ensure_ascii=False) + "\n")
    fp.flush()


def append_jsonl(path: Path | str, event: dict[str, Any]) -> None:
    """Anexa um evento a um ficheiro JSONL."""
    p = Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    with p.open("a", encoding="utf-8") as f:
        write_jsonl_event(f, event)


def print_summary_table(events: list[dict[str, Any]], file: TextIO | None = None) -> None:
    """Imprime tabela de resumo (Rich se disponível; senão texto simples)."""
    out = file or sys.stdout
    if not events:
        print("[profiler] Nenhum evento registado.", file=out)
        return

    try:
        from rich.console import Console
        from rich.table import Table

        console = Console(file=out, force_terminal=False)
        table = Table(title="Profiler — spans", show_header=True, header_style="bold")
        table.add_column("span", style="cyan")
        table.add_column("ms", justify="right")
        table.add_column("ΔRSS MB", justify="right")
        table.add_column("ΔCUDA MB", justify="right")

        for ev in events:
            name = str(ev.get("span", "?"))
            ms = ev.get("duration_ms", "")
            drss = ev.get("rss_delta_mb", "")
            dcuda = ev.get("cuda_allocated_delta_mb", "")
            if drss == "" or drss is None:
                drss = "—"
            if dcuda == "" or dcuda is None:
                dcuda = "—"
            table.add_row(name, f"{ms}", f"{drss}", f"{dcuda}")
        console.print(table)
    except ImportError:
        print("Profiler — spans", file=out)
        print("-" * 72, file=out)
        for ev in events:
            print(
                f"  {ev.get('span', '?')}: {ev.get('duration_ms', '')} ms "
                f"rss_delta={ev.get('rss_delta_mb', '—')} cuda_Δ={ev.get('cuda_allocated_delta_mb', '—')}",
                file=out,
            )


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()
