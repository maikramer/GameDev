"""
Valores por defeito do Paint3D.

Constantes de configuração para Hunyuan3D-Paint 2.1 e upscale IA.
"""

from __future__ import annotations

# --- Hunyuan3D-Paint 2.1 (hy3dpaint / hunyuan3d-paintpbr-v2-1 em tencent/Hunyuan3D-2.1) ---
DEFAULT_PAINT_HF_REPO = "tencent/Hunyuan3D-2.1"
DEFAULT_PAINT_SUBFOLDER = "hunyuan3d-paintpbr-v2-1"
DEFAULT_PAINT_CPU_OFFLOAD = True
DEFAULT_PAINT_MAX_VIEWS = 4
DEFAULT_PAINT_VIEW_RESOLUTION = 512

# --- Upscaling IA (Real-ESRGAN via spandrel) ---
DEFAULT_UPSCALE = False
DEFAULT_UPSCALE_FACTOR = 4
