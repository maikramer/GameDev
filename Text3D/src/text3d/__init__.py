"""
Text3D — Text-to-3D com Text2D + Hunyuan3D-2mini (image-to-3D).

Descarrega o modelo 2D antes de carregar o Hunyuan3D. Padrões de inferência em
``text3d.defaults`` (perfil ~6GB VRAM CUDA, validado na prática); constantes ``HUNYUAN_HQ_*`` para GPU grande.
"""

__version__ = "0.1.0"
__author__ = "Text3D Project"

from . import defaults
from .generator import HunyuanTextTo3DGenerator
from .materialize_pbr import (
    apply_materialize_pbr,
    extract_base_color_and_uv,
    pack_metallic_roughness_gltf,
)
from .painter import apply_hunyuan_paint, load_mesh_trimesh, paint_file_to_file

__all__ = [
    "HunyuanTextTo3DGenerator",
    "apply_hunyuan_paint",
    "apply_materialize_pbr",
    "extract_base_color_and_uv",
    "defaults",
    "load_mesh_trimesh",
    "pack_metallic_roughness_gltf",
    "paint_file_to_file",
]
