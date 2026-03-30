"""
Paint3D — Texturização 3D: Hunyuan3D-Paint + Materialize PBR + Upscale IA.

Pipeline standalone de textura para meshes 3D. Funciona independentemente
ou como dependência opcional do Text3D (``text3d generate --texture``).
"""

__version__ = "0.1.0"
__author__ = "Paint3D Project"

from .materialize_pbr import (
    apply_materialize_pbr,
    extract_base_color_and_uv,
    pack_metallic_roughness_gltf,
)
from .painter import apply_hunyuan_paint, check_paint_rasterizer_available, paint_file_to_file
from .texture_upscale import upscale_image, upscale_trimesh_texture
from .utils.mesh_io import load_mesh_trimesh, save_glb

__all__ = [
    "apply_hunyuan_paint",
    "apply_materialize_pbr",
    "check_paint_rasterizer_available",
    "extract_base_color_and_uv",
    "load_mesh_trimesh",
    "pack_metallic_roughness_gltf",
    "paint_file_to_file",
    "save_glb",
    "upscale_image",
    "upscale_trimesh_texture",
]
