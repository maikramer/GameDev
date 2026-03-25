"""Memória, GPU e informações do sistema — delegate para gamedev_shared.gpu."""

from gamedev_shared.gpu import (
    format_bytes,
    get_gpu_info,
    get_system_info,
    check_gpu_compatibility,
)

__all__ = ["format_bytes", "get_gpu_info", "get_system_info", "check_gpu_compatibility"]
