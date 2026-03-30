"""
Resolve o código ``hy3dpaint`` vendored em ``paint3d.hy3dpaint``.

O código vem de https://github.com/Tencent-Hunyuan/Hunyuan3D-2.1 (pasta ``hy3dpaint/``),
integrado directamente no pacote — sem submodule.

Os **modelos** (pesos) são descarregados sob demanda via ``huggingface_hub.snapshot_download``
de ``tencent/Hunyuan3D-2.1`` (pasta ``hunyuan3d-paintpbr-v2-1``).
"""

from __future__ import annotations

import sys
from pathlib import Path

_REALESRGAN_URL = (
    "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.1.0/RealESRGAN_x4plus.pth"
)


def resolve_hy3dpaint_root() -> Path:
    """Return the vendored ``hy3dpaint`` directory inside this package."""
    return Path(__file__).resolve().parent / "hy3dpaint"


def ensure_hy3dpaint_on_path() -> Path:
    """Insert ``hy3dpaint`` at the front of ``sys.path`` and return its path."""
    root = resolve_hy3dpaint_root()
    s = str(root)
    if s not in sys.path:
        sys.path.insert(0, s)
    return root


def default_realesrgan_ckpt() -> Path:
    """Default path for the Real-ESRGAN checkpoint (inside HF cache or local ckpt dir)."""
    return resolve_hy3dpaint_root() / "ckpt" / "RealESRGAN_x4plus.pth"


def ensure_realesrgan_ckpt() -> Path:
    """Download Real-ESRGAN checkpoint if missing and return its path."""
    ckpt = default_realesrgan_ckpt()
    if ckpt.is_file():
        return ckpt
    ckpt.parent.mkdir(parents=True, exist_ok=True)
    import urllib.request
    print(f"[Paint3D] A descarregar RealESRGAN_x4plus.pth → {ckpt}")
    urllib.request.urlretrieve(_REALESRGAN_URL, ckpt)
    return ckpt


def default_cfg_yaml() -> Path:
    """Path to the vendored hunyuan-paint-pbr config YAML."""
    return resolve_hy3dpaint_root() / "cfgs" / "hunyuan-paint-pbr.yaml"
