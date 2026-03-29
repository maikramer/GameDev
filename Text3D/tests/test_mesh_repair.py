"""Testes do pós-processamento de mesh (sombra na base)."""

import numpy as np
import trimesh

from text3d.utils.mesh_repair import (
    _from_export_y_up,
    _remove_connected_ground_plinth,
    _to_export_y_up,
    remove_ground_shadow_artifacts,
    remove_small_islands,
)


def test_y_up_roundtrip_matches_export_rotation() -> None:
    box = trimesh.creation.box(extents=[0.4, 1.8, 0.4])
    box.apply_translation([0, 0.9, 0])
    hunyuan_like = _from_export_y_up(box)
    back = _to_export_y_up(hunyuan_like)
    assert np.allclose(back.bounds, box.bounds, atol=1e-4)


def test_remove_ground_shadow_drops_separate_thin_disc() -> None:
    body = trimesh.creation.box(extents=[0.5, 2.0, 0.5])
    body.apply_translation([0, 1.0, 0])
    disc = trimesh.creation.cylinder(radius=0.55, height=0.04, sections=48)
    disc.apply_translation([0, 0.02, 0])
    scene_yup = trimesh.util.concatenate([body, disc])
    mesh_hunyuan = _from_export_y_up(scene_yup)

    out = remove_ground_shadow_artifacts(mesh_hunyuan)
    out_yup = _to_export_y_up(out)
    parts = out_yup.split(only_watertight=False)
    assert len(parts) == 1
    assert len(out.faces) < len(mesh_hunyuan.faces)


def test_remove_small_islands_drops_tiny_fragment() -> None:
    big = trimesh.creation.icosphere(subdivisions=2, radius=1.0)
    tiny = trimesh.creation.box(extents=[0.02, 0.02, 0.02])
    tiny.apply_translation([4.0, 0.0, 0.0])
    combined = trimesh.util.concatenate([big, tiny])
    n_parts_before = len(combined.split(only_watertight=False))
    out = remove_small_islands(
        combined,
        min_face_ratio=0.02,
        min_faces_abs=64,
    )
    n_parts_after = len(out.split(only_watertight=False))
    assert n_parts_after < n_parts_before
    assert len(out.faces) < len(combined.faces)


def test_remove_connected_ground_plinth_detects_plinth() -> None:
    """Testa detecção de pedestal conectado ao mesh principal."""
    # Corpo principal (cilindro estreito)
    body = trimesh.creation.cylinder(radius=0.3, height=1.5, sections=32)
    body.apply_translation([0, 0.75, 0])

    # Pedestal na base (disco mais largo, conectado ao corpo)
    plinth = trimesh.creation.cylinder(radius=0.8, height=0.08, sections=48)
    plinth.apply_translation([0, 0.04, 0])

    # Concatenar — o pedestal está conectado ao corpo
    combined = trimesh.util.concatenate([body, plinth])

    # Aplicar remoção de pedestal com threshold baixo para capturar este caso
    out = _remove_connected_ground_plinth(combined, min_expansion=1.05)

    # Deve ter removido algumas faces (do pedestal)
    assert len(out.faces) < len(combined.faces)
    # Deve manter o corpo principal
    assert len(out.faces) > 100


def test_remove_connected_ground_plinth_no_false_positive() -> None:
    """Testa que não remove geometria legítima quando não há pedestal."""
    # Cilindro uniforme — não há pedestal
    body = trimesh.creation.cylinder(radius=0.5, height=1.5, sections=32)
    body.apply_translation([0, 0.75, 0])

    n_faces_before = len(body.faces)
    out = _remove_connected_ground_plinth(body)

    # Não deve remover muita geometria de um cilindro uniforme
    assert len(out.faces) >= n_faces_before * 0.95


def test_very_aggressive_removes_connected_plinth() -> None:
    """Testa o modo very_aggressive que combina múltiplas heurísticas."""
    # Corpo principal
    body = trimesh.creation.cylinder(radius=0.3, height=1.5, sections=32)
    body.apply_translation([0, 0.75, 0])

    # Pedestal largo conectado
    plinth = trimesh.creation.cylinder(radius=1.0, height=0.1, sections=64)
    plinth.apply_translation([0, 0.05, 0])

    combined = trimesh.util.concatenate([body, plinth])
    mesh_hunyuan = _from_export_y_up(combined)

    # Modo normal (não aggressive) — pode não remover tudo
    out_normal = remove_ground_shadow_artifacts(mesh_hunyuan, aggressive=False)

    # Modo very_aggressive — deve ser mais efetivo
    out_very = remove_ground_shadow_artifacts(mesh_hunyuan, very_aggressive=True)

    # O modo very_aggressive deve remover mais faces
    assert len(out_very.faces) <= len(out_normal.faces)
    # Mas deve manter o corpo principal
    assert len(out_very.faces) > 100
