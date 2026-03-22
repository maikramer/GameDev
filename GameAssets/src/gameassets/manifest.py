"""Leitura do manifest CSV."""

from __future__ import annotations

import csv
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator


@dataclass(frozen=True)
class ManifestRow:
    id: str
    idea: str
    kind: str | None
    generate_3d: bool


def _parse_bool(value: str | None) -> bool:
    if value is None or str(value).strip() == "":
        return False
    s = str(value).strip().lower()
    return s in ("1", "true", "yes", "sim", "y", "on")


def load_manifest(path: Path) -> list[ManifestRow]:
    """Lê CSV com cabeçalhos: id, idea; opcionais: kind, generate_3d."""
    rows: list[ManifestRow] = []
    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        if not reader.fieldnames:
            raise ValueError("CSV vazio ou sem cabeçalhos")
        fields = {h.strip().lower(): h for h in reader.fieldnames if h}
        if "id" not in fields or "idea" not in fields:
            raise ValueError("CSV deve incluir colunas 'id' e 'idea'")
        id_key = fields["id"]
        idea_key = fields["idea"]
        kind_key = fields.get("kind")
        g3_key = fields.get("generate_3d")
        for raw in reader:
            rid = (raw.get(id_key) or "").strip()
            idea = (raw.get(idea_key) or "").strip()
            if not rid or not idea:
                continue
            kind_val = None
            if kind_key:
                k = (raw.get(kind_key) or "").strip()
                kind_val = k if k else None
            g3 = False
            if g3_key:
                g3 = _parse_bool(raw.get(g3_key))
            rows.append(ManifestRow(id=rid, idea=idea, kind=kind_val, generate_3d=g3))
    if not rows:
        raise ValueError("Nenhuma linha válida no manifest (id + idea obrigatórios)")
    return rows


def iter_manifest(path: Path) -> Iterator[ManifestRow]:
    yield from load_manifest(path)
