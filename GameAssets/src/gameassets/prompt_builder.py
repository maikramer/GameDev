"""Construção do prompt final a partir do perfil, preset e linha do manifest."""

from __future__ import annotations

import re
from typing import Any

from .manifest import ManifestRow
from .profile import GameProfile

_KIND_HINTS: dict[str, str] = {
    "prop": "isolated game prop, hero asset, clear readability",
    "character": "game character design, full figure or bust as appropriate, memorable silhouette",
    "environment": "environment art piece, readable composition, game level visual",
}

# Descrições sonoras por kind (Text2Sound — não usar hints visuais de imagem)
_AUDIO_KIND_HINTS: dict[str, str] = {
    "prop": "short game sound effect, punchy and clear",
    "character": "character-related audio, game-ready",
    "environment": "ambient soundscape, loop-friendly atmospheric audio",
}

# Quando generate_3d=true, a mesma imagem alimenta image-to-3D; sombras/pedestal viram geometria fantasma.
# Estratégia v2: framing positivo em vez de negações. FLUX ignora "no X" mas respeita
# descrições positivas do tipo de render.
_I2M_REF_LIGHTING = (
    "3D game asset reference render, "
    "flat ambient lighting from all directions equally, "
    "uniform soft diffuse illumination, "
    "pure white seamless infinite void background on all sides, "
    "single isolated object centered in frame, "
    "vibrant flat colors, completely shadowless, matte surface finish, "
    "white background visible beneath and around the object, "
    "clean silhouette"
)

_I2M_EXTRA_NEGATIVES: tuple[str, ...] = (
    "cast shadows",
    "harsh directional light",
    "contact shadow",
    "ground shadow",
    "drop shadow",
    "dark circle under object",
    "pedestal",
    "platform base",
    "ground plane",
    "spotlight",
    "rim light",
    "volumetric light",
)

_MESH_HINT_NO_FAKE_GROUND = (
    "Watertight mesh only; no spurious ground disc, pedestal ring, or base slab from shadow-like shading"
)

# O título do jogo no prompt tende a aparecer como texto/logótipo na imagem; não incluir.
# Restrições extra para 2D (referência) — modelos desenhadores de UI/caption.
_UI_TEXT_NEGATIVES: tuple[str, ...] = (
    "text overlay",
    "caption",
    "watermark",
    "logo",
    "title card",
    "typography",
    "UI",
    "HUD",
    "icon strip",
    "app icon",
)


def _mood_atmosphere(genre: str, tone: str) -> str:
    return ", ".join(p for p in (genre.strip(), tone.strip()) if p)


def build_prompt(
    profile: GameProfile,
    preset: dict[str, Any],
    row: ManifestRow,
    *,
    for_3d: bool = False,
) -> str:
    """Monta o prompt positivo (negativos concatenados ao final como restrições)."""
    prefix = str(preset.get("prompt_prefix") or "").strip()
    label_hint = ""
    label_hint = str(preset.get("hint_3d") or "").strip() if for_3d else str(preset.get("hint_2d") or "").strip()

    kind = (row.kind or "").strip().lower()
    kind_extra = _KIND_HINTS.get(kind, "")

    mood = _mood_atmosphere(profile.genre, profile.tone)
    idea = row.idea.strip()

    chunks: list[str] = []
    if prefix:
        chunks.append(prefix)
    if mood:
        chunks.append(f"Mood and setting: {mood}.")
    chunks.append(f"{idea}.")
    if kind_extra:
        chunks.append(kind_extra + ".")
    if label_hint:
        chunks.append(label_hint + ".")
    if row.generate_3d:
        if for_3d:
            chunks.append(_MESH_HINT_NO_FAKE_GROUND + ".")
        else:
            chunks.append(_I2M_REF_LIGHTING + ".")

    main = " ".join(chunks)
    main = re.sub(r"\s+", " ", main).strip()

    neg_parts: list[str] = []
    neg_suffix = str(preset.get("negative_suffix") or "").strip()
    if neg_suffix:
        neg_parts.append(neg_suffix)
    for kw in profile.negative_keywords:
        kw = str(kw).strip()
        if kw:
            neg_parts.append(kw)
    if row.generate_3d and not for_3d:
        neg_parts.extend(_I2M_EXTRA_NEGATIVES)
    if not for_3d:
        neg_parts.extend(_UI_TEXT_NEGATIVES)
    if neg_parts:
        neg_joined = ", ".join(neg_parts)
        main = f"{main} Avoid: {neg_joined}."

    return main


def build_audio_prompt(
    profile: GameProfile,
    preset: dict[str, Any],
    row: ManifestRow,
) -> str:
    """Prompt para Text2Sound (áudio a partir de texto; sem referências visuais de imagem 3D)."""
    prefix = str(preset.get("prompt_prefix") or "").strip()
    label_hint = str(preset.get("hint_audio") or preset.get("hint_2d") or "").strip()

    kind = (row.kind or "").strip().lower()
    kind_extra = _AUDIO_KIND_HINTS.get(kind, "game audio, stereo, high quality")

    mood = _mood_atmosphere(profile.genre, profile.tone)
    idea = row.idea.strip()

    chunks: list[str] = []
    if prefix:
        chunks.append(prefix)
    if mood:
        chunks.append(f"Mood: {mood}.")
    chunks.append(f"{idea}.")
    chunks.append(kind_extra + ".")
    if label_hint:
        chunks.append(label_hint + ".")

    main = " ".join(chunks)
    main = re.sub(r"\s+", " ", main).strip()

    neg_parts: list[str] = []
    neg_suffix = str(preset.get("negative_suffix") or "").strip()
    if neg_suffix:
        neg_parts.append(neg_suffix)
    for kw in profile.negative_keywords:
        kw = str(kw).strip()
        if kw:
            neg_parts.append(kw)
    if neg_parts:
        neg_joined = ", ".join(neg_parts)
        main = f"{main} Avoid: {neg_joined}."

    return main
