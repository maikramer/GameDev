"""
Pós-processamento de meshes Hunyuan3D: componentes desconexas, artefactos finos.

O modelo image-to-3D frequentemente gera várias ilhas (ex.: pés separados do corpo)
ou perde geometria fina; aqui aplicamos heurísticas conservadoras.
"""

from __future__ import annotations

from typing import Optional

import trimesh


def keep_largest_component(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """
    Mantém apenas a componente conexa com mais faces (descarta ilhas pequenas).

    Resolve casos em que partes do corpo aparecem como mesh separada (pés flutuantes
    como ilha extra, etc.). Se houver uma única componente, devolve igual.
    """
    mesh = mesh.copy()
    try:
        parts = mesh.split(only_watertight=False)
    except Exception:
        return mesh
    if len(parts) <= 1:
        return mesh
    return max(parts, key=lambda m: len(m.faces))


def laplacian_smooth(mesh: trimesh.Trimesh, iterations: int = 1, lamb: float = 0.5) -> trimesh.Trimesh:
    """Suavização Laplaciana leve (reduz aspereza tipo 'argila'; pode arredondar arestas)."""
    if iterations <= 0:
        return mesh
    m = mesh.copy()
    trimesh.smoothing.filter_laplacian(m, iterations=iterations, lamb=lamb)
    return m


def repair_mesh(
    mesh: trimesh.Trimesh,
    *,
    keep_largest: bool = True,
    merge_vertices: bool = True,
    smooth_iterations: int = 0,
    smooth_lamb: float = 0.45,
) -> trimesh.Trimesh:
    """
    Encadeia heurísticas de reparo.

    ``merge_vertices`` ajuda a fechar buracos pequenos de malha e consistência.
    """
    m = mesh.copy()

    if merge_vertices:
        try:
            m.merge_vertices()
        except Exception:
            pass

    if keep_largest:
        m = keep_largest_component(m)

    if smooth_iterations > 0:
        m = laplacian_smooth(m, iterations=smooth_iterations, lamb=smooth_lamb)

    try:
        m.remove_unreferenced_vertices()
    except Exception:
        pass

    return m
