"""Testes do pós-processamento de mesh (sombra na base)."""

import numpy as np
import trimesh

from text3d.utils.mesh_repair import (
    _from_export_y_up,
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
