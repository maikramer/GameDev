"""Leitura do manifest CSV."""

from __future__ import annotations

import csv
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path

from .profile import GameProfile


@dataclass(frozen=True)
class ManifestRow:
    id: str
    idea: str
    kind: str | None
    generate_3d: bool
    # Sobrepõe game.yaml image_source para esta linha (text2d | texture2d)
    image_source: str | None = None
    # Gera clip de áudio com Text2Sound (requer bloco text2sound no perfil ou defaults)
    generate_audio: bool = False
    # Auto-rig do GLB (Rigging3D) após Text3D; requer --with-rig e generate_3d=true
    generate_rig: bool = False


def effective_image_source(profile: GameProfile, row: ManifestRow) -> str:
    """Fonte 2D efectiva: coluna CSV ou defeito do perfil."""
    if row.image_source:
        return row.image_source
    return profile.image_source


def _parse_bool(value: str | None) -> bool:
    if value is None or str(value).strip() == "":
        return False
    s = str(value).strip().lower()
    return s in ("1", "true", "yes", "sim", "y", "on")


def _parse_image_source(value: str | None) -> str | None:
    if value is None or str(value).strip() == "":
        return None
    s = str(value).strip().lower()
    if s not in ("text2d", "texture2d", "skymap2d"):
        raise ValueError("Coluna image_source deve ser text2d, texture2d ou skymap2d (ou vazio para herdar o perfil)")
    return s


def load_manifest(path: Path) -> list[ManifestRow]:
    """Lê CSV com cabeçalhos: id, idea; opcionais: kind, generate_3d, image_source, generate_audio, generate_rig."""
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
        img_src_key = fields.get("image_source")
        ga_key = fields.get("generate_audio")
        gr_key = fields.get("generate_rig")
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
            img_src: str | None = None
            if img_src_key:
                img_src = _parse_image_source(raw.get(img_src_key))
            ga = False
            if ga_key:
                ga = _parse_bool(raw.get(ga_key))
            gr = False
            if gr_key:
                gr = _parse_bool(raw.get(gr_key))
            rows.append(
                ManifestRow(
                    id=rid,
                    idea=idea,
                    kind=kind_val,
                    generate_3d=g3,
                    image_source=img_src,
                    generate_audio=ga,
                    generate_rig=gr,
                )
            )
    if not rows:
        raise ValueError("Nenhuma linha válida no manifest (id + idea obrigatórios)")
    return rows


def iter_manifest(path: Path) -> Iterator[ManifestRow]:
    yield from load_manifest(path)
