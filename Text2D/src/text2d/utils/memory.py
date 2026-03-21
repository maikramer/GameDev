"""Memória, GPU e informações do sistema (paridade com Text3D)."""

import sys
from typing import Any, Dict, List

import torch


def get_gpu_info() -> List[Dict[str, Any]]:
    """Lista GPUs com VRAM e nome."""
    gpus: List[Dict[str, Any]] = []
    if not torch.cuda.is_available():
        return gpus

    for i in range(torch.cuda.device_count()):
        props = torch.cuda.get_device_properties(i)
        try:
            free_memory = torch.cuda.mem_get_info(i)[0] if hasattr(torch.cuda, "mem_get_info") else 0
            total_memory = props.total_memory
        except Exception:
            free_memory = 0
            total_memory = props.total_memory

        gpus.append(
            {
                "id": i,
                "name": props.name,
                "total_memory": total_memory,
                "free_memory": free_memory,
                "compute_capability": f"{props.major}.{props.minor}",
            }
        )

    return gpus


def get_system_info() -> Dict[str, Any]:
    """Python, PyTorch, CUDA e GPUs."""
    info: Dict[str, Any] = {
        "python_version": f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}",
        "torch_version": torch.__version__,
        "cuda_available": torch.cuda.is_available(),
    }
    if torch.cuda.is_available():
        info["cuda_version"] = torch.version.cuda
        info["gpus"] = get_gpu_info()
    return info


def format_bytes(bytes_val: int) -> str:
    """Formata bytes (ex.: 4.5 GB)."""
    for unit in ["B", "KB", "MB", "GB", "TB"]:
        if bytes_val < 1024.0:
            return f"{bytes_val:.1f} {unit}"
        bytes_val /= 1024.0
    return f"{bytes_val:.1f} PB"


def check_gpu_compatibility(min_vram_gb: float = 8.0) -> tuple[bool, str]:
    """Verifica VRAM mínima para FLUX Klein (orientativo)."""
    if not torch.cuda.is_available():
        return False, "CUDA não disponível. Usando CPU (muito mais lento)."

    gpus = get_gpu_info()
    for gpu in gpus:
        vram_gb = gpu["total_memory"] / (1024**3)
        if vram_gb >= min_vram_gb:
            return True, f"GPU {gpu['name']} com {vram_gb:.1f} GB (compatível)."

    if gpus:
        max_vram = max(g["total_memory"] for g in gpus) / (1024**3)
        return (
            False,
            f"VRAM pode ser insuficiente (máx. {max_vram:.1f} GB). Use --low-vram ou resolução menor.",
        )

    return False, "Nenhuma GPU detectada."
