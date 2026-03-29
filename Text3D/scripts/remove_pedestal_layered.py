#!/usr/bin/env python3
"""
Remove pedestal por camadas horizontais puras.

Remove apenas camadas finas da base onde >80% das faces são
puramente horizontais (normal alinhada >0.95 com o eixo).
Para quando encontra camada com menos horizontais (anatomia).
"""

from __future__ import annotations

import argparse
import sys
from collections import deque
from pathlib import Path

import numpy as np
import trimesh
import trimesh.repair as trimesh_repair


# ---------------------------------------------------------------------------
#  Utilitários de conversão trimesh <-> arrays
# ---------------------------------------------------------------------------

def _boundary_edge_count(mesh: trimesh.Trimesh) -> int:
    """Conta arestas de fronteira (buracos abertos)."""
    try:
        from trimesh.grouping import group_rows
        return len(group_rows(mesh.edges_sorted, require_count=1))
    except Exception:
        return -1


def _repair_open_holes_trimesh(m: trimesh.Trimesh) -> trimesh.Trimesh:
    """Repara non-manifold e fecha buracos (PyMeshLab + PyMeshFix)."""
    if _boundary_edge_count(m) <= 0:
        return m
    try:
        import pymeshlab as _pml
        import tempfile as _tf
        from pathlib import Path as _Path

        with _tf.TemporaryDirectory(prefix="holefix_") as tmpdir:
            in_ply = str(_Path(tmpdir) / "in.ply")
            out_ply = str(_Path(tmpdir) / "out.ply")
            m.export(in_ply)
            ms = _pml.MeshSet()
            ms.load_new_mesh(in_ply)
            ms.meshing_repair_non_manifold_edges()
            ms.meshing_repair_non_manifold_vertices()
            ms.meshing_close_holes(maxholesize=220)
            ms.save_current_mesh(out_ply)
            m_new = trimesh.load(out_ply, force="mesh")
            if len(m_new.faces) > 0:
                m = m_new
    except Exception:
        pass
    if _boundary_edge_count(m) > 0:
        try:
            from pymeshfix import PyTMesh

            verts = np.asarray(m.vertices, dtype=np.float64)
            faces = np.asarray(m.faces, dtype=np.int64)
            mfix = PyTMesh()
            mfix.load_array(verts, faces)
            mfix.fill_small_boundaries(nbe=0, refine=True)
            v, f = mfix.return_arrays()
            m = trimesh.Trimesh(vertices=np.asarray(v), faces=np.asarray(f), process=True)
        except Exception:
            pass
    return m


def _seal_residual_boundary(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """
    Fecha buracos restantes após remesh ou quando o helper leve (220) não chega.
    Usa primeiro PyMeshLab com ``max_hole_size=500`` (como na cascata principal).
    """
    if _boundary_edge_count(mesh) <= 0:
        return mesh
    m = _repair_pymeshlab(mesh, max_hole_size=500)
    if _boundary_edge_count(m) > 0:
        m = _repair_open_holes_trimesh(m)
    return m


def _trimesh_to_arrays(mesh: trimesh.Trimesh):
    """Retorna (vertices, faces) como arrays numpy."""
    return np.asarray(mesh.vertices, dtype=np.float64), np.asarray(mesh.faces, dtype=np.int64)


def _arrays_to_trimesh(verts: np.ndarray, faces: np.ndarray) -> trimesh.Trimesh:
    """Cria trimesh a partir de arrays."""
    return trimesh.Trimesh(vertices=verts, faces=faces, process=True)


# ---------------------------------------------------------------------------
#  ETAPA 1: PyMeshFix — reparo robusto baseado no algoritmo MeshFix de Attene
# ---------------------------------------------------------------------------

def _repair_pymeshfix(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """
    PyMeshFix: wrapper C++ do MeshFix (Marco Attene).
    Excelente para fechar buracos e tornar mesh watertight.
    Usa refinamento para manter a densidade de sampling.
    """
    try:
        from pymeshfix import PyTMesh
        verts, faces = _trimesh_to_arrays(mesh)
        mfix = PyTMesh()
        mfix.load_array(verts, faces)
        mfix.fill_small_boundaries(nbe=0, refine=True)
        v, f = mfix.return_arrays()
        return _arrays_to_trimesh(
            np.asarray(v, dtype=np.float64),
            np.asarray(f, dtype=np.int64),
        )
    except Exception as e:
        print(f"    [pymeshfix] falhou: {e}")
        return mesh


# ---------------------------------------------------------------------------
#  ETAPA 2: PyMeshLab — topologia opcionalmente com close_holes
# ---------------------------------------------------------------------------

def _pymeshlab_topology_and_optional_close(
    mesh: trimesh.Trimesh,
    *,
    close_holes: bool,
    max_hole_size: int = 500,
) -> trimesh.Trimesh:
    """
    Componentes pequenas, faces nulas, non-manifold, duplicados.
    Se ``close_holes`` for True, aplica também ``meshing_close_holes``.
    """
    try:
        import pymeshlab
        import tempfile
        from pathlib import Path

        prefix = "meshlab_repair_" if close_holes else "meshlab_clean_"
        with tempfile.TemporaryDirectory(prefix=prefix) as tmpdir:
            in_ply = str(Path(tmpdir) / "in.ply")
            out_ply = str(Path(tmpdir) / "out.ply")
            mesh.export(in_ply)

            ms = pymeshlab.MeshSet()
            ms.load_new_mesh(in_ply)

            try:
                n_faces = ms.current_mesh().face_number()
                min_component = max(100, int(n_faces * 0.005))
                ms.meshing_remove_small_components(nbfacemin=min_component)
            except Exception:
                pass

            try:
                ms.meshing_remove_null_faces()
            except Exception:
                pass

            ms.meshing_repair_non_manifold_edges()
            ms.meshing_repair_non_manifold_vertices()
            ms.meshing_remove_duplicate_faces()
            ms.meshing_remove_duplicate_vertices()
            ms.meshing_remove_unreferenced_vertices()

            if close_holes:
                ms.meshing_close_holes(maxholesize=max_hole_size)

            ms.save_current_mesh(out_ply)
            return trimesh.load(out_ply, force="mesh")
    except Exception as e:
        if close_holes:
            print(f"    [pymeshlab] falhou: {e}")
        return mesh


def _pymeshlab_clean_topology(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """PyMeshLab só topologia — sem fechar buracos (antes da limpeza agressiva na base)."""
    return _pymeshlab_topology_and_optional_close(mesh, close_holes=False)


def _repair_pymeshlab(mesh: trimesh.Trimesh, *, max_hole_size: int = 500) -> trimesh.Trimesh:
    """PyMeshLab: limpeza + ``meshing_close_holes``."""
    return _pymeshlab_topology_and_optional_close(
        mesh, close_holes=True, max_hole_size=max_hole_size
    )


# ---------------------------------------------------------------------------
#  ETAPA 3: Liepa hole-filling (triangulate + refine + fairing)
# ---------------------------------------------------------------------------

def _repair_liepa(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """
    Algoritmo Liepa (2003) — triangulate + refine + fairing.
    Preenche buracos com geometria suave que respeita a curvatura local.
    Baseado no paper "Filling holes in meshes" (Eurographics/SIGGRAPH).
    """
    try:
        import igl
        from hole_filling import close_hole, triangulation_refine_leipa, mesh_fair_laplacian_energy

        verts, faces = _trimesh_to_arrays(mesh)

        # Detectar loops de fronteira usando igl.boundary_loop
        out_v = verts.copy()
        out_f = faces.copy()
        max_iter = 50
        for _ in range(max_iter):
            try:
                b = igl.boundary_loop(out_f)
            except Exception:
                break
            if len(b) == 0:
                break
            out_v, out_f = close_hole(out_v, out_f, b)

        return _arrays_to_trimesh(out_v, out_f)
    except Exception as e:
        print(f"    [liepa] falhou: {e}")
        return mesh


# ---------------------------------------------------------------------------
#  ETAPA 4: MeshLib — universal metric hole filling
# ---------------------------------------------------------------------------

def _repair_meshlib(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """
    MeshLib: preenchimento de buracos com métrica universal otimizada.
    Detecta cada buraco e preenche com geometria que minimiza distorção.
    """
    try:
        import meshlib.mrmeshpy as mrmeshpy
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory(prefix="meshlib_repair_") as tmpdir:
            in_stl = str(Path(tmpdir) / "in.stl")
            out_stl = str(Path(tmpdir) / "out.stl")
            mesh.export(in_stl)

            ml_mesh = mrmeshpy.loadMesh(in_stl)
            hole_edges = ml_mesh.topology.findHoleRepresentiveEdges()

            for e in hole_edges:
                params = mrmeshpy.FillHoleParams()
                params.metric = mrmeshpy.getUniversalMetric(ml_mesh)
                mrmeshpy.fillHole(ml_mesh, e, params)

            mrmeshpy.saveMesh(ml_mesh, out_stl)
            return trimesh.load(out_stl, force="mesh")
    except Exception as e:
        print(f"    [meshlib] falhou: {e}")
        return mesh


def _cascade_close_holes(mesh: trimesh.Trimesh, *, tag: str = "[Fechar]") -> trimesh.Trimesh:
    """Cascata PyMeshLab → PyMeshFix → Liepa → MeshLib enquanto houver fronteira."""
    m = mesh
    boundary = _boundary_edge_count(m)
    if boundary <= 0:
        return m
    print(f"    {tag} Cascata de fechamento de buracos...")
    if boundary > 0:
        print("    [1/4] PyMeshLab close_holes...")
        m_new = _repair_pymeshlab(m, max_hole_size=500)
        b_new = _boundary_edge_count(m_new)
        if b_new < boundary:
            m = m_new
            boundary = b_new
            print(f"          -> {boundary} arestas restantes")
        else:
            print(f"          -> sem melhora ({b_new})")

    if boundary > 0:
        print("    [2/4] PyMeshFix (MeshFix de Attene)...")
        m_new = _repair_pymeshfix(m)
        b_new = _boundary_edge_count(m_new)
        if b_new < boundary:
            m = m_new
            boundary = b_new
            print(f"          -> {boundary} arestas restantes")
        else:
            print(f"          -> sem melhora ({b_new})")

    if boundary > 0:
        print("    [3/4] Liepa triangulate+refine+fair...")
        m_new = _repair_liepa(m)
        b_new = _boundary_edge_count(m_new)
        if b_new < boundary:
            m = m_new
            boundary = b_new
            print(f"          -> {boundary} arestas restantes")
        else:
            print(f"          -> sem melhora ({b_new})")

    if boundary > 0:
        print("    [4/4] MeshLib universal metric fill...")
        m_new = _repair_meshlib(m)
        b_new = _boundary_edge_count(m_new)
        if b_new < boundary:
            m = m_new
            boundary = b_new
            print(f"          -> {boundary} arestas restantes")
        else:
            print(f"          -> sem melhora ({b_new})")

    return m


def _dissolve_fill_plate_triangles(
    mesh: trimesh.Trimesh,
    axis: int,
    direction: int,
    *,
    band_frac: float = 0.65,
    area_vs_median: float = 5.2,
    aspect_min: float = 7.2,
    area_vs_median_loose: float = 14.0,
    aspect_loose_min: float = 4.8,
    global_area_percentile: float = 99.55,
    global_aspect_min: float = 9.5,
    max_remove_frac: float = 0.14,
) -> trimesh.Trimesh:
    """
    Remove triângulos típicos de patches de hole-filling: área muito acima da mediana
    na banda inferior e arestas muito desiguais (chapas/“agulhas”), mais outliers
    globais (triângulos enormes e alongados, p.ex. placas laterais).

    Valores um pouco mais agressivos que a primeira versão; limitado por ``max_remove_frac``.

    Abre buracos de propósito para o reparo seguinte fechar melhor.
    """
    try:
        m = mesh.copy()
        if len(m.faces) == 0:
            return mesh

        areas = m.area_faces
        bottom = _bottom_zone_face_mask(m, axis, direction, band_frac)
        med = float(np.median(areas[bottom])) if np.any(bottom) else float(np.median(areas))
        if med < 1e-18:
            return mesh

        p_global = float(np.percentile(areas, global_area_percentile))

        remove = np.zeros(len(m.faces), dtype=bool)

        for i in range(len(m.faces)):
            a = float(areas[i])
            tri = m.vertices[m.faces[i]]
            e0 = tri[1] - tri[0]
            e1 = tri[2] - tri[0]
            e2 = tri[2] - tri[1]
            l0 = float(np.linalg.norm(e0))
            l1 = float(np.linalg.norm(e1))
            l2 = float(np.linalg.norm(e2))
            le = max(l0, l1, l2)
            se = min(l0, l1, l2)
            aspect = le / max(se, 1e-9)

            global_needle = a >= p_global and aspect >= global_aspect_min
            if global_needle:
                remove[i] = True
                continue

            if not bottom[i]:
                continue
            if a < med * 2.5:
                continue

            needle = aspect >= aspect_min and a >= area_vs_median * med
            slab = aspect >= aspect_loose_min and a >= area_vs_median_loose * med
            if needle or slab:
                remove[i] = True

        n_rm = int(np.count_nonzero(remove))
        n_face = len(m.faces)
        if n_rm == 0 or n_rm > max_remove_frac * n_face:
            return mesh

        keep = ~remove
        sub = m.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            sub.remove_unreferenced_vertices()
            print(
                f"    [Dissolver] Removendo {n_rm:,} faces de placas de fecho "
                f"(banda base + outliers globais)"
            )
            return sub
    except Exception as e:
        print(f"    [Dissolver] falhou: {e}")
    return mesh


# ---------------------------------------------------------------------------
#  Pipeline completa de repair
# ---------------------------------------------------------------------------

def repair_mesh_complete(
    mesh: trimesh.Trimesh,
    *,
    post_remesh: bool = False,
) -> trimesh.Trimesh:
    """
    Ordem restaurada (malha sólida antes de cortes agressivos):

    1. **Pré-ligeiro:** faces muito pequenas, componentes óbvias, merge — sem spikes/teias/placas.
    2. **Fechar:** cascata PyMeshLab, PyMeshFix, Liepa, MeshLib.
    2b. **Dissolver** placas de fecho (triângulos enormes/alongados) e **re-fechar** a cascata.
    3. **Pós-fechamento:** spikes, teias planas, placas (com reparo de buracos entre passos).
    4. Normais, componentes, selagem opcional, remesh opcional.
    """
    m = mesh.copy()
    r_axis, r_direction = detect_base_axis(m)
    initial_boundary = _boundary_edge_count(m)
    print(f"    Arestas de fronteira iniciais: {initial_boundary}")

    # 0. Pré-ligeiro apenas (não cortar anatomia com malha aberta)
    try:
        m = _clean_small_faces(m, area_threshold=1e-7, iterations=2)
    except Exception as e:
        print(f"    [Pré] faces pequenas: {e}")

    try:
        parts = m.split(only_watertight=False)
        if len(parts) > 1:
            main_size = max(len(p.faces) for p in parts)
            min_size = int(main_size * 0.01)
            significant = [p for p in parts if len(p.faces) >= min_size]
            if significant and len(significant) < len(parts):
                m = trimesh.util.concatenate(significant)
                m.remove_unreferenced_vertices()
                print(f"    [Pré] Componentes: {len(parts)} -> {len(significant)}")
    except Exception as e:
        print(f"    [Pré] componentes: {e}")

    try:
        m.merge_vertices()
    except Exception:
        pass

    boundary = _boundary_edge_count(m)
    boundary_before_cascade = boundary
    if boundary > 0:
        print(f"    Após merge: {boundary} arestas (antes da cascata [Fechar])")

    m = _cascade_close_holes(m, tag="[Fechar]")

    n_faces_before_dissolve = len(m.faces)
    m = _dissolve_fill_plate_triangles(m, r_axis, r_direction)
    if len(m.faces) < n_faces_before_dissolve:
        bd = _boundary_edge_count(m)
        print(f"    [Dissolver] fronteira após remoção: {bd} arestas")
        if bd > 0:
            m = _cascade_close_holes(m, tag="[Re-fechar]")

    # Limpeza agressiva *depois* de ter malha fechada (ou quase)
    try:
        print("    [Clean] Removendo artefactos pós-fechamento...")
        m = _clean_small_faces(m, area_threshold=1e-8, iterations=3)

        m = _remove_spikes_at_base(m, r_axis, r_direction, max_spike_angle=10.0)

        try:
            b_sp = _boundary_edge_count(m)
            if b_sp > 0:
                print(f"    [Clean] Fechando {b_sp} buracos pós-spike...")
                m = _repair_open_holes_trimesh(m)
                print(f"          -> {_boundary_edge_count(m)} arestas restantes")
        except Exception as e:
            print(f"    [Clean] Fechamento pós-spike falhou: {e}")

        try:
            m_pw = _remove_planar_webs_at_base(m, r_axis, r_direction)
            if m_pw is not m and len(m_pw.faces) < len(m.faces):
                m = m_pw
                b_pw = _boundary_edge_count(m)
                if b_pw > 0:
                    print(f"    [Clean] Fechando {b_pw} buracos pós-teias planas...")
                    m = _repair_open_holes_trimesh(m)
                    print(f"          -> {_boundary_edge_count(m)} arestas restantes")
        except Exception as e:
            print(f"    [Clean] teias planas falhou: {e}")

        try:
            m_pl = _remove_thin_plaque_clusters_at_base(m, r_axis, r_direction)
            if m_pl is not m and len(m_pl.faces) < len(m.faces):
                m = m_pl
                b_pl = _boundary_edge_count(m)
                if b_pl > 0:
                    print(f"    [Clean] Fechando {b_pl} buracos pós-placas...")
                    m = _repair_open_holes_trimesh(m)
                    print(f"          -> {_boundary_edge_count(m)} arestas restantes")
        except Exception as e:
            print(f"    [Clean] placas finas falhou: {e}")

        parts = m.split(only_watertight=False)
        if len(parts) > 1:
            main_size = max(len(p.faces) for p in parts)
            significant = [p for p in parts if len(p.faces) >= int(main_size * 0.03)]
            if significant:
                m = trimesh.util.concatenate(significant)
                m.remove_unreferenced_vertices()
                print(f"          -> {len(significant)} componentes, {len(m.faces):,} faces")
    except Exception as e:
        print(f"    [Clean] falhou: {e}")

    # Limpeza final
    try:
        trimesh_repair.fix_normals(m, multibody=True)
    except Exception:
        pass

    # Remove componentes pequenas (artefatos)
    try:
        parts = m.split(only_watertight=False)
        if len(parts) > 1:
            main_size = max(len(p.faces) for p in parts)
            min_size = max(1000, int(main_size * 0.05))
            significant = [p for p in parts if len(p.faces) >= min_size]
            if significant and len(significant) < len(parts):
                m = trimesh.util.concatenate(significant)
    except Exception:
        pass

    # Maior componente
    try:
        parts = m.split(only_watertight=False)
        if len(parts) > 1:
            m = max(parts, key=lambda p: len(p.faces))
    except Exception:
        pass

    try:
        m.remove_unreferenced_vertices()
    except Exception:
        pass

    b_pre = _boundary_edge_count(m)
    if b_pre > 0:
        print(f"    [Pós-pré] Fechando {b_pre} arestas antes do remesh...")
        m = _seal_residual_boundary(m)

    # Pós-tratamento opcional: remesh + smoothing para uniformizar a geometria reparada.
    try:
        if not post_remesh:
            residual_boundary = _boundary_edge_count(m)
            if residual_boundary > 0:
                print(
                    "    [Pós] Remesh desativado; fechando fronteira restante sem re-mesh..."
                )
                m = _seal_residual_boundary(m)
        else:
            import pymeshlab
            import tempfile
            from pathlib import Path

            print("    [Pós] PyMeshLab remesh localizado + smoothing...")
            with tempfile.TemporaryDirectory(prefix="remesh_post_") as tmpdir:
                in_ply = str(Path(tmpdir) / "in.ply")
                out_ply = str(Path(tmpdir) / "out.ply")
                m.export(in_ply)

                ms = pymeshlab.MeshSet()
                ms.load_new_mesh(in_ply)

                ms.meshing_repair_non_manifold_edges()
                ms.meshing_repair_non_manifold_vertices()
                ms.meshing_remove_duplicate_faces()
                ms.meshing_remove_duplicate_vertices()
                ms.meshing_remove_unreferenced_vertices()

                diag = ms.current_mesh().bounding_box().diagonal()

                try:
                    ms.meshing_close_holes(maxholesize=500)
                except Exception:
                    pass

                # Isotropic remesh leve (só para suavizar patches reparados)
                target_edge = diag / 150
                ms.meshing_isotropic_explicit_remeshing(
                    iterations=3,
                    targetlen=pymeshlab.PureValue(target_edge),
                    adaptive=True,
                    selectedonly=False,
                    checksurfdist=True,
                    maxsurfdist=pymeshlab.PureValue(target_edge * 0.5),
                )

                # Taubin smoothing (preserva volume, suaviza patches irregulares)
                ms.apply_coord_taubin_smoothing(
                    stepsmoothnum=5,
                    lambda_=0.5,
                    mu=-0.53,
                )

                ms.save_current_mesh(out_ply)
                m_new = trimesh.load(out_ply, force="mesh")
                if len(m_new.faces) > 0:
                    m = m_new
                    if _boundary_edge_count(m) > 0:
                        m = _seal_residual_boundary(m)
                    print(f"          -> {len(m.vertices):,} verts, {len(m.faces):,} faces")
    except Exception as e:
        print(f"    [Pós] remesh falhou: {e}")

    if _boundary_edge_count(m) <= 0 and not m.is_watertight:
        try:
            print("    [Pós] PyMeshFix para corrigir malha não-watertight (sem fronteira)...")
            m = _repair_pymeshfix(m)
        except Exception:
            pass

    final_boundary = _boundary_edge_count(m)
    print(
        f"    Resultado: {initial_boundary} -> {final_boundary} arestas de fronteira "
        f"(antes da cascata [Fechar]: {boundary_before_cascade})"
    )
    print(f"    Watertight: {m.is_watertight}")

    return m


def _clean_small_faces(mesh: trimesh.Trimesh, *, area_threshold: float = 1e-6, iterations: int = 3) -> trimesh.Trimesh:
    """
    Limpa faces muito pequenas (flutuantes ou degeneradas).

    Remove faces com área < threshold e componentes pequenas.
    """
    m = mesh.copy()
    for _ in range(iterations):
        if len(m.faces) == 0:
            break
        # Remover faces muito pequenas
        face_areas = m.area_faces
        small_faces = face_areas < area_threshold
        if np.count_nonzero(small_faces) > 0:
            keep = ~small_faces
            try:
                sub = m.submesh([np.where(keep)[0]], append=True, only_watertight=False)
                if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
                    sub.remove_unreferenced_vertices()
                    m = sub
            except Exception:
                break
        else:
            break
    return m


def _remove_thin_faces_at_base(mesh: trimesh.Trimesh, base_axis: int, *, min_edge_ratio: float = 0.1) -> trimesh.Trimesh:
    """
    Remove faces "finas" na base - faces onde a aresta na direção do eixo é
    desproporcionalmente curta em relação às outras arestas (indica artefato).
    """
    try:
        m = mesh.copy()
        coords = m.vertices[:, base_axis]
        base = coords.min()
        height = coords.max() - coords.min()

        # Considerar apenas faces na base (últimos 5%)
        centers = m.triangles_center
        bottom_zone = centers[:, base_axis] <= base + 0.05 * height

        # Analisar arestas de cada face
        thin_faces = []
        for i, face in enumerate(m.faces):
            if not bottom_zone[i]:
                continue
            # Obter vértices da face
            v0, v1, v2 = m.vertices[face]
            # Comprimentos das arestas na direção do eixo base
            edges_axis = [
                abs(v0[base_axis] - v1[base_axis]),
                abs(v1[base_axis] - v2[base_axis]),
                abs(v2[base_axis] - v0[base_axis]),
            ]
            # Comprimentos das arestas no plano perpendicular
            def perp_len(v_a, v_b):
                diff = v_a - v_b
                diff[base_axis] = 0  # Ignorar componente do eixo base
                return np.linalg.norm(diff)
            edges_perp = [
                perp_len(v0, v1),
                perp_len(v1, v2),
                perp_len(v2, v0),
            ]
            max_perp = max(edges_perp)
            min_axis = min(edges_axis)
            if max_perp > 0 and min_axis / max_perp < min_edge_ratio:
                thin_faces.append(i)

        if len(thin_faces) > 0 and len(thin_faces) < len(m.faces) * 0.3:
            print(f"          -> Removendo {len(thin_faces):,} faces finas na base")
            keep_mask = np.ones(len(m.faces), dtype=bool)
            keep_mask[thin_faces] = False
            sub = m.submesh([np.where(keep_mask)[0]], append=True, only_watertight=False)
            if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
                sub.remove_unreferenced_vertices()
                return sub
    except Exception as e:
        print(f"    [thin_faces] falhou: {e}")
    return mesh


def _remove_spikes_at_base(
    mesh: trimesh.Trimesh, base_axis: int, base_direction: int, *, max_spike_angle: float = 15.0
) -> trimesh.Trimesh:
    """
    Remove "spikes" (pontas) na base - faces com ângulo muito agudo.

    Detecta faces onde um vértice forma um ângulo muito pequeno com os outros dois,
    criando uma ponta fina que geralmente é um artefato.
    """
    try:
        m = mesh.copy()
        bottom_zone = _bottom_zone_face_mask(m, base_axis, base_direction, 0.08)

        spike_faces = []
        for i, face in enumerate(m.faces):
            if not bottom_zone[i]:
                continue

            v0, v1, v2 = m.vertices[face]

            # Vetores das arestas
            e0 = v1 - v0
            e1 = v2 - v0
            e2 = v2 - v1

            # Comprimentos
            l0 = np.linalg.norm(e0)
            l1 = np.linalg.norm(e1)
            l2 = np.linalg.norm(e2)

            if l0 < 1e-10 or l1 < 1e-10 or l2 < 1e-10:
                continue

            # Calcular ângulos (usando dot product)
            # Ângulo em v0
            cos_angle_0 = np.dot(e0, e1) / (l0 * l1)
            angle_0 = np.degrees(np.arccos(np.clip(cos_angle_0, -1, 1)))

            # Ângulo em v1
            e0_inv = v0 - v1
            e2_v1 = v2 - v1
            cos_angle_1 = np.dot(e0_inv, e2_v1) / (np.linalg.norm(e0_inv) * l2)
            angle_1 = np.degrees(np.arccos(np.clip(cos_angle_1, -1, 1)))

            # Ângulo em v2
            e1_inv = v0 - v2
            e2_inv = v1 - v2
            cos_angle_2 = np.dot(e1_inv, e2_inv) / (np.linalg.norm(e1_inv) * np.linalg.norm(e2_inv))
            angle_2 = np.degrees(np.arccos(np.clip(cos_angle_2, -1, 1)))

            # Se qualquer ângulo for muito pequeno, é um spike
            min_angle = min(angle_0, angle_1, angle_2)
            if min_angle < max_spike_angle:
                spike_faces.append(i)

        if len(spike_faces) > 0 and len(spike_faces) < len(m.faces) * 0.2:
            print(f"          -> Removendo {len(spike_faces):,} faces spike na base")
            keep_mask = np.ones(len(m.faces), dtype=bool)
            keep_mask[spike_faces] = False
            sub = m.submesh([np.where(keep_mask)[0]], append=True, only_watertight=False)
            if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
                sub.remove_unreferenced_vertices()
                return sub
    except Exception as e:
        print(f"    [spikes] falhou: {e}")
    return mesh


def detect_base_axis(mesh: trimesh.Trimesh) -> tuple[int, int]:
    """Detecta qual eixo contém a base do modelo."""
    best_axis = 1
    best_direction = -1
    best_score = 0

    for axis in range(3):
        coords = mesh.vertices[:, axis]
        min_c, max_c = coords.min(), coords.max()
        range_c = max_c - min_c

        if range_c < 1e-6:
            continue

        lower_zone = coords <= (min_c + 0.05 * range_c)
        n_lower = np.count_nonzero(lower_zone)
        upper_zone = coords >= (max_c - 0.05 * range_c)
        n_upper = np.count_nonzero(upper_zone)

        score = abs(n_lower - n_upper)
        if score > best_score:
            best_score = score
            best_axis = axis
            best_direction = -1 if n_lower > n_upper else 1

    return best_axis, best_direction


def _bottom_zone_face_mask(
    mesh: trimesh.Trimesh, axis: int, direction: int, band_frac: float
) -> np.ndarray:
    coords = mesh.vertices[:, axis]
    h = float(coords.max() - coords.min())
    if h < 1e-8:
        return np.zeros(len(mesh.faces), dtype=bool)
    cmin, cmax = float(coords.min()), float(coords.max())
    centers = mesh.triangles_center
    if direction == -1:
        return centers[:, axis] <= cmin + band_frac * h
    return centers[:, axis] >= cmax - band_frac * h


def _remove_planar_webs_at_base(
    mesh: trimesh.Trimesh,
    axis: int,
    direction: int,
    *,
    bottom_frac: float = 0.10,
    normal_dot: float = 0.88,
    thin_ratio_max: float = 0.182,
    edge_aspect_min: float = 4.0,
    max_remove_frac: float = 0.12,
) -> trimesh.Trimesh:
    """Teias planas entre garras: horizontais, chapas finas ou triângulos muito alongados."""
    try:
        m = mesh.copy()
        h = float(m.vertices[:, axis].max() - m.vertices[:, axis].min())
        if h < 1e-8:
            return mesh

        bottom = _bottom_zone_face_mask(m, axis, direction, bottom_frac)
        normals = m.face_normals
        n_axis = np.abs(normals[:, axis])
        horizontal = n_axis >= normal_dot
        ax1 = (axis + 1) % 3
        ax2 = (axis + 2) % 3
        remove = np.zeros(len(m.faces), dtype=bool)

        for i in range(len(m.faces)):
            if not bottom[i] or not horizontal[i]:
                continue
            tri = m.vertices[m.faces[i]]
            ext_axis = float(tri[:, axis].max() - tri[:, axis].min())
            dx = float(tri[:, ax1].max() - tri[:, ax1].min())
            dy = float(tri[:, ax2].max() - tri[:, ax2].min())
            span_perp = float(np.hypot(dx, dy))
            thin_ratio = ext_axis / max(span_perp, 1e-9)

            e0 = tri[1] - tri[0]
            e1 = tri[2] - tri[0]
            e2 = tri[2] - tri[1]
            l0 = float(np.linalg.norm(e0))
            l1 = float(np.linalg.norm(e1))
            l2 = float(np.linalg.norm(e2))
            le = max(l0, l1, l2)
            se = min(l0, l1, l2)
            aspect = le / max(se, 1e-9)

            v0, v1, v2 = tri[0], tri[1], tri[2]
            ev0_a, ev0_b = v1 - v0, v2 - v0
            ev1_a, ev1_b = v0 - v1, v2 - v1
            ev2_a, ev2_b = v0 - v2, v1 - v2
            n0 = float(np.linalg.norm(ev0_a) * np.linalg.norm(ev0_b))
            n1 = float(np.linalg.norm(ev1_a) * np.linalg.norm(ev1_b))
            n2 = float(np.linalg.norm(ev2_a) * np.linalg.norm(ev2_b))
            a0 = float(np.degrees(np.arccos(np.clip(float(np.dot(ev0_a, ev0_b)) / max(n0, 1e-9), -1.0, 1.0))))
            a1 = float(np.degrees(np.arccos(np.clip(float(np.dot(ev1_a, ev1_b)) / max(n1, 1e-9), -1.0, 1.0))))
            a2 = float(np.degrees(np.arccos(np.clip(float(np.dot(ev2_a, ev2_b)) / max(n2, 1e-9), -1.0, 1.0))))
            amin, amax = min(a0, a1, a2), max(a0, a1, a2)

            is_pancake = thin_ratio <= thin_ratio_max and span_perp >= 0.004 * h
            is_needle = aspect >= edge_aspect_min
            is_obtuse_slab = amin >= 52.0 and amax >= 108.0 and aspect >= 2.8

            if is_pancake or is_needle or is_obtuse_slab:
                remove[i] = True

        n_rm = int(np.count_nonzero(remove))
        if n_rm == 0 or n_rm > max_remove_frac * len(m.faces):
            return mesh

        keep = ~remove
        sub = m.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            sub.remove_unreferenced_vertices()
            print(f"          -> Removendo {n_rm:,} faces de teias planas na base")
            return sub
    except Exception as e:
        print(f"    [planar_webs] falhou: {e}")
    return mesh


def _remove_thin_plaque_clusters_at_base(
    mesh: trimesh.Trimesh,
    axis: int,
    direction: int,
    *,
    band_frac: float = 0.155,
    normal_dot: float = 0.86,
    max_thickness_ratio: float = 0.108,
    min_faces: int = 16,
    max_cluster_frac: float = 0.22,
    max_remove_frac: float = 0.25,
    min_dot_ref_normal: float = 0.72,
) -> trimesh.Trimesh:
    """
    Remove placas horizontais finas conexas (pedestal/sombra grudada à anatomia).

    Agrupa faces horizontais na base por adjacência; remove componentes muito
    chatos no eixo de suporte e com normais coerentes (chapa), não superfícies curvas.
    """
    try:
        m = mesh.copy()
        h = float(m.vertices[:, axis].max() - m.vertices[:, axis].min())
        if h < 1e-8 or len(m.faces) == 0:
            return mesh

        bottom = _bottom_zone_face_mask(m, axis, direction, band_frac)
        normals = m.face_normals
        n_axis = np.abs(normals[:, axis])
        horizontal = bottom & (n_axis >= normal_dot)
        if not np.any(horizontal):
            return mesh

        adj = m.face_adjacency
        nbr: list[list[int]] = [[] for _ in range(len(m.faces))]
        for a, b in adj:
            a, b = int(a), int(b)
            if horizontal[a] and horizontal[b]:
                nbr[a].append(b)
                nbr[b].append(a)

        visited = np.zeros(len(m.faces), dtype=bool)
        to_remove = np.zeros(len(m.faces), dtype=bool)
        ax1 = (axis + 1) % 3
        ax2 = (axis + 2) % 3
        n_face = len(m.faces)

        for seed in range(n_face):
            if not horizontal[seed] or visited[seed]:
                continue
            comp: list[int] = []
            q = deque([seed])
            visited[seed] = True
            while q:
                f = q.popleft()
                comp.append(f)
                for j in nbr[f]:
                    if not visited[j]:
                        visited[j] = True
                        q.append(j)

            n_c = len(comp)
            if n_c < min_faces or n_c > max_cluster_frac * n_face:
                continue

            fi = np.array(comp, dtype=np.int64)
            verts_idx = m.faces[fi].ravel()
            pts = m.vertices[verts_idx]
            ext_axis = float(pts[:, axis].max() - pts[:, axis].min())
            span = float(
                np.hypot(
                    pts[:, ax1].max() - pts[:, ax1].min(),
                    pts[:, ax2].max() - pts[:, ax2].min(),
                )
            )
            ratio = ext_axis / max(span, 1e-9)
            if ratio > max_thickness_ratio:
                continue

            nc = normals[fi]
            ref = nc[0] / max(float(np.linalg.norm(nc[0])), 1e-9)
            dots = np.abs(nc @ ref)
            if float(np.min(dots)) < min_dot_ref_normal:
                continue

            to_remove[fi] = True

        n_rm = int(np.count_nonzero(to_remove))
        if n_rm == 0 or n_rm > max_remove_frac * n_face:
            return mesh

        keep = ~to_remove
        sub = m.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            sub.remove_unreferenced_vertices()
            print(f"          -> Removendo {n_rm:,} faces em placas planas finas (clusters)")
            return sub
    except Exception as e:
        print(f"    [plaque_clusters] falhou: {e}")
    return mesh


def remove_pedestal_layers(
    mesh: trimesh.Trimesh,
    axis: int,
    direction: int,
    *,
    layer_depth_frac: float = 0.015,  # 1.5% da altura por camada
    min_horizontal_pct: float = 0.80,  # 80% mínimo de faces horizontais
    normal_threshold: float = 0.95,  # Puramente horizontal
    max_layers: int = 5,
    max_remove_frac: float = 0.35,
) -> trimesh.Trimesh:
    """Remove pedestal removendo camadas puramente horizontais."""
    if len(mesh.faces) == 0:
        return mesh

    coords_all = mesh.vertices[:, axis]
    height = coords_all.max() - coords_all.min()
    base_value = coords_all.min() if direction == -1 else coords_all.max()

    if height < 1e-8:
        return mesh

    layer_depth = layer_depth_frac * height
    centers = mesh.triangles_center
    normals = mesh.face_normals
    n_axis = np.abs(normals[:, axis])
    is_very_horizontal = n_axis >= normal_threshold

    to_remove = np.zeros(len(mesh.faces), dtype=bool)
    removed_total = 0

    for layer in range(max_layers):
        # Limites da camada
        if direction == -1:
            layer_start = base_value + (layer * layer_depth)
            layer_end = base_value + ((layer + 1) * layer_depth)
            in_layer = (centers[:, axis] >= layer_start) & (centers[:, axis] < layer_end)
        else:
            layer_start = base_value - (layer * layer_depth)
            layer_end = base_value - ((layer + 1) * layer_depth)
            in_layer = (centers[:, axis] <= layer_start) & (centers[:, axis] > layer_end)

        n_in_layer = np.count_nonzero(in_layer)
        if n_in_layer == 0:
            print(f"    Camada {layer + 1}: vazia, parando")
            break

        n_horizontal = np.count_nonzero(in_layer & is_very_horizontal)
        horizontal_pct = n_horizontal / n_in_layer if n_in_layer > 0 else 0

        print(f"    Camada {layer + 1}: {n_in_layer:,} faces, {horizontal_pct*100:.0f}% puramente horizontais")

        # Só remove se >80% são horizontais puras
        if horizontal_pct >= min_horizontal_pct:
            layer_remove_mask = in_layer & is_very_horizontal
            n_to_remove = np.count_nonzero(layer_remove_mask)
            to_remove[layer_remove_mask] = True
            removed_total += n_to_remove
            print(f"      -> Removendo {n_to_remove:,} faces")
        else:
            print(f"      -> Parando (ratio insuficiente)")
            break

    n_remove = int(np.count_nonzero(to_remove))
    if n_remove == 0:
        print("    Nenhuma face removida")
        return mesh

    if n_remove > max_remove_frac * len(mesh.faces):
        print(f"    Aviso: remoção ({n_remove} faces) excede {max_remove_frac*100:.0f}% — abortando")
        return mesh

    print(f"    Total removido: {n_remove:,} faces ({n_remove/len(mesh.faces)*100:.1f}%)")

    keep = ~to_remove
    try:
        sub = mesh.submesh([np.where(keep)[0]], append=True, only_watertight=False)
        if isinstance(sub, trimesh.Trimesh) and len(sub.faces) > 0:
            sub.remove_unreferenced_vertices()

            # Limpeza: remover componentes pequenas/isoladas (artefatos)
            components = sub.split(only_watertight=False)
            if len(components) > 1:
                # Manter apenas componentes significativas (>5% da maior ou >2000 faces)
                main_size = max(len(c.faces) for c in components)
                min_size = max(2000, int(main_size * 0.05))
                significant = [c for c in components if len(c.faces) >= min_size]
                if len(significant) > 0:
                    sub = trimesh.util.concatenate(significant)
                    sub.remove_unreferenced_vertices()

            # PyMeshLab: limpeza adicional de componentes pequenas e faces null
            try:
                import pymeshlab
                import tempfile
                from pathlib import Path

                with tempfile.TemporaryDirectory(prefix="clean_") as tmpdir:
                    in_ply = str(Path(tmpdir) / "in.ply")
                    out_ply = str(Path(tmpdir) / "out.ply")
                    sub.export(in_ply)

                    ms = pymeshlab.MeshSet()
                    ms.load_new_mesh(in_ply)

                    n_faces = ms.current_mesh().face_number()
                    min_component = max(50, int(n_faces * 0.02))
                    ms.meshing_remove_small_components(nbfacemin=min_component)
                    ms.meshing_remove_null_faces()
                    ms.meshing_remove_duplicate_faces()
                    ms.meshing_remove_duplicate_vertices()
                    ms.meshing_remove_unreferenced_vertices()

                    ms.save_current_mesh(out_ply)
                    cleaned = trimesh.load(out_ply, force="mesh")
                    if len(cleaned.faces) > 0:
                        return cleaned
            except Exception:
                pass

            return sub
    except Exception as e:
        print(f"    Erro na submesh: {e}")

    return mesh


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Remove pedestal por camadas puramente horizontais",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Remove apenas camadas onde >80%% das faces são puramente horizontais.
Preserva anatomia (pés, cauda) que têm geometria curva.

Exemplos:
  %(prog)s griffin.glb -o griffin_clean.glb
  %(prog)s dragao.glb --threshold 0.90 -o dragao_clean.glb
        """,
    )
    parser.add_argument("input", type=Path, help="Arquivo de entrada (.glb, .obj)")
    parser.add_argument("-o", "--output", type=Path, help="Arquivo de saída")
    parser.add_argument(
        "--threshold", type=float, default=0.95,
        help="Alinhamento mínimo com eixo para 'horizontal puro' (default: 0.95)"
    )
    parser.add_argument(
        "--min-pct", type=float, default=0.80,
        help="Porcentagem mínima de horizontais para remover camada (default: 0.80)"
    )
    parser.add_argument(
        "--post-remesh",
        action="store_true",
        help="Ativa remesh final (mais agressivo, pode perder detalhes; por defeito está desativado).",
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
        output_path = input_path.parent / f"{stem}_clean.glb"

    print(f"Carregando: {input_path}")
    try:
        mesh = trimesh.load(str(input_path), force="mesh")
    except Exception as e:
        print(f"Erro ao carregar mesh: {e}", file=sys.stderr)
        return 1

    print(f"  Vértices: {len(mesh.vertices):,}")
    print(f"  Faces: {len(mesh.faces):,}")

    print("\nDetectando orientação...")
    axis, direction = detect_base_axis(mesh)
    axis_names = ['X', 'Y', 'Z']
    dir_str = 'mínimo' if direction == -1 else 'máximo'
    print(f"  Base detetada no eixo {axis_names[axis]} ({dir_str})")

    print(f"\nRemovendo camadas puramente horizontais...")
    try:
        cleaned = remove_pedestal_layers(
            mesh,
            axis,
            direction,
            normal_threshold=args.threshold,
            min_horizontal_pct=args.min_pct,
        )
    except Exception as e:
        print(f"Erro durante remoção: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1

    print(f"\nResultado da remoção:")
    print(f"  Vértices: {len(cleaned.vertices):,} (antes: {len(mesh.vertices):,})")
    print(f"  Faces: {len(cleaned.faces):,} (antes: {len(mesh.faces):,})")

    # REPAIR COMPLETO: fechar buracos e limpar artefatos
    print(f"\nExecutando repair completo (fechamento de buracos)...")
    try:
        cleaned = repair_mesh_complete(cleaned, post_remesh=args.post_remesh)
        print(f"  Após repair: {len(cleaned.vertices):,} vértices, {len(cleaned.faces):,} faces")
    except Exception as e:
        print(f"  Aviso: erro no repair: {e}")

    print(f"\nSalvando: {output_path}")
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        cleaned.export(str(output_path), file_type="glb")
    except Exception as e:
        print(f"Erro ao salvar: {e}", file=sys.stderr)
        return 1

    print("Pronto!")
    return 0


if __name__ == "__main__":
    sys.exit(main())
