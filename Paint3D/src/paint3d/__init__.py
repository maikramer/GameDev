"""
Paint3D — Texturização 3D: Hunyuan3D-Paint 2.1 + Upscale IA.

Pipeline standalone de textura para meshes 3D. Funciona independentemente
ou como dependência opcional do Text3D (``text3d generate --texture``).
"""

__version__ = "0.1.0"
__author__ = "Paint3D Project"

from .painter import apply_hunyuan_paint, check_paint_rasterizer_available, paint_file_to_file
from .texture_upscale import upscale_image, upscale_trimesh_texture
from .utils.mesh_io import load_mesh_trimesh, save_glb

__all__ = [
    "apply_hunyuan_paint",
    "check_paint_rasterizer_available",
    "load_mesh_trimesh",
    "paint_file_to_file",
    "save_glb",
    "upscale_image",
    "upscale_trimesh_texture",
]
