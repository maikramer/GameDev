"""
Valores por defeito do Paint3D.

Constantes de configuração para Hunyuan3D-Paint, Materialize PBR e upscale IA.
"""

from __future__ import annotations

# --- Hunyuan3D-Paint (textura multivista, hy3dgen.texgen) ---
DEFAULT_PAINT_HF_REPO = "tencent/Hunyuan3D-2"
DEFAULT_PAINT_SUBFOLDER = "hunyuan3d-paint-v2-0-turbo"
DEFAULT_PAINT_CPU_OFFLOAD = True

# --- Upscaling IA (Real-ESRGAN via spandrel) ---
DEFAULT_UPSCALE = False
DEFAULT_UPSCALE_FACTOR = 4

# --- Materialize PBR presets ---
MATERIALIZE_PRESETS = ("default", "skin", "floor", "metal", "fabric", "wood", "stone")
