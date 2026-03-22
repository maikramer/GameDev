"""
Valores por defeito do Text3D.

**Perfil padrão (validado):** combinação estável em ~6 GB VRAM (CUDA) com boa qualidade
na prática (text-to-3D: robô, veículo, planta, etc.). O pico de VRAM costuma ser no
*volume decoding* do Hunyuan; estes valores evitam OOM nessa fase.

Para hardware mais capaz, usa as constantes *HQ* ou flags CLI maiores
(--octree-resolution, --num-chunks, --steps).
"""

# --- Text2D (imagem intermédia) ---
# 1024² puxa muita VRAM no FLUX; 768 é um compromisso estável em ~6GB.
DEFAULT_T2D_WIDTH = 768
DEFAULT_T2D_HEIGHT = 768

DEFAULT_T2D_STEPS = 4
DEFAULT_T2D_GUIDANCE = 1.0

# FLUX.2 Klein 4B não cabe em ~5–6GB com pipe.to(cuda); usar enable_model_cpu_offload.
# Desliga com t2d_full_gpu=True (CLI --t2d-full-gpu) em GPUs grandes.
DEFAULT_T2D_CPU_OFFLOAD = True

# --- Hunyuan3D-2mini (shape) — mesmo perfil que o CLI usa por defeito ---
DEFAULT_SUBFOLDER = "hunyuan3d-dit-v2-mini"

DEFAULT_HY_STEPS = 24
DEFAULT_HY_GUIDANCE = 5.0
DEFAULT_OCTREE_RESOLUTION = 128
DEFAULT_NUM_CHUNKS = 4096

# Pós-processo ao gravar mesh (CLI): 0 = só maior componente + merge; 1–2 suaviza superfície.
DEFAULT_MESH_SMOOTH = 0

# --- Hunyuan3D-Paint (textura multivista, hy3dgen.texgen) ---
# Pesos no repositório Hunyuan3D-2 (não confundir com Hunyuan3D-2mini só shape).
DEFAULT_PAINT_HF_REPO = "tencent/Hunyuan3D-2"
DEFAULT_PAINT_SUBFOLDER = "hunyuan3d-paint-v2-0-turbo"
# Delight + multiview diffusion: por defeito offload (VRAM semelhante a Text2D).
DEFAULT_PAINT_CPU_OFFLOAD = True

# --- Referência "alta qualidade" (model card HF / GPU com bastante VRAM) ---
# Ex.: --octree-resolution 380 --num-chunks 20000 --steps 30
HUNYUAN_HQ_OCTREE = 380
HUNYUAN_HQ_NUM_CHUNKS = 20000
HUNYUAN_HQ_STEPS = 30

# Marching cubes (Hunyuan): 0 = defeito do pipeline; valores pequenos podem alterar superfície.
DEFAULT_MC_LEVEL = 0.0

# Perfis CLI `--preset`: substituem steps + octree + num_chunks de uma vez.
# fast: menos VRAM/tempo; balanced: igual aos DEFAULT_*; hq: próximo do model card HF.
PRESET_HUNYUAN = {
    "fast": {"steps": 18, "octree": 96, "chunks": 3072},
    "balanced": {
        "steps": DEFAULT_HY_STEPS,
        "octree": DEFAULT_OCTREE_RESOLUTION,
        "chunks": DEFAULT_NUM_CHUNKS,
    },
    "hq": {"steps": HUNYUAN_HQ_STEPS, "octree": HUNYUAN_HQ_OCTREE, "chunks": HUNYUAN_HQ_NUM_CHUNKS},
}
