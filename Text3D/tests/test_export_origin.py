"""Testes para convenção de origem na exportação (feet / center / none)."""

from __future__ import annotations

import numpy as np
import trimesh

from text3d.utils.export import _apply_origin_trimesh


def _shifted_unit_cube() -> trimesh.Trimesh:
    """Cubo unitário centrado na origem, deslocado +1 em Y: Y ∈ [0.5, 1.5], X e Z ∈ [-0.5, 0.5]."""
    mesh = trimesh.creation.box(extents=[1.0, 1.0, 1.0])
    mesh.apply_translation([0.0, 1.0, 0.0])
    return mesh


class TestApplyOriginTrimesh:
    def test_feet_origin_sets_base_at_y_zero(self) -> None:
        mesh = _shifted_unit_cube()
        _apply_origin_trimesh(mesh, "feet")
        assert np.isclose(mesh.bounds[0][1], 0.0, atol=1e-6)

    def test_feet_origin_centers_xz(self) -> None:
        mesh = _shifted_unit_cube()
        _apply_origin_trimesh(mesh, "feet")
        b = mesh.bounds
        cx = (b[0][0] + b[1][0]) * 0.5
        cz = (b[0][2] + b[1][2]) * 0.5
        assert np.isclose(cx, 0.0, atol=1e-6)
        assert np.isclose(cz, 0.0, atol=1e-6)

    def test_center_origin_centers_all_axes(self) -> None:
        mesh = _shifted_unit_cube()
        _apply_origin_trimesh(mesh, "center")
        b = mesh.bounds
        center = (b[0] + b[1]) * 0.5
        np.testing.assert_allclose(center, [0.0, 0.0, 0.0], atol=1e-6)

    def test_none_origin_preserves_position(self) -> None:
        mesh = _shifted_unit_cube()
        bounds_before = mesh.bounds.copy()
        out = _apply_origin_trimesh(mesh, "none")
        assert out is mesh
        np.testing.assert_allclose(mesh.bounds, bounds_before, atol=1e-6)
