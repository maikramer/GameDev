"""
Valores por defeito do Part3D.

**Perfil padrão:** FP16 + CPU offloading sequencial em ~6 GB VRAM (CUDA).
Cada componente do pipeline (P3-SAM, Conditioner, DiT, ShapeVAE) é carregado
na GPU apenas quando necessário e descarregado após uso.

Tamanhos dos pesos (FP32 → FP16):
  model (DiT):     6.63 GB → ~3.3 GB
  conditioner:     1.76 GB → ~880 MB
  shapevae:        656 MB  → ~328 MB
  p3sam:           451 MB  → ~225 MB
  Total FP16:      ~4.75 GB (pico ~5.2 GB durante denoising)

Otimizações disponíveis:
  - Quantização 4-bit/8-bit via bitsandbytes ou torchao
  - Attention slicing para reduzir pico de VRAM
  - torch.compile para acelerar inferência
  - Group offloading (mais eficiente que sequential)
"""

from __future__ import annotations

# --- Modelo HuggingFace ---
DEFAULT_HF_REPO = "tencent/Hunyuan3D-Part"

# --- Inferência X-Part (DiT denoising) ---
DEFAULT_NUM_INFERENCE_STEPS = 50
DEFAULT_GUIDANCE_SCALE = -1.0

# --- Marching cubes (decode latents → mesh) ---
# 256 é conservador para ~6GB; 384 para GPUs maiores; 512 para A100/H100.
DEFAULT_OCTREE_RESOLUTION = 256
DEFAULT_NUM_CHUNKS = 20000
DEFAULT_MC_LEVEL = -1 / 512
DEFAULT_MC_ALGO = "mc"

# --- P3-SAM (segmentação) ---
DEFAULT_POSTPROCESS = True
DEFAULT_POSTPROCESS_THRESHOLD = 0.95

# --- VRAM ---
# FP16 é o modo padrão; o __call__ do PartFormer já usa autocast bfloat16.
DEFAULT_DTYPE = "float16"
# CPU offloading: mover cada componente para GPU apenas quando necessário.
DEFAULT_CPU_OFFLOAD = True

# --- Otimizações de quantização ---
DEFAULT_QUANTIZATION_MODE = "auto"  # "auto", "none", "int8", "int4", "torchao-int4", "torchao-int8"
DEFAULT_QUANTIZE_DIT = True  # Quantizar o DiT (modelo mais pesado)
DEFAULT_QUANTIZE_VAE = False  # Quantizar o ShapeVAE

# --- Otimizações de attention e compile ---
DEFAULT_ENABLE_ATTENTION_SLICING = True  # Reduz pico de VRAM
DEFAULT_TORCH_COMPILE = False  # Compilar DiT para speedup
DEFAULT_TORCH_COMPILE_MODE = "reduce-overhead"  # "default", "reduce-overhead", "max-autotune"

# --- xFormers para attention eficiente ---
DEFAULT_USE_XFORMERS = True

# --- Group offloading (mais eficiente que sequential) ---
DEFAULT_USE_GROUP_OFFLOAD = True  # Usar group offloading quando disponível

# --- Dtype (BF16 melhor para RTX 40 series) ---
DEFAULT_DTYPE = "float16"  # "float16", "bfloat16", "float32"

# --- RTX 4050 6GB Modo Especial ---
DEFAULT_RTX4050_MODE = False
