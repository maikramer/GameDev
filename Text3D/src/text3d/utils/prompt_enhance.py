"""Utilitários para melhorar prompts Text2D e evitar artefactos na base do mesh.

O Hunyuan3D interpreta silhuetas e contrastes da imagem 2D como geometria.
Sombras de contato, "ground planes" e pedestais visuais na imagem
viram discos/placas na base do mesh 3D.
"""

from __future__ import annotations

# Termos que incentivam sombras de contato ou plataformas na imagem 2D
# -> Evitar a todo custo pois viram geometria extra no mesh 3D
GROUND_PLANE_TERMS: tuple[str, ...] = (
    "on the ground",
    "on the floor",
    "on a pedestal",
    "on a platform",
    "standing on",
    "sitting on",
    "on a surface",
    "studio floor",
    "contact shadow",
    "drop shadow",
    "ground shadow",
    "ambient occlusion on ground",
)

# Termos que devem ser adicionados ao prompt para evitar a base
# e garantir um fundo verdadeiramente neutro sem "placas"
CLEAN_BASE_SUFFIXES: tuple[str, ...] = (
    "floating in empty space",
    "no ground plane",
    "no floor",
    "no platform",
    "no pedestal",
    "no base",
    "no contact shadow",
    "no shadow underneath",
    "clean bottom",
    "seamless neutral background",
    "solid neutral background",
    "no visible ground",
)

# Termos de fundo neutro que ainda podem ter sombras implícitas
# -> Precisam dos sufixos acima para garantir limpeza
NEUTRAL_BG_TERMS: tuple[str, ...] = (
    "neutral background",
    "plain background",
    "simple background",
    "studio background",
    "white background",
    "gray background",
    "grey background",
)


def enhance_prompt_for_clean_base(prompt: str, aggressive: bool = True) -> str:
    """Melhora o prompt para evitar 'placas' e sombras de contato na base.

    Args:
        prompt: Prompt original do usuário.
        aggressive: Se True, adiciona múltiplos sufixos anti-placa.
                   Se False, adiciona apenas o essencial.

    Returns:
        Prompt melhorado com termos que evitam geometria indesejada na base.
    """
    prompt_lower = prompt.lower()

    # Verificar se já tem termos de fundo neutro
    has_neutral_bg = any(term in prompt_lower for term in NEUTRAL_BG_TERMS)

    # Construir lista de sufixos a adicionar
    suffixes: list[str] = []

    # Sempre adicionar no-ground quando aggressive
    if aggressive:
        # Seleção estratégica de sufixos para evitar redundância
        suffixes.extend([
            "no ground plane",
            "no contact shadow",
            "floating in empty space",
            "clean bottom",
        ])
    else:
        # Modo minimalista - apenas o essencial
        suffixes.append("no ground plane, no contact shadow")

    # Se não tem fundo neutro definido, adicionar um
    if not has_neutral_bg:
        suffixes.append("neutral background")

    # Combinar prompt original com sufixos
    enhanced = prompt.rstrip().rstrip(",. ")
    suffix_str = ", ".join(suffixes)
    enhanced = f"{enhanced}, {suffix_str}"

    return enhanced


def sanitize_prompt(prompt: str) -> str:
    """Remove termos problemáticos que causam placas na base.

    Args:
        prompt: Prompt original.

    Returns:
        Prompt com termos problemáticos removidos ou substituídos.
    """
    import re

    result = prompt

    # Remover termos que incentivam ground planes
    for term in GROUND_PLANE_TERMS:
        # Regex case-insensitive para substituir termos
        pattern = re.compile(re.escape(term), re.IGNORECASE)
        result = pattern.sub("", result)

    # Limpar espaços múltiplos e pontuação residual
    result = re.sub(r"\s+", " ", result)
    result = re.sub(r"[,;]+\s*[,;]*", ", ", result)
    result = result.strip(",. ")

    return result


def create_optimized_prompt(prompt: str, aggressive: bool = True) -> str:
    """Pipeline completo: sanitizar + melhorar o prompt.

    Args:
        prompt: Prompt original do usuário.
        aggressive: Modo agressivo de anti-placa.

    Returns:
        Prompt otimizado para gerar meshes sem artefactos na base.
    """
    # Primeiro remover termos problemáticos
    clean = sanitize_prompt(prompt)
    # Depois adicionar termos protetores
    enhanced = enhance_prompt_for_clean_base(clean, aggressive=aggressive)
    return enhanced
