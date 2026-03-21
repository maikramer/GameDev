"""
Variáveis de ambiente para desempenho e estabilidade (CUDA).
"""

import os


def ensure_pytorch_cuda_alloc_conf() -> None:
    """
    Define ``PYTORCH_CUDA_ALLOC_CONF`` com ``expandable_segments:True`` se ainda não
    estiver definido — reduz fragmentação de VRAM em GPUs ~6GB (Hunyuan decode, Paint).
    """
    if os.environ.get("PYTORCH_CUDA_ALLOC_CONF"):
        return
    os.environ["PYTORCH_CUDA_ALLOC_CONF"] = "expandable_segments:True"
