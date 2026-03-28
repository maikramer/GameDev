"""
Pós-processamento de meshes Hunyuan3D: componentes desconexas, artefactos finos.

O modelo image-to-3D frequentemente gera várias ilhas (ex.: pés separados do corpo)
ou perde geometria fina; aqui aplicamos heurísticas conservadoras.

Sombras / contact shadows na imagem de referência tendem a virar um disco ou placa
fina na base; removemos por geometria (componentes planos no solo + faces horizontais
na faixa inferior), no mesmo espaço Y-up que o export GLB.
"""

from __future__ import annotations

import contextlib
from typing import Literal

import numpy as np
import trimesh
import trimesh.repair as trimesh_repair
from trimesh.grouping import group_rows
from trimesh.transformations import rotation_matrix

from ..defaults import get_export_rotation_x_rad

try:
    import networkx as nx
except ImportError:
    nx = None  # type: ignore[assignment]


def _rotate_mesh_x(mesh: trimesh.Trimesh, angle: float) -> trimesh.Trimesh:
    """Rotação no eixo X (mesma convenção que ``export._apply_rotation_trimesh``)."""
    m = mesh.copy()
    m.apply_transform(rotation_matrix(angle, [1, 0, 0]))
    return m


def _to_export_y_up(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """Alinha com ``save_mesh(..., rotate=True)`` (``get_export_rotation_x_rad()``)."""
    return _rotate_mesh_x(mesh, float(get_export_rotation_x_rad()))


def _from_export_y_up(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """Inverso de ``_to_export_y_up`` para devolver a mesh no espaço Hunyuan."""
    return _rotate_mesh_x(mesh, -float(get_export_rotation_x_rad()))


def _is_thin_plaque(part: trimesh.Trimesh, *, thin_ratio: float) -> bool:
    """
    Placa fina (disco de sombra, pedestal) em qualquer orientação na AABB.

    Usa min(extents) vs max(extents): sombras podem vir com a malha fina em Y, Z ou X
    conforme o triângulo original (ex.: cilindro Z-up do trimesh).
    """
    e = sorted(float(x) for x in part.extents)
    if len(e) != 3:
        return False
    return e[0] < thin_ratio * e[2]


def _remove_flat_bottom_islands(
    mesh_yup: trimesh.Trimesh,
    *,
    aggressive: bool = False,
) -> trimesh.Trimesh:
    """
    Descarta componentes desconexas que são placas finas coladas ao solo.

    Só actua quando existe pelo menos uma componente que *não* é uma placa fina
    (corpo principal), para não destruir props inteiramente chatos.

    ``aggressive``: placas/cascas mais espessas (ratio maior) e mais volume relativo
    ainda removidos se estiverem na base.
    """
    try:
        parts = mesh_yup.split(only_watertight=False)
    except Exception:
        return mesh_yup
    if len(parts) <= 1:
        return mesh_yup

    y_min_global = float(mesh_yup.bounds[0, 1])
    h_global = float(mesh_yup.extents[1])
    if h_global < 1e-8:
        return mesh_yup

    bottom_eps = (0.055 if aggressive else 0.035) * h_global
    thin_ratio = 0.28 if aggressive else 0.085
    vol_ratio_max = 0.52 if aggressive else 0.22

    def touches_bottom(p: trimesh.Trimesh) -> bool:
        return float(p.bounds[0, 1]) <= y_min_global + bottom_eps

    has_non_plaque = any(not _is_thin_plaque(p, thin_ratio=thin_ratio) or not touches_bottom(p) for p in parts)
    if not has_non_plaque:
        return mesh_yup

    def bbox_volume(p: trimesh.Trimesh) -> float:
        e = p.extents
        return float(e[0] * e[1] * e[2])

    max_vol = max(bbox_volume(p) for p in parts)
    if max_vol < 1e-18:
        return mesh_yup

    kept: list[trimesh.Trimesh] = []
    for p in parts:
        if touches_bottom(p) and _is_thin_plaque(p, thin_ratio=thin_ratio) and bbox_volume(p) < vol_ratio_max * max_vol:
            continue
        kept.append(p)

    if not kept:
        return mesh_yup
    if len(kept) == 1:
        return kept[0]
    try:
        return trimesh.util.concatenate(kept)
    except Exception:
        return mesh_yup


def _peel_bottom_upward_faces(
    mesh_yup: trimesh.Trimesh,
    *,
    band_frac: float = 0.018,
    min_normal_y: float = 0.82,
    max_remove_frac: float = 0.11,
) -> trimesh.Trimesh:
    """
    Remove faces **quase horizontais** (|ny| alto) na faixa mais baixa do bbox.

    Sombras modeladas como placa têm normais para +Y ou -Y (face de cima/baixo);
    só aceitar +Y falhava em muitos GLB (ex.: Godot / pintura).

    Conservador: aborta se a remoção afectar demasiadas faces (p.ex. sola inteira).
    """
    if len(mesh_yup.faces) == 0:
        return mesh_yup

    _ = np.asarray(mesh_yup.face_normals)

    ymin = float(mesh_yup.vertices[:, 1].min())
    ymax = float(mesh_yup.vertices[:, 1].max())
    h = ymax - ymin
    if h < 1e-8:
        return mesh_yup

    band = max(band_frac * h, 1e-6)
    centers = mesh_yup.triangles_center
    normals = mesh_yup.face_normals
    if normals is None or len(normals) != len(mesh_yup.faces):
        return mesh_yup

    ny = np.asarray(normals[:, 1], dtype=np.float64)
    horizontal = np.abs(ny) >= min_normal_y
    remove = (centers[:, 1] <= ymin + band) & horizontal
    n_remove = int(np.count_nonzero(remove))
    if n_remove == 0:
        return mesh_yup
    if n_remove > max_remove_frac * len(mesh_yup.faces):
        return mesh_yup

    keep = ~remove
    try:
        sub = mesh_yup.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            return sub
    except Exception:
        pass
    return mesh_yup


def _remove_bottom_center_cylinder(
    mesh_yup: trimesh.Trimesh,
    *,
    height_frac: float = 0.15,
    radius_frac: float = 0.9,
    min_normal_y: float | None = 0.4,
    max_remove_frac: float = 0.52,
) -> trimesh.Trimesh:
    """
    Remove faces no cilindro vertical sob o centro (XZ) na parte baixa do bbox.

    Corta cascas 3D / “ovos” de sombra grandes que ocupam o solo sob o prop,
    desde que as faces sejam sobretudo horizontais (|ny| alto) ou ``min_normal_y``
    None para corte total na zona (último recurso).
    """
    if len(mesh_yup.faces) == 0:
        return mesh_yup
    bounds = mesh_yup.bounds
    ymin = float(bounds[0, 1])
    h = float(bounds[1, 1] - bounds[0, 1])
    if h < 1e-8:
        return mesh_yup
    cx = 0.5 * (bounds[0, 0] + bounds[1, 0])
    cz = 0.5 * (bounds[0, 2] + bounds[1, 2])
    rx = 0.5 * (bounds[1, 0] - bounds[0, 0])
    rz = 0.5 * (bounds[1, 2] - bounds[0, 2])
    R = radius_frac * max(rx, rz, 1e-9)
    y_cut = ymin + height_frac * h
    centers = mesh_yup.triangles_center
    dx = centers[:, 0] - cx
    dz = centers[:, 2] - cz
    in_disk = (dx * dx + dz * dz) <= (R * R)
    in_bottom = centers[:, 1] <= y_cut
    remove = in_disk & in_bottom
    if min_normal_y is not None:
        _ = np.asarray(mesh_yup.face_normals)
        ny = np.abs(np.asarray(mesh_yup.face_normals)[:, 1], dtype=np.float64)
        remove = remove & (ny >= float(min_normal_y))
    n_remove = int(np.count_nonzero(remove))
    if n_remove == 0:
        return mesh_yup
    if n_remove > max_remove_frac * len(mesh_yup.faces):
        return mesh_yup
    keep = ~remove
    try:
        sub = mesh_yup.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            return sub
    except Exception:
        pass
    return mesh_yup


def remove_small_islands(
    mesh: trimesh.Trimesh,
    *,
    min_face_ratio: float = 0.0002,
    min_faces_abs: int = 48,
) -> trimesh.Trimesh:
    """
    Remove componentes conexas muito pequenas (fragmentos flutuantes, lixo de triângulos).

    Mantém todas as ilhas com ``faces >= max(min_faces_abs, min_face_ratio * maior_ilha)``.
    """
    try:
        parts = mesh.split(only_watertight=False)
    except Exception:
        return mesh
    if len(parts) <= 1:
        return mesh
    main = max(parts, key=lambda m: len(m.faces))
    max_f = max(len(p.faces) for p in parts)
    thr = max(int(min_faces_abs), int(min_face_ratio * max_f))
    kept = [p for p in parts if len(p.faces) >= thr]
    if not kept:
        return main
    if len(kept) == 1:
        return kept[0]
    try:
        return trimesh.util.concatenate(kept)
    except Exception:
        return mesh


def _order_cycle_vertices_planar(mesh: trimesh.Trimesh, cycle: list[int]) -> list[int]:
    """Ordena vértices de um ciclo de buraco (ângulo no plano PCA) para fan triangulation."""
    vidx = np.asarray(cycle, dtype=np.int64)
    if len(vidx) < 3:
        return list(map(int, vidx))
    pts = mesh.vertices[vidx]
    c = pts.mean(axis=0)
    centered = pts - c
    if np.linalg.norm(centered) < 1e-12:
        return list(map(int, vidx))
    _, _, vh = np.linalg.svd(centered, full_matrices=False)
    u, v = vh[0], vh[1]
    ang = np.arctan2(np.dot(centered, v), np.dot(centered, u))
    order = np.argsort(ang)
    return list(map(int, vidx[order]))


def fill_small_boundary_holes(
    mesh: trimesh.Trimesh,
    *,
    max_boundary_edges: int = 16,
) -> trimesh.Trimesh:
    """
    Fecha buracos com contorno curto: primeiro o ``fill_holes`` do trimesh (tri/quad),
    depois triangulação em leque em ciclos de fronteira com ≤ ``max_boundary_edges`` arestas.

    Buracos grandes ou não planares podem ficar por fechar (evita distorcer o modelo).
    """
    m = mesh.copy()
    _fill_small_boundary_holes_inplace(m, max_boundary_edges)
    return m


def _fill_small_boundary_holes_inplace(mesh: trimesh.Trimesh, max_boundary_edges: int) -> None:
    """Igual a ``fill_small_boundary_holes`` mas altera ``mesh`` in-place (menos cópias)."""
    if max_boundary_edges < 3:
        return
    if mesh.is_watertight:
        return
    if nx is None:
        return
    with contextlib.suppress(Exception):
        trimesh_repair.fill_holes(mesh)
    if mesh.is_watertight:
        return
    boundary_groups = group_rows(mesh.edges_sorted, require_count=1)
    if len(boundary_groups) < 3:
        return
    be = mesh.edges[boundary_groups]
    # cycle_basis fica pesado com fronteiras enormes (malhas densas)
    if len(be) > 48_000:
        return
    g = nx.from_edgelist(be)
    new_faces: list[list[int]] = []
    for cycle in nx.cycle_basis(g):
        n = len(cycle)
        if n < 3 or n > max_boundary_edges:
            continue
        ordered = _order_cycle_vertices_planar(mesh, list(map(int, cycle)))
        if len(ordered) < 3:
            continue
        for i in range(1, len(ordered) - 1):
            new_faces.append([ordered[0], ordered[i], ordered[i + 1]])
    if not new_faces:
        return
    nf = np.asarray(new_faces, dtype=np.int64)
    try:
        from trimesh import triangles as tri_mod

        tri_pts = mesh.vertices[nf]
        _, valid = tri_mod.normals(tri_pts)
        nf = nf[valid]
    except Exception:
        pass
    if len(nf) == 0:
        return
    try:
        mesh.faces = np.vstack((mesh.faces, nf))
        mesh.remove_unreferenced_vertices()
        trimesh_repair.fix_normals(mesh, multibody=True)
    except Exception:
        pass


def remove_ground_shadow_artifacts(
    mesh: trimesh.Trimesh,
    *,
    mesh_space: Literal["hunyuan", "y_up"] = "hunyuan",
    y_up_flip_x_rad: float = 0.0,
    aggressive: bool = False,
) -> trimesh.Trimesh:
    """
    Remove disco/placa de sombra na base.

    ``hunyuan`` (defeito): mesh no espaço bruto Hunyuan3D; alinha com a rotação do
    ``save_mesh(rotate=True)`` antes de processar.

    ``y_up``: mesh já em Y-up no motor; não aplica a rotação Hunyuan→export.

    ``y_up_flip_x_rad``: só com ``y_up`` — roda π rad em X **antes** do peel (ficheiro
    gravado de cabeça para baixo); a saída fica em pé + limpa.

    ``aggressive``: opt-in — cilindro estreito sob o centro + peel mais forte (só para
    cascas enormes na base; pode comer geometria lateral se estiver no cone).
    O defeito é conservador (placa fina + peel leve).
    """
    m = mesh.copy()
    if mesh_space == "y_up" and y_up_flip_x_rad != 0.0:
        m = _rotate_mesh_x(m, float(y_up_flip_x_rad))
    yup = _to_export_y_up(m) if mesh_space == "hunyuan" else m
    yup = _remove_flat_bottom_islands(yup, aggressive=aggressive)
    if aggressive:
        # Cilindro mais estreito e normais mais horizontais — menos risco nas laterais.
        yup = _remove_bottom_center_cylinder(
            yup,
            height_frac=0.13,
            radius_frac=0.58,
            min_normal_y=0.68,
            max_remove_frac=0.3,
        )
        yup = _peel_bottom_upward_faces(
            yup,
            band_frac=0.045,
            min_normal_y=0.62,
            max_remove_frac=0.24,
        )
    else:
        yup = _peel_bottom_upward_faces(yup)
    with contextlib.suppress(Exception):
        yup.remove_unreferenced_vertices()
    if len(yup.faces) == 0:
        return mesh
    if mesh_space == "hunyuan":
        return _from_export_y_up(yup)
    return yup


def _bbox_volume(part: trimesh.Trimesh) -> float:
    """Volume do bounding box (produto dos extents)."""
    e = part.extents
    return float(e[0] * e[1] * e[2])


def keep_largest_component(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """
    Mantém a componente conexa "principal" (descarta ilhas/placas).

    Usa volume do bounding box como critério em vez de contagem de faces:
    com octree alto, placas de sombra planas podem ter mais triângulos
    que o modelo 3D real mas volume AABB muito menor.
    """
    mesh = mesh.copy()
    try:
        parts = mesh.split(only_watertight=False)
    except Exception:
        return mesh
    if len(parts) <= 1:
        return mesh
    return max(parts, key=_bbox_volume)


def laplacian_smooth(mesh: trimesh.Trimesh, iterations: int = 1, lamb: float = 0.5) -> trimesh.Trimesh:
    """Suavização Laplaciana leve (reduz aspereza tipo 'argila'; pode arredondar arestas)."""
    if iterations <= 0:
        return mesh
    m = mesh.copy()
    trimesh.smoothing.filter_laplacian(m, iterations=iterations, lamb=lamb)
    return m


def isotropic_remesh(
    mesh: trimesh.Trimesh,
    *,
    resolution: int = 150,
    iterations: int = 5,
    adaptive: bool = True,
    close_holes: bool = True,
    close_holes_max_edges: int = 300,
    taubin_steps: int = 3,
    taubin_lambda: float = 0.5,
    taubin_mu: float = -0.53,
) -> trimesh.Trimesh:
    """Isotropic remeshing via pymeshlab + Taubin smoothing.

    Reconstrói a topologia com triângulos uniformes, eliminando faces
    degeneradas (spikes) sem perder as features da superfície.

    ``resolution`` controla o nível de detalhe (~nº de subdivisões na diagonal).
    ``close_holes`` fecha buracos (marching cubes deixa buracos nas bordas do volume).
    ``taubin_steps`` controla suavização pós-remesh (0 = desliga, preserva volume).
    """
    try:
        import pymeshlab
    except ImportError:
        import warnings
        warnings.warn(
            "pymeshlab não instalado — isotropic_remesh indisponível. "
            "pip install pymeshlab",
            stacklevel=2,
        )
        return mesh

    import tempfile
    from pathlib import Path

    with tempfile.TemporaryDirectory(prefix="remesh_") as tmpdir:
        in_ply = str(Path(tmpdir) / "in.ply")
        out_ply = str(Path(tmpdir) / "out.ply")
        mesh.export(in_ply)

        ms = pymeshlab.MeshSet()
        ms.load_new_mesh(in_ply)

        ms.meshing_repair_non_manifold_edges()
        ms.meshing_repair_non_manifold_vertices()
        ms.meshing_remove_duplicate_faces()
        ms.meshing_remove_duplicate_vertices()
        ms.meshing_remove_unreferenced_vertices()

        if close_holes:
            ms.meshing_close_holes(maxholesize=close_holes_max_edges)

        diag = ms.current_mesh().bounding_box().diagonal()
        target_edge = diag / max(resolution, 10)

        ms.meshing_isotropic_explicit_remeshing(
            iterations=iterations,
            targetlen=pymeshlab.PureValue(target_edge),
            adaptive=adaptive,
            selectedonly=False,
            checksurfdist=True,
            maxsurfdist=pymeshlab.PureValue(target_edge * 0.5),
        )

        if taubin_steps > 0:
            ms.apply_coord_taubin_smoothing(
                stepsmoothnum=taubin_steps,
                lambda_=taubin_lambda,
                mu=taubin_mu,
            )

        ms.save_current_mesh(out_ply)
        return trimesh.load(out_ply, force="mesh")


def repair_mesh(
    mesh: trimesh.Trimesh,
    *,
    keep_largest: bool = True,
    merge_vertices: bool = True,
    remove_ground_shadow: bool = True,
    ground_artifact_mesh_space: Literal["hunyuan", "y_up"] = "hunyuan",
    ground_artifact_y_up_flip_x_rad: float = 0.0,
    ground_shadow_aggressive: bool = False,
    remove_small_island_fragments: bool = True,
    small_island_min_face_ratio: float = 0.0002,
    small_island_min_faces_abs: int = 48,
    fill_small_holes_max_edges: int = 16,
    smooth_iterations: int = 0,
    smooth_lamb: float = 0.45,
    remesh: bool = False,
    remesh_resolution: int = 150,
    remesh_taubin_steps: int = 3,
) -> trimesh.Trimesh:
    """
    Encadeia heurísticas de reparo.

    ``merge_vertices`` ajuda a fechar buracos pequenos de malha e consistência.
    ``remove_ground_shadow`` remove discos/placas de sombra na base (antes do merge,
    para não fundir sombra com o corpo).
    ``ground_artifact_mesh_space``: ``hunyuan`` para saída do pipeline Text3D;
    ``y_up`` para GLB já orientado (ex.: Godot).
    ``ground_shadow_aggressive``: cilindro na base + peel forte (cascas 3D grandes).
    ``remove_small_island_fragments``: apaga ilhas minúsculas (fragmentos flutuantes).
    ``fill_small_holes_max_edges``: fecha buracos com contorno até N arestas (0 = desliga).
    ``remesh``: isotropic remeshing — reconstrói topologia com triângulos uniformes,
    eliminando spikes/faces degeneradas. Requer pymeshlab.
    """
    m = mesh.copy()

    if remove_ground_shadow:
        with contextlib.suppress(Exception):
            m = remove_ground_shadow_artifacts(
                m,
                mesh_space=ground_artifact_mesh_space,
                y_up_flip_x_rad=ground_artifact_y_up_flip_x_rad,
                aggressive=ground_shadow_aggressive,
            )

    if remove_small_island_fragments:
        try:
            ratio = float(small_island_min_face_ratio)
            abs_m = int(small_island_min_faces_abs)
            if ground_shadow_aggressive:
                ratio = max(ratio, 0.0018)
                abs_m = max(abs_m, 256)
            m = remove_small_islands(
                m,
                min_face_ratio=ratio,
                min_faces_abs=abs_m,
            )
        except Exception:
            pass

    if merge_vertices:
        with contextlib.suppress(Exception):
            m.merge_vertices()

    if fill_small_holes_max_edges > 0:
        with contextlib.suppress(Exception):
            _fill_small_boundary_holes_inplace(m, fill_small_holes_max_edges)

    if keep_largest:
        m = keep_largest_component(m)

    if remesh:
        with contextlib.suppress(Exception):
            m = isotropic_remesh(
                m,
                resolution=remesh_resolution,
                taubin_steps=remesh_taubin_steps,
            )

    if smooth_iterations > 0:
        m = laplacian_smooth(m, iterations=smooth_iterations, lamb=smooth_lamb)

    with contextlib.suppress(Exception):
        m.remove_unreferenced_vertices()

    return m
