"""Pós-instalação Paint3D: submodule Hunyuan3D-2.1 e peso Real-ESRGAN."""

from __future__ import annotations

import importlib.util
import subprocess
import urllib.request
from pathlib import Path

from ..logging import Logger

_REALESRGAN_URL = (
    "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.1.0/RealESRGAN_x4plus.pth"
)


def _apply_hunyuan21_patches(monorepo_root: Path, logger: Logger) -> None:
    script = monorepo_root / "Paint3D" / "scripts" / "apply_hunyuan21_patches.py"
    if not script.is_file():
        return
    spec = importlib.util.spec_from_file_location("apply_hunyuan21_patches", script)
    if spec is None or spec.loader is None:
        return
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    changed = mod.apply_patches(monorepo_root)
    for rel in changed:
        logger.info(f"Patch hy3dpaint: {rel}")


def run_paint3d_post_install(monorepo_root: Path, logger: Logger) -> bool:
    """Inicializa ``third_party/Hunyuan3D-2.1`` e garante ``RealESRGAN_x4plus.pth``."""
    git_dir = monorepo_root / ".git"
    sub_path = monorepo_root / "third_party" / "Hunyuan3D-2.1"
    hy3d = sub_path / "hy3dpaint"

    if git_dir.exists():
        logger.step("Submodule Hunyuan3D-2.1 (hy3dpaint)...")
        try:
            subprocess.run(
                ["git", "submodule", "update", "--init", "third_party/Hunyuan3D-2.1"],
                cwd=str(monorepo_root),
                check=True,
            )
            logger.success("Submodule third_party/Hunyuan3D-2.1 pronto")
        except (subprocess.CalledProcessError, FileNotFoundError) as e:
            logger.warn(
                f"Não foi possível atualizar o submodule (git: {e}). "
                "Define HUNYUAN3D_21_ROOT para um clone de "
                "https://github.com/Tencent-Hunyuan/Hunyuan3D-2.1"
            )
    elif not hy3d.is_dir():
        logger.warn(
            "Sem .git na raiz: não foi possível clonar o submodule automaticamente. "
            "Coloca Hunyuan3D-2.1 em third_party/Hunyuan3D-2.1 ou define HUNYUAN3D_21_ROOT."
        )

    ckpt_dir = hy3d / "ckpt"
    ckpt = ckpt_dir / "RealESRGAN_x4plus.pth"
    if hy3d.is_dir():
        _apply_hunyuan21_patches(monorepo_root, logger)
        ckpt_dir.mkdir(parents=True, exist_ok=True)
        if not ckpt.is_file():
            logger.step(f"A descarregar Real-ESRGAN → {ckpt.name} ...")
            try:
                urllib.request.urlretrieve(_REALESRGAN_URL, ckpt)
                logger.success("RealESRGAN_x4plus.pth instalado")
            except OSError as e:
                logger.error(f"Falha ao descarregar Real-ESRGAN: {e}")
                logger.info(
                    "Descarrega manualmente:\n"
                    f"  {_REALESRGAN_URL}\n"
                    f"  → {ckpt}"
                )
                return False
    else:
        logger.warn(
            "hy3dpaint não encontrado; salta download do Real-ESRGAN até o código 2.1 estar disponível."
        )

    return True
