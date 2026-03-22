"""Testes para materialize_pbr (packing glTF e extração de textura)."""

import numpy as np
import pytest
import trimesh
from PIL import Image

from text3d.materialize_pbr import (
    extract_base_color_and_uv,
    pack_metallic_roughness_gltf,
)


def test_pack_metallic_roughness_gltf_channels():
    # 1x1: metallic branco (1), smoothness branco (1) → roughness 0
    met = Image.new("L", (1, 1), color=255)
    sm = Image.new("L", (1, 1), color=255)
    mr = pack_metallic_roughness_gltf(met, sm, roughness_from_one_minus_smoothness=True)
    assert mr.mode == "RGBA"
    px = mr.getpixel((0, 0))
    assert px[0] == 255  # R
    assert px[1] == 0  # G roughness
    assert px[2] == 255  # B metallic
    assert px[3] == 255


def test_pack_metallic_roughness_no_invert():
    met = Image.new("L", (1, 1), color=0)
    sm = Image.new("L", (1, 1), color=128)
    mr = pack_metallic_roughness_gltf(met, sm, roughness_from_one_minus_smoothness=False)
    px = mr.getpixel((0, 0))
    assert px[1] == 128  # G ≈ smoothness as roughness


def test_extract_base_color_and_uv_simple():
    img = Image.new("RGB", (2, 2), color=(10, 20, 30))
    box = trimesh.creation.box()
    uv = np.random.rand(len(box.vertices), 2).astype(np.float64)
    mat = trimesh.visual.material.SimpleMaterial(image=img)
    box.visual = trimesh.visual.TextureVisuals(uv=uv, material=mat)
    uvo, out = extract_base_color_and_uv(box)
    assert uvo.shape == uv.shape
    assert out.size == img.size
    assert out.getpixel((0, 0)) == (10, 20, 30)


def test_extract_requires_texture_visual():
    mesh = trimesh.creation.box()
    with pytest.raises(ValueError, match="visual"):
        extract_base_color_and_uv(mesh)
