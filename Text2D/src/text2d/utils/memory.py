"""Memória, GPU e informações do sistema — delegate para gamedev_shared.gpu."""

from gamedev_shared.gpu import (
    check_gpu_compatibility,
    format_bytes,
    get_gpu_info,
    get_system_info,
)

__all__ = ["check_gpu_compatibility", "format_bytes", "get_gpu_info", "get_system_info"]
