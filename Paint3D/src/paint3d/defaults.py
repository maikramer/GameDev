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

# --- Otimizações de VRAM ---
DEFAULT_QUANTIZATION_MODE = "auto"  # "auto", "none", "fp8", "int8", "int4", "quanto-int8", "quanto-int4"
DEFAULT_USE_TINY_VAE = False  # TAESD para menor uso de VRAM no VAE
DEFAULT_ENABLE_VAE_SLICING = True  # Slicing para batch processing
DEFAULT_ENABLE_VAE_TILING = True  # Tiling para imagens grandes
DEFAULT_VAE_TILE_SIZE = 256  # Tamanho mínimo do tile para VAE
DEFAULT_TORCH_COMPILE = False  # Compilar UNet com torch.compile
DEFAULT_TORCH_COMPILE_MODE = "reduce-overhead"  # "default", "reduce-overhead", "max-autotune"
DEFAULT_ENABLE_ATTENTION_SLICING = True  # Attention slicing para reduzir pico de VRAM

# --- BF16 vs FP16 ---
DEFAULT_DTYPE = "float16"  # "float16", "bfloat16", "float32"
# BF16 é melhor em RTX 40 series (Ada Lovelace) - mais eficiente e estável que FP16

# --- xFormers ---
DEFAULT_USE_XFORMERS = True  # Usar xformers memory efficient attention quando disponível
DEFAULT_XFORMERS_ATTENTION_OP = None  # None para default, ou "FlashAttention", "MemoryEfficientAttention"

# --- FP8 (RTX 40 series) ---
DEFAULT_FP8_INFERENCE = False  # Habilitar FP8 para GPUs com compute capability >= 8.9

# --- RTX 4050 6GB Perfil Especial ---
DEFAULT_RTX4050_MODE = False  # Ativa todas as otimizações para RTX 4050 6GB
# Quando ativado: BF16, int4, xformers, tiny VAE, tile size 128, no compile

# --- Upscaling IA (Real-ESRGAN via spandrel) ---
DEFAULT_UPSCALE = False
DEFAULT_UPSCALE_FACTOR = 4
