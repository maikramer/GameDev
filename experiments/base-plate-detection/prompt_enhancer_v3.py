"""
Prompt Enhancer v3 - Combinando Text3D + GameAssets + estratégias avançadas

Estratégias implementadas:
1. Framing positivo (render flat, iluminação uniforme)
2. Negativos explícitos de sombras
3. Termos tóxicos expandidos
4. Instruções específicas para evitar placas na base
5. Referência a fundo infinito branco
"""

from __future__ import annotations

import re

# Termos tóxicos expandidos (Text3D + GameAssets + extras)
TOXIC_TERMS: tuple[str, ...] = (
    # Posição/chão
    "on the ground",
    "on the floor",
    "on a pedestal",
    "on a platform",
    "standing on",
    "sitting on",
    "on a surface",
    "on a table",
    "on a desk",
    "studio floor",
    "ground surface",
    "floor surface",
    "platform",
    "base platform",
    "standing",
    "sitting",
    # Sombras
    "contact shadow",
    "drop shadow",
    "ground shadow",
    "ambient occlusion on ground",
    "harsh lighting",
    "dramatic lighting",
    "rim light",
    "strong directional light",
    "spotlight",
    "volumetric light",
    "god rays",
    "lens flare",
    "backlit",
    "side lit",
    "chiaroscuro",
    "cast shadows",
    "dark circle under object",
    "pedestal ring",
    "ground disc",
    "base slab",
    "shadow",
    "shadows",
    # Flutuação
    "floating",
    "levitating",
    # Outros
    "dark background",
    "gradient background",
    "colored background",
)

# Render prefix - versão agressiva (GameAssets + melhorias)
RENDER_PREFIX_V3 = (
    "3D game asset reference render, "
    "three-quarter view showing depth and volume, "
    "flat ambient lighting from all directions equally, "
    "uniform soft diffuse illumination with NO shadows, "
    "pure white seamless infinite void background on all sides, "
    "single isolated object centered in frame, "
    "NO ground plane, NO base platform, NO contact shadows, "
    "white background visible beneath and around the object"
)

# Render prefix - versão ultra agressiva
RENDER_PREFIX_ULTRA = (
    "3D game asset reference render, "
    "three-quarter elevated view, "
    "completely flat uniform albedo lighting from all angles, "
    "zero shadows, zero shading gradients, "
    "pure white infinite seamless background, "
    "object isolated with white background surrounding entire perimeter, "
    "no ground contact, no base disc, no pedestal, no platform, "
    "object floating in white void with clear gap beneath"
)

# Sufixo de reforço - versão v3
RENDER_SUFFIX_V3 = (
    "vibrant flat colors, "
    "completely shadowless, "
    "matte surface finish, "
    "full 3D volume visible from all angles, "
    "clean silhouette without ground contact, "
    "game asset quality, "
    "no base geometry, no bottom plate"
)

# Negativos adicionais para concatenar
EXTRA_NEGATIVES = (
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
    "text overlay",
    "watermark",
    "logo",
    "title card",
)


def sanitize_prompt_v3(prompt: str) -> str:
    """Remove termos que causam placas/sombras."""
    result = prompt

    # Processar termos mais longos primeiro
    for term in sorted(TOXIC_TERMS, key=len, reverse=True):
        pattern = re.compile(re.escape(term), re.IGNORECASE)
        result = pattern.sub("", result)

    # Limpar conjunções órfãs
    result = re.sub(r"\bwith\s+(and|,)", r"\1", result, flags=re.IGNORECASE)
    result = re.sub(r"\bwith\s*$", "", result)
    result = re.sub(r"\bwith\s*,", ",", result)
    result = re.sub(r"\band\s*,", ",", result)
    result = re.sub(r",\s*and\s*$", "", result)
    result = re.sub(r",\s*and\s*,", ",", result)
    result = re.sub(r"\b(standing|sitting)\s+(and\s*)?$", "", result, flags=re.IGNORECASE)
    result = re.sub(r"\b(standing|sitting)\s+(and\s*)?,", ",", result, flags=re.IGNORECASE)
    result = re.sub(r"\band\s*$", "", result, flags=re.IGNORECASE)
    result = re.sub(r"\s+", " ", result)
    result = re.sub(r"[,;]+\s*[,;]*", ", ", result)
    result = result.strip(",. ")

    return result


def enhance_prompt_v3(prompt: str, mode: str = "standard") -> str:
    """
    Melhora o prompt para evitar placas na base.

    Args:
        prompt: Prompt original do usuário
        mode: "standard" (v3) ou "ultra" (mais agressivo)
    """
    clean = sanitize_prompt_v3(prompt)

    if mode == "ultra":
        prefix = RENDER_PREFIX_ULTRA
    else:
        prefix = RENDER_PREFIX_V3

    # Construir prompt final
    enhanced = f"{prefix}, {clean.strip()}, {RENDER_SUFFIX_V3}"

    # Adicionar negativos
    neg_joined = ", ".join(EXTRA_NEGATIVES)
    enhanced = f"{enhanced} Avoid: {neg_joined}."

    return enhanced


def compare_enhancements(prompt: str) -> dict[str, str]:
    """Compara diferentes versões de enhancement."""
    return {
        "original": prompt,
        "sanitized": sanitize_prompt_v3(prompt),
        "v3_standard": enhance_prompt_v3(prompt, mode="standard"),
        "v3_ultra": enhance_prompt_v3(prompt, mode="ultra"),
    }


if __name__ == "__main__":
    # Testar com os prompts do experimento
    test_prompts = [
        "modern minimalist chair with wooden legs, clean design, studio lighting",
        "ceramic vase with a narrow base, smooth surface, decorative pottery",
        "robot standing on ground, mechanical legs, industrial design",
        "cartoon character on pedestal, stylized figure, colorful",
        "small side table with four legs, wooden top, furniture",
    ]

    print("=" * 80)
    print("COMPARAÇÃO DE PROMPT ENHANCEMENTS")
    print("=" * 80)

    for prompt in test_prompts:
        print(f"\n{'=' * 80}")
        print(f"ORIGINAL: {prompt}")
        print("-" * 80)

        versions = compare_enhancements(prompt)

        print(f"\nSANITIZED:\n  {versions['sanitized']}")
        print(f"\nV3 STANDARD:\n  {versions['v3_standard']}")
        print(f"\nV3 ULTRA:\n  {versions['v3_ultra']}")
        print(
            f"\nTamanhos: original={len(prompt)}, v3_std={len(versions['v3_standard'])}, v3_ultra={len(versions['v3_ultra'])}"
        )
