"""
Utilitários para gerenciamento de memória e GPU.
"""

import sys
from typing import Dict, List, Any, Optional

import torch


def get_gpu_info() -> List[Dict[str, Any]]:
    """
    Obtém informações sobre GPUs disponíveis.
    
    Returns:
        Lista de dicionários com informações de cada GPU
    """
    gpus = []
    
    if not torch.cuda.is_available():
        return gpus
    
    for i in range(torch.cuda.device_count()):
        props = torch.cuda.get_device_properties(i)
        
        # Obter memória
        try:
            torch.cuda.reset_peak_memory_stats(i)
            free_memory = torch.cuda.mem_get_info(i)[0] if hasattr(torch.cuda, 'mem_get_info') else 0
            total_memory = props.total_memory
        except:
            free_memory = 0
            total_memory = props.total_memory
        
        gpus.append({
            'id': i,
            'name': props.name,
            'total_memory': total_memory,
            'free_memory': free_memory,
            'compute_capability': f"{props.major}.{props.minor}",
            'multi_processor_count': props.multi_processor_count,
        })
    
    return gpus


def get_system_info() -> Dict[str, Any]:
    """
    Obtém informações completas do sistema.
    
    Returns:
        Dicionário com informações do sistema
    """
    info = {
        'python_version': f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}",
        'torch_version': torch.__version__,
        'cuda_available': torch.cuda.is_available(),
    }
    
    if torch.cuda.is_available():
        info['cuda_version'] = torch.version.cuda
        info['gpus'] = get_gpu_info()
    
    return info


def format_bytes(bytes_val: int) -> str:
    """
    Formata bytes para representação legível.
    
    Args:
        bytes_val: Valor em bytes
        
    Returns:
        String formatada (ex: "4.5 GB")
    """
    for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
        if bytes_val < 1024.0:
            return f"{bytes_val:.1f} {unit}"
        bytes_val /= 1024.0
    return f"{bytes_val:.1f} PB"


def estimate_vram_requirement(
    frame_size: int = 256,
    batch_size: int = 1,
    model_size_gb: float = 4.9
) -> float:
    """
    Estima VRAM necessária para geração.
    
    Args:
        frame_size: Tamanho do frame (128, 256, 512)
        batch_size: Tamanho do batch
        model_size_gb: Tamanho base do modelo em GB
        
    Returns:
        Estimativa de VRAM em GB
    """
    # Heurística para modelos HF (Text2D / Hunyuan)
    base_vram = model_size_gb
    
    # Overhead por frame_size
    # 256x256 é o padrão, 512 requer ~4x memória
    size_multiplier = (frame_size / 256) ** 2
    
    # Overhead por batch
    batch_multiplier = batch_size
    
    # Overhead de ativações (~20%)
    activation_overhead = 1.2
    
    estimated = base_vram * size_multiplier * batch_multiplier * activation_overhead
    
    return estimated


def check_gpu_compatibility(min_vram_gb: float = 6.0) -> tuple[bool, str]:
    """
    Verifica se a GPU é compatível.
    
    Args:
        min_vram_gb: VRAM mínima necessária em GB
        
    Returns:
        (compatível, mensagem)
    """
    if not torch.cuda.is_available():
        return False, "CUDA não disponível. Usando CPU (mais lento)."
    
    gpus = get_gpu_info()
    
    for gpu in gpus:
        vram_gb = gpu['total_memory'] / (1024**3)
        if vram_gb >= min_vram_gb:
            return True, f"GPU {gpu['name']} com {vram_gb:.1f}GB compatível."
    
    # Verificar maior GPU
    if gpus:
        max_vram = max(g['total_memory'] for g in gpus) / (1024**3)
        return False, f"VRAM insuficiente. Máximo: {max_vram:.1f}GB, Necessário: {min_vram_gb}GB. Use --low-vram."
    
    return False, "Nenhuma GPU detectada."
