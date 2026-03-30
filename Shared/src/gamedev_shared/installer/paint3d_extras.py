"""Pós-instalação Paint3D: peso Real-ESRGAN (o código hy3dpaint é vendored)."""

from __future__ import annotations

import urllib.request
from pathlib import Path

from ..logging import Logger

_REALESRGAN_URL = "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.1.0/RealESRGAN_x4plus.pth"


def run_paint3d_post_install(monorepo_root: Path, logger: Logger) -> bool:
    """Garante ``RealESRGAN_x4plus.pth`` no directório de checkpoints vendored."""
    ckpt_dir = monorepo_root / "Paint3D" / "src" / "paint3d" / "hy3dpaint" / "ckpt"
    ckpt = ckpt_dir / "RealESRGAN_x4plus.pth"

    ckpt_dir.mkdir(parents=True, exist_ok=True)
    if not ckpt.is_file():
        logger.step(f"A descarregar Real-ESRGAN → {ckpt.name} ...")
        try:
            urllib.request.urlretrieve(_REALESRGAN_URL, ckpt)
            logger.success("RealESRGAN_x4plus.pth instalado")
        except OSError as e:
            logger.error(f"Falha ao descarregar Real-ESRGAN: {e}")
            logger.info(f"Descarrega manualmente:\n  {_REALESRGAN_URL}\n  → {ckpt}")
            return False
    else:
        logger.success("RealESRGAN_x4plus.pth já existe")

    return True
