"""
Utilitários para gerenciamento de memória e GPU.
"""

import gc
import os
import signal
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

import torch


def clear_cuda_memory() -> None:
    """
    Força recolha de lixo e esvazia o cache CUDA.

    Usado entre fases pesadas (Hunyuan shape → Paint → Materialize) em GPUs ~6 GB
    para reduzir OOM por fragmentação ou picos residuais.
    """
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.empty_cache()


# VRAM já ocupada por *qualquer* processo acima disto → recusar inferência (GPU partilhada).
DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB = 300


def gpu_bytes_in_use(device: int = 0) -> int | None:
    """
    Bytes de VRAM em uso na GPU (total − livre), incluindo todos os processos.

    Devolve ``None`` se ``mem_get_info`` não existir (PyTorch antigo) — o chamador
    pode ignorar a verificação de exclusividade.
    """
    if not torch.cuda.is_available():
        return 0
    if not hasattr(torch.cuda, "mem_get_info"):
        return None
    free, total = torch.cuda.mem_get_info(device)
    return int(total - free)


def enforce_exclusive_gpu(
    *,
    device: int = 0,
    max_used_mib: int = DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB,
    allow_shared: bool = False,
) -> None:
    """
    Garante que a GPU está quase livre antes de carregar modelos grandes.

    Se ``allow_shared`` for falso e a VRAM já ocupada exceder ``max_used_mib``,
    levanta ``RuntimeError`` (o CLI converte em mensagem ao utilizador).
    """
    if allow_shared:
        return
    used = gpu_bytes_in_use(device)
    if used is None:
        return
    max_bytes = max_used_mib * 1024 * 1024
    if used > max_bytes:
        mib = used / (1024 * 1024)
        raise RuntimeError(
            f"GPU com ~{mib:.0f} MiB já em uso (limite para arrancar: {max_used_mib} MiB). "
            "Fecha outras aplicações que usem a GPU ou usa --gpu-kill-others (defeito) / "
            "TEXT3D_GPU_KILL_OTHERS=1 para terminar processos listados pelo nvidia-smi; "
            "ou --allow-shared-gpu / TEXT3D_ALLOW_SHARED_GPU=1 para ignorar esta verificação."
        )


# Processos que não devem ser mortos (risco de derrubar sessão gráfica).
_GPU_KILL_PROTECTED_NAMES = frozenset(
    {
        "xorg",
        "x",
        "gnome-shell",
        "plasmashell",
        "kwin",
        "kwin_x11",
        "kwin_wayland",
        "sddm",
        "gdm",
        "gdm-wayland",
        "dbus-daemon",
        "pipewire",
        "wireplumber",
        "nvidia-egl",
        "nvidia-persistenced",
        "nvidia-gridd",
        "nvidia-modeset",
        "gsd-xsettings",
        "mutter",
        "cinnamon",
        "xfwm4",
        "budgie-wm",
        "muffin",
    }
)


def _gpu_kill_basename(proc_name: str) -> str:
    s = proc_name.strip()
    if not s:
        return ""
    return Path(s.split()[0]).name.lower()


def _is_protected_gpu_process(proc_name: str) -> bool:
    b = _gpu_kill_basename(proc_name)
    if b in _GPU_KILL_PROTECTED_NAMES:
        return True
    low = proc_name.lower()
    if "xwayland" in low:
        return True
    return False


def list_nvidia_compute_apps() -> list[tuple[int, str, Optional[int]]]:
    """
    Lista processos reportados como compute apps (nvidia-smi).

    Cada entrada: ``(pid, process_name, used_mib_ou_None)``.
    """
    if not shutil.which("nvidia-smi"):
        return []
    r = subprocess.run(
        [
            "nvidia-smi",
            "--query-compute-apps=pid,process_name,used_gpu_memory",
            "--format=csv,noheader,nounits",
        ],
        capture_output=True,
        text=True,
        timeout=20,
    )
    if r.returncode != 0 or not (r.stdout or "").strip():
        return []
    out: list[tuple[int, str, Optional[int]]] = []
    for line in r.stdout.strip().splitlines():
        parts = [p.strip() for p in line.split(",")]
        if len(parts) < 2:
            continue
        try:
            pid = int(parts[0])
        except ValueError:
            continue
        name = parts[1]
        mib: Optional[int] = None
        if len(parts) >= 3 and parts[2] and parts[2].upper() not in ("N/A", "[N/A]"):
            try:
                mib = int(float(parts[2].replace(" MiB", "").strip()))
            except ValueError:
                pass
        out.append((pid, name, mib))
    return out


def kill_gpu_compute_processes_aggressive(
    *,
    exclude_pid: int,
    term_wait_seconds: float = 2.0,
) -> list[str]:
    """
    Envia SIGTERM a processos GPU listados pelo driver; depois SIGKILL se ainda vivos.

    Exclui ``exclude_pid`` (o processo atual) e nomes protegidos (display / sessão).
    Devolve linhas de log legíveis.
    """
    logs: list[str] = []
    apps = list_nvidia_compute_apps()
    targets: list[tuple[int, str]] = []
    for pid, name, mib in apps:
        if pid == exclude_pid:
            continue
        if _is_protected_gpu_process(name):
            logs.append(f"[ignorado] PID {pid} ({name}) — protegido (display/sessão)")
            continue
        extra = f" ~{mib} MiB" if mib is not None else ""
        targets.append((pid, name))
        logs.append(f"[alvo] PID {pid} ({name}){extra}")

    if not targets:
        if not apps:
            logs.append("nvidia-smi não listou compute apps (driver ou GPU idle).")
        else:
            logs.append("Sem alvos para terminar (só processo atual ou entradas protegidas).")
        return logs

    for pid, name in targets:
        try:
            os.kill(pid, signal.SIGTERM)
            logs.append(f"SIGTERM → PID {pid} ({name})")
        except ProcessLookupError:
            logs.append(f"PID {pid} já terminou")
        except PermissionError:
            logs.append(f"PID {pid} ({name}): sem permissão (SIGTERM)")
        except OSError as e:
            logs.append(f"PID {pid}: {e}")

    time.sleep(term_wait_seconds)

    for pid, name in targets:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            continue
        except OSError:
            continue
        try:
            os.kill(pid, signal.SIGKILL)
            logs.append(f"SIGKILL → PID {pid} ({name})")
        except ProcessLookupError:
            pass
        except PermissionError:
            logs.append(f"PID {pid} ({name}): sem permissão (SIGKILL)")

    return logs


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
