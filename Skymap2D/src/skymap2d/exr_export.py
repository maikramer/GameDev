"""Exportação EXR (RGB linear float32) a partir de imagens geradas (LDR 8-bit).

O Materialize (Rust) também grava EXR para mapas PBR; aqui usamos o mesmo *tipo*
de ficheiro (OpenEXR, precisão float), mas **sem** o pipeline de height/normal —
apenas o panorama equirectangular em RGB scene-linear.

O conteúdo radiométrico continua limitado ao que o modelo devolve (PNG 8-bit);
o EXR serve para motores que esperam ficheiros lineares (.exr) e evita dupla
codificação sRGB em cadeia.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import numpy as np

from PIL import Image


def pil_rgb_to_linear_f32(image: Image.Image) -> np.ndarray:
    """Converte PIL RGB 8-bit sRGB para array float32 RGB linear [0, 1] (por canal)."""
    import numpy as np

    rgb = np.asarray(image.convert("RGB"), dtype=np.float32) / 255.0
    mask = rgb <= 0.04045
    out = np.empty_like(rgb, dtype=np.float32)
    out[mask] = rgb[mask] / 12.92
    out[~mask] = np.power((rgb[~mask] + 0.055) / 1.055, 2.4, dtype=np.float32)
    return np.clip(out, 0.0, 1.0)


def write_exr_rgb_linear(path: Path, rgb_linear: np.ndarray, *, scale: float = 1.0) -> None:
    """Grava um EXR scanline com canal intercalado ``RGB`` (float32, linear)."""
    import numpy as np
    import OpenEXR

    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)

    arr = np.ascontiguousarray(rgb_linear, dtype=np.float32)
    if scale != 1.0:
        arr = arr * float(scale)
    arr = np.maximum(arr, 0.0)

    channels = {"RGB": arr}
    header = {
        "compression": OpenEXR.ZIP_COMPRESSION,
        "type": OpenEXR.scanlineimage,
    }
    with OpenEXR.File(header, channels) as outfile:
        outfile.write(str(path))
