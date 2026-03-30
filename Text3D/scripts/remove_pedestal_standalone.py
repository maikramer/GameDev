#!/usr/bin/env python3
"""
Remove pedestal/plataforma de arquivos GLB/OBJ — versão standalone.

Não requer o ambiente Text3D completo, apenas trimesh.

Exemplos:
    python remove_pedestal_standalone.py modelo.glb --output modelo_limpo.glb
    python remove_pedestal_standalone.py modelo.glb --very-aggressive
"""

from __future__ import annotations

import argparse
import contextlib
import sys
from collections import deque
from pathlib import Path

import numpy as np
import trimesh


def _remove_connected_ground_plinth(
    mesh: trimesh.Trimesh,
    *,
    bottom_frac: float = 0.12,
    min_normal_y: float = 0.35,
    max_remove_frac: float = 0.35,
    min_expansion: float = 1.15,
) -> trimesh.Trimesh:
    """Remove pedestal/plataforma conectada ao mesh principal na base."""
    if len(mesh.faces) == 0:
        return mesh

    bounds = mesh.bounds
    ymin = float(bounds[0, 1])
    ymax = float(bounds[1, 1])
    h = ymax - ymin
    if h < 1e-8:
        return mesh

    y_cut = ymin + bottom_frac * h
    centers = mesh.triangles_center
    normals = mesh.face_normals

    in_bottom = centers[:, 1] <= y_cut
    ny = np.abs(normals[:, 1])
    is_horizontal = ny >= min_normal_y
    candidate_mask = in_bottom & is_horizontal

    if np.count_nonzero(candidate_mask) == 0:
        return mesh

    cx = 0.5 * (bounds[0, 0] + bounds[1, 0])
    cz = 0.5 * (bounds[0, 2] + bounds[1, 2])

    bottom_faces_idx = np.where(in_bottom)[0]
    if len(bottom_faces_idx) == 0:
        return mesh

    bottom_centers = centers[bottom_faces_idx]
    dx_bottom = bottom_centers[:, 0] - cx
    dz_bottom = bottom_centers[:, 2] - cz
    r_bottom_p90 = np.percentile(np.sqrt(dx_bottom**2 + dz_bottom**2), 90)
    r_bottom_mean = np.sqrt(dx_bottom**2 + dz_bottom**2).mean()

    upper_y = ymin + 0.35 * h
    upper_mask = (centers[:, 1] > y_cut) & (centers[:, 1] <= upper_y)
    upper_faces_idx = np.where(upper_mask)[0]

    if len(upper_faces_idx) == 0:
        r_upper_p90 = min(bounds[1, 0] - bounds[0, 0], bounds[1, 2] - bounds[0, 2]) * 0.25
        r_upper_mean = r_upper_p90 * 0.8
    else:
        upper_centers = centers[upper_faces_idx]
        dx_upper = upper_centers[:, 0] - cx
        dz_upper = upper_centers[:, 2] - cz
        r_upper_p90 = np.percentile(np.sqrt(dx_upper**2 + dz_upper**2), 90)
        r_upper_mean = np.sqrt(dx_upper**2 + dz_upper**2).mean()

    if r_upper_p90 < 1e-9:
        return mesh

    expansion_p90 = r_bottom_p90 / r_upper_p90 if r_upper_p90 > 0 else 1.0
    expansion_mean = r_bottom_mean / r_upper_mean if r_upper_mean > 0 else 1.0
    expansion_ratio = max(expansion_p90, expansion_mean)

    if expansion_ratio < min_expansion:
        return mesh

    visited = np.zeros(len(mesh.faces), dtype=bool)
    to_remove = np.zeros(len(mesh.faces), dtype=bool)

    # Build face adjacency using edges
    face_to_faces = [set() for _ in range(len(mesh.faces))]
    for _edge_idx, _face_idx in enumerate(mesh.edges_face):
        # edges_face gives the face index for each edge
        # We need to find which faces share each edge
        pass  # Will use a different approach below

    # Better approach: use face_adjacency
    try:
        adjacency = mesh.face_adjacency
        for f1, f2 in adjacency:
            face_to_faces[f1].add(f2)
            face_to_faces[f2].add(f1)
    except Exception:
        # Fallback: use edges to build adjacency
        edges = mesh.edges_unique
        edge_faces = mesh.edges_face
        # Create a map from edge to faces
        edge_to_faces = {}
        for i, edge in enumerate(edges):
            f = edge_faces[i]
            if edge not in edge_to_faces:
                edge_to_faces[edge] = []
            edge_to_faces[edge].append(f)
        # Connect faces that share an edge
        for _edge, faces in edge_to_faces.items():
            if len(faces) == 2:
                f1, f2 = faces
                face_to_faces[f1].add(f2)
                face_to_faces[f2].add(f1)

    seed_faces = np.where(candidate_mask)[0]
    for seed in seed_faces:
        if visited[seed]:
            continue
        component = []
        queue = deque([seed])
        visited[seed] = True

        while queue:
            face_idx = queue.popleft()
            component.append(face_idx)
            for neighbor in face_to_faces[face_idx]:
                if not visited[neighbor] and in_bottom[neighbor]:
                    visited[neighbor] = True
                    queue.append(neighbor)

        if len(component) == 0:
            continue

        component_arr = np.array(component)
        component_horizontal = is_horizontal[component_arr]
        horizontal_ratio = np.count_nonzero(component_horizontal) / len(component)

        if horizontal_ratio > 0.55:
            to_remove[component_arr] = True

    n_remove = int(np.count_nonzero(to_remove))
    if n_remove == 0:
        return mesh

    if n_remove > max_remove_frac * len(mesh.faces):
        return mesh

    keep = ~to_remove
    try:
        sub = mesh.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            return sub
    except Exception:
        pass

    return mesh


def _remove_flat_bottom_islands(
    mesh: trimesh.Trimesh,
    *,
    aggressive: bool = False,
) -> trimesh.Trimesh:
    """Remove componentes desconexas que são placas finas coladas ao solo."""
    try:
        parts = mesh.split(only_watertight=False)
    except Exception:
        return mesh
    if len(parts) <= 1:
        return mesh

    y_min_global = float(mesh.bounds[0, 1])
    h_global = float(mesh.extents[1])
    if h_global < 1e-8:
        return mesh

    bottom_eps = (0.055 if aggressive else 0.035) * h_global
    thin_ratio = 0.28 if aggressive else 0.085
    vol_ratio_max = 0.52 if aggressive else 0.22

    def touches_bottom(p: trimesh.Trimesh) -> bool:
        return float(p.bounds[0, 1]) <= y_min_global + bottom_eps

    def _is_thin_plaque(part: trimesh.Trimesh, *, thin_ratio: float) -> bool:
        e = sorted(float(x) for x in part.extents)
        if len(e) != 3:
            return False
        return e[0] < thin_ratio * e[2]

    def bbox_volume(p: trimesh.Trimesh) -> float:
        e = p.extents
        return float(e[0] * e[1] * e[2])

    has_non_plaque = any(not _is_thin_plaque(p, thin_ratio=thin_ratio) or not touches_bottom(p) for p in parts)
    if not has_non_plaque:
        return mesh

    max_vol = max(bbox_volume(p) for p in parts)
    if max_vol < 1e-18:
        return mesh

    kept: list[trimesh.Trimesh] = []
    for p in parts:
        if touches_bottom(p) and _is_thin_plaque(p, thin_ratio=thin_ratio) and bbox_volume(p) < vol_ratio_max * max_vol:
            continue
        kept.append(p)

    if not kept:
        return mesh
    if len(kept) == 1:
        return kept[0]
    try:
        return trimesh.util.concatenate(kept)
    except Exception:
        return mesh


def _peel_bottom_upward_faces(
    mesh: trimesh.Trimesh,
    *,
    band_frac: float = 0.018,
    min_normal_y: float = 0.82,
    max_remove_frac: float = 0.11,
) -> trimesh.Trimesh:
    """Remove faces quase horizontais na faixa mais baixa do bbox."""
    if len(mesh.faces) == 0:
        return mesh

    _ = np.asarray(mesh.face_normals)

    ymin = float(mesh.vertices[:, 1].min())
    ymax = float(mesh.vertices[:, 1].max())
    h = ymax - ymin
    if h < 1e-8:
        return mesh

    band = max(band_frac * h, 1e-6)
    centers = mesh.triangles_center
    normals = mesh.face_normals
    if normals is None or len(normals) != len(mesh.faces):
        return mesh

    ny = np.asarray(normals[:, 1], dtype=np.float64)
    horizontal = np.abs(ny) >= min_normal_y
    remove = (centers[:, 1] <= ymin + band) & horizontal
    n_remove = int(np.count_nonzero(remove))
    if n_remove == 0:
        return mesh
    if n_remove > max_remove_frac * len(mesh.faces):
        return mesh

    keep = ~remove
    try:
        sub = mesh.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            return sub
    except Exception:
        pass
    return mesh


def _remove_bottom_center_cylinder(
    mesh: trimesh.Trimesh,
    *,
    height_frac: float = 0.15,
    radius_frac: float = 0.9,
    min_normal_y: float | None = 0.4,
    max_remove_frac: float = 0.52,
) -> trimesh.Trimesh:
    """Remove faces no cilindro vertical sob o centro na parte baixa do bbox."""
    if len(mesh.faces) == 0:
        return mesh

    bounds = mesh.bounds
    ymin = float(bounds[0, 1])
    h = float(bounds[1, 1] - bounds[0, 1])
    if h < 1e-8:
        return mesh

    cx = 0.5 * (bounds[0, 0] + bounds[1, 0])
    cz = 0.5 * (bounds[0, 2] + bounds[1, 2])
    rx = 0.5 * (bounds[1, 0] - bounds[0, 0])
    rz = 0.5 * (bounds[1, 2] - bounds[0, 2])
    R = radius_frac * max(rx, rz, 1e-9)
    y_cut = ymin + height_frac * h

    centers = mesh.triangles_center
    dx = centers[:, 0] - cx
    dz = centers[:, 2] - cz
    in_disk = (dx * dx + dz * dz) <= (R * R)
    in_bottom = centers[:, 1] <= y_cut
    remove = in_disk & in_bottom

    if min_normal_y is not None:
        _ = np.asarray(mesh.face_normals)
        ny = np.abs(np.asarray(mesh.face_normals)[:, 1], dtype=np.float64)
        remove = remove & (ny >= float(min_normal_y))

    n_remove = int(np.count_nonzero(remove))
    if n_remove == 0:
        return mesh
    if n_remove > max_remove_frac * len(mesh.faces):
        return mesh

    keep = ~remove
    try:
        sub = mesh.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            return sub
    except Exception:
        pass
    return mesh


def remove_ground_shadow_artifacts(
    mesh: trimesh.Trimesh,
    *,
    aggressive: bool = False,
    very_aggressive: bool = False,
) -> trimesh.Trimesh:
    """Remove disco/placa de sombra na base."""
    m = mesh.copy()
    m = _remove_flat_bottom_islands(m, aggressive=aggressive or very_aggressive)

    if very_aggressive:
        m = _remove_connected_ground_plinth(
            m,
            bottom_frac=0.15,
            min_normal_y=0.25,
            max_remove_frac=0.40,
            min_expansion=1.08,
        )
        m = _remove_bottom_center_cylinder(
            m,
            height_frac=0.18,
            radius_frac=0.75,
            min_normal_y=0.45,
            max_remove_frac=0.40,
        )
        m = _peel_bottom_upward_faces(
            m,
            band_frac=0.065,
            min_normal_y=0.55,
            max_remove_frac=0.30,
        )
    elif aggressive:
        m = _remove_bottom_center_cylinder(
            m,
            height_frac=0.13,
            radius_frac=0.58,
            min_normal_y=0.68,
            max_remove_frac=0.3,
        )
        m = _peel_bottom_upward_faces(
            m,
            band_frac=0.045,
            min_normal_y=0.62,
            max_remove_frac=0.24,
        )
    else:
        m = _peel_bottom_upward_faces(m)

    with contextlib.suppress(Exception):
        m.remove_unreferenced_vertices()

    if len(m.faces) == 0:
        return mesh
    return m


def keep_largest_component(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """Mantém a componente conexa principal."""
    mesh = mesh.copy()
    try:
        parts = mesh.split(only_watertight=False)
    except Exception:
        return mesh
    if len(parts) <= 1:
        return mesh

    def bbox_volume(p: trimesh.Trimesh) -> float:
        e = p.extents
        return float(e[0] * e[1] * e[2])

    return max(parts, key=bbox_volume)


def repair_mesh(
    mesh: trimesh.Trimesh,
    *,
    keep_largest: bool = True,
    merge_vertices: bool = True,
    remove_ground_shadow: bool = True,
    ground_shadow_aggressive: bool = False,
    ground_shadow_very_aggressive: bool = False,
    fill_small_holes_max_edges: int = 16,
) -> trimesh.Trimesh:
    """Encadeia heurísticas de reparo."""
    m = mesh.copy()

    if remove_ground_shadow:
        try:
            m = remove_ground_shadow_artifacts(
                m,
                aggressive=ground_shadow_aggressive and not ground_shadow_very_aggressive,
                very_aggressive=ground_shadow_very_aggressive,
            )
        except Exception as e:
            print(f"  Aviso: falha na remoção de sombra: {e}")

    if merge_vertices:
        with contextlib.suppress(Exception):
            m.merge_vertices()

    # Fechar buracos pequenos via trimesh
    if fill_small_holes_max_edges > 0:
        try:
            import trimesh.repair as trimesh_repair

            trimesh_repair.fill_holes(m)
        except Exception:
            pass

    if keep_largest:
        m = keep_largest_component(m)

    with contextlib.suppress(Exception):
        m.remove_unreferenced_vertices()

    return m


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Remove pedestal/plataforma da base de modelos 3D",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Modos:
  defeito          — conservador
  --aggressive     — modo forte
  --very-aggressive — modo EXTREMO para pedestais grudados

Exemplos:
  %(prog)s griffin.glb --very-aggressive --fill-holes 24 -o griffin_clean.glb
        """,
    )
    parser.add_argument("input", type=Path, help="Arquivo de entrada (.glb, .obj)")
    parser.add_argument("-o", "--output", type=Path, help="Arquivo de saída (defeito: input_clean.glb)")
    parser.add_argument(
        "--aggressive",
        action="store_true",
        help="Modo agressivo — cilindro na base + peel",
    )
    parser.add_argument(
        "--very-aggressive",
        action="store_true",
        help="Modo EXTREMO — flood-fill + análise de silhueta",
    )
    parser.add_argument(
        "--keep-largest",
        action="store_true",
        default=True,
        help="Mantém só a maior componente conexa",
    )
    parser.add_argument(
        "--fill-holes",
        type=int,
        default=16,
        help="Tenta fechar buracos (0 = desliga)",
    )
    parser.add_argument(
        "--no-keep-largest",
        action="store_true",
        help="Não filtra por maior componente",
    )

    args = parser.parse_args()

    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Erro: arquivo não encontrado: {input_path}", file=sys.stderr)
        return 1

    if args.output:
        output_path = Path(args.output)
    else:
        stem = input_path.stem
        suffix = input_path.suffix
        if suffix in (".glb", ".gltf"):
            output_path = input_path.parent / f"{stem}_clean.glb"
        else:
            output_path = input_path.parent / f"{stem}_clean{suffix}"

    print(f"Carregando: {input_path}")
    try:
        mesh = trimesh.load(str(input_path), force="mesh")
    except Exception as e:
        print(f"Erro ao carregar mesh: {e}", file=sys.stderr)
        return 1

    print(f"  Vértices: {len(mesh.vertices):,}")
    print(f"  Faces: {len(mesh.faces):,}")

    print("\nReparando mesh...")
    mode_str = ""
    if args.very_aggressive:
        mode_str = " (modo EXTREMO)"
    elif args.aggressive:
        mode_str = " (modo agressivo)"
    print(f"  - Anti-sombra{mode_str}")
    if args.keep_largest and not args.no_keep_largest:
        print("  - Manter maior componente")
    if args.fill_holes > 0:
        print("  - Fechar buracos")

    try:
        cleaned = repair_mesh(
            mesh,
            keep_largest=args.keep_largest and not args.no_keep_largest,
            merge_vertices=True,
            remove_ground_shadow=True,
            ground_shadow_aggressive=args.aggressive and not args.very_aggressive,
            ground_shadow_very_aggressive=args.very_aggressive,
            fill_small_holes_max_edges=args.fill_holes,
        )
    except Exception as e:
        print(f"Erro durante reparo: {e}", file=sys.stderr)
        return 1

    print("\nResultado:")
    print(f"  Vértices: {len(cleaned.vertices):,} (antes: {len(mesh.vertices):,})")
    print(f"  Faces: {len(cleaned.faces):,} (antes: {len(mesh.faces):,})")

    ext = output_path.suffix.lower()
    if ext in (".glb", ".gltf"):
        file_type = "glb"
    elif ext == ".obj":
        file_type = "obj"
    elif ext == ".ply":
        file_type = "ply"
    elif ext == ".stl":
        file_type = "stl"
    else:
        file_type = "glb"

    print(f"\nSalvando: {output_path}")
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        cleaned.export(str(output_path), file_type=file_type)
    except Exception as e:
        print(f"Erro ao salvar: {e}", file=sys.stderr)
        return 1

    print("Pronto!")
    return 0


if __name__ == "__main__":
    sys.exit(main())
