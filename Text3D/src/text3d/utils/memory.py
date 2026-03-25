"""Memória, GPU e gestão de processos — delegate para gamedev_shared.gpu."""

from gamedev_shared.gpu import (
    format_bytes,
    get_gpu_info,
    get_system_info,
    check_gpu_compatibility,
    estimate_vram_requirement,
    clear_cuda_memory,
    gpu_bytes_in_use,
    enforce_exclusive_gpu,
    DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB,
    list_nvidia_compute_apps,
    kill_gpu_compute_processes_aggressive,
)

__all__ = [
    "format_bytes",
    "get_gpu_info",
    "get_system_info",
    "check_gpu_compatibility",
    "estimate_vram_requirement",
    "clear_cuda_memory",
    "gpu_bytes_in_use",
    "enforce_exclusive_gpu",
    "DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB",
    "list_nvidia_compute_apps",
    "kill_gpu_compute_processes_aggressive",
]
