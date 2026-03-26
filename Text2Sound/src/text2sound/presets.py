"""Text2Sound — presets temáticos para desenvolvimento de jogos.

Cada preset define prompt, duração, passos de difusão e CFG scale
otimizados para cenários comuns de game audio.

Os presets assumem o modelo **Open 1.0** (música, até ~47s). Com
``--profile effects`` (Open Small, máx. ~11s), presets com duração
superior falham na validação — usa ``-d`` explícito ou perfil música.
"""

from __future__ import annotations

from typing import Any

AUDIO_PRESETS: dict[str, dict[str, Any]] = {
    "ambient": {
        "prompt": (
            "Calm ambient soundscape with gentle wind, distant birds chirping, "
            "and soft nature atmosphere, peaceful and immersive"
        ),
        "duration": 45,
        "steps": 100,
        "cfg_scale": 7.0,
    },
    "battle": {
        "prompt": (
            "Intense orchestral battle music with epic drums, brass fanfare, "
            "driving percussion and heroic strings, cinematic game combat soundtrack"
        ),
        "duration": 30,
        "steps": 120,
        "cfg_scale": 8.0,
    },
    "menu": {
        "prompt": (
            "Soft loopable menu music with gentle piano, warm pads, "
            "and subtle ambient textures, calm and inviting game menu theme"
        ),
        "duration": 30,
        "steps": 100,
        "cfg_scale": 7.0,
    },
    "footsteps-stone": {
        "prompt": (
            "Footsteps walking on stone floor, clear and rhythmic steps "
            "on hard surface, indoor stone corridor"
        ),
        "duration": 5,
        "steps": 80,
        "cfg_scale": 9.0,
    },
    "footsteps-grass": {
        "prompt": (
            "Footsteps walking on grass and soft ground, quiet rustling "
            "of vegetation underfoot, outdoor nature path"
        ),
        "duration": 5,
        "steps": 80,
        "cfg_scale": 9.0,
    },
    "rain": {
        "prompt": (
            "Steady rainfall with distant thunder rumbles, rain hitting surfaces, "
            "atmospheric storm ambience for game environment"
        ),
        "duration": 45,
        "steps": 100,
        "cfg_scale": 7.5,
    },
    "wind": {
        "prompt": (
            "Strong wind blowing through open landscape, gusty howling wind, "
            "atmospheric outdoor wind ambience"
        ),
        "duration": 30,
        "steps": 100,
        "cfg_scale": 7.0,
    },
    "thunder": {
        "prompt": (
            "Dramatic thunder crack and rolling rumble, powerful storm thunder, "
            "single thunderclap with reverb tail"
        ),
        "duration": 8,
        "steps": 100,
        "cfg_scale": 9.0,
    },
    "ui-click": {
        "prompt": (
            "Short UI click sound, clean digital button press, "
            "crisp interface interaction sound effect"
        ),
        "duration": 2,
        "steps": 60,
        "cfg_scale": 10.0,
    },
    "ui-confirm": {
        "prompt": (
            "Positive confirmation chime, bright ascending tone, "
            "success notification sound for game interface"
        ),
        "duration": 3,
        "steps": 60,
        "cfg_scale": 10.0,
    },
    "forest": {
        "prompt": (
            "Dense forest ambience with birds singing, leaves rustling in wind, "
            "distant stream flowing, rich woodland atmosphere"
        ),
        "duration": 45,
        "steps": 100,
        "cfg_scale": 7.0,
    },
    "ocean": {
        "prompt": (
            "Ocean waves crashing on shore, rhythmic sea surf, "
            "coastal seascape with seagulls in distance"
        ),
        "duration": 45,
        "steps": 100,
        "cfg_scale": 7.0,
    },
    "dungeon": {
        "prompt": (
            "Dark dungeon ambience with dripping water echoes, distant chains, "
            "eerie underground cave atmosphere, subtle tension"
        ),
        "duration": 30,
        "steps": 110,
        "cfg_scale": 8.0,
    },
    "tavern": {
        "prompt": (
            "Busy medieval tavern atmosphere with crowd chatter, "
            "clinking glasses, crackling fireplace, and distant lute music"
        ),
        "duration": 30,
        "steps": 110,
        "cfg_scale": 7.5,
    },
    "explosion": {
        "prompt": (
            "Powerful explosion blast with deep bass impact, "
            "debris scattering, cinematic boom sound effect"
        ),
        "duration": 5,
        "steps": 80,
        "cfg_scale": 10.0,
    },
    "sword-clash": {
        "prompt": (
            "Metal sword clash and parry, sharp metallic impact of blades, "
            "weapon combat sound effect with ring"
        ),
        "duration": 3,
        "steps": 80,
        "cfg_scale": 10.0,
    },
    "magic-spell": {
        "prompt": (
            "Magical spell cast with shimmering energy, mystical whoosh "
            "and crystalline sparkle, fantasy magic sound effect"
        ),
        "duration": 4,
        "steps": 90,
        "cfg_scale": 9.0,
    },
    "victory-fanfare": {
        "prompt": (
            "Triumphant victory fanfare with brass and strings, "
            "uplifting celebratory short jingle, game level complete"
        ),
        "duration": 8,
        "steps": 100,
        "cfg_scale": 8.5,
    },
}


def list_presets() -> list[str]:
    """Retorna nomes dos presets disponíveis, ordenados."""
    return sorted(AUDIO_PRESETS.keys())


def get_preset(name: str) -> dict[str, Any]:
    """Retorna preset pelo nome (case-insensitive).

    Raises:
        KeyError: Preset não encontrado.
    """
    key = name.lower().replace(" ", "-").replace("_", "-")
    if key in AUDIO_PRESETS:
        return AUDIO_PRESETS[key]
    raise KeyError(
        f"Preset desconhecido: {name!r}. "
        f"Disponíveis: {', '.join(list_presets())}"
    )
