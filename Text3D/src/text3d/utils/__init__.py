"""
Utilitários para Text3D.
"""

from .memory import get_gpu_info, get_system_info, format_bytes, check_gpu_compatibility
from .export import save_mesh, save_gif, convert_mesh, get_mesh_info
from .mesh_repair import (
    repair_mesh,
    keep_largest_component,
    remove_ground_shadow_artifacts,
    remove_small_islands,
    fill_small_boundary_holes,
)

__all__ = [
    'get_gpu_info',
    'get_system_info',
    'format_bytes',
    'check_gpu_compatibility',
    'save_mesh',
    'save_gif',
    'convert_mesh',
    'get_mesh_info',
    'repair_mesh',
    'keep_largest_component',
    'remove_ground_shadow_artifacts',
    'remove_small_islands',
    'fill_small_boundary_holes',
]
