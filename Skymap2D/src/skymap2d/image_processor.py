"""Utilitários de processamento de imagem para Skymap2D."""

import json
import logging
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

from PIL import Image

logger = logging.getLogger(__name__)

DEFAULT_OUTPUT_DIR = Path("outputs") / "skymaps"


def save_image(
    image: Image.Image,
    prompt: str,
    params: Dict[str, Any],
    output_dir: Optional[Path] = None,
    filename: Optional[str] = None,
    metadata: Optional[Dict[str, Any]] = None,
    *,
    image_format: str = "png",
    exr_scale: float = 1.0,
) -> Path:
    """Grava uma imagem com metadata JSON ao lado.

    ``image_format``: ``png`` (8-bit sRGB) ou ``exr`` (RGB float32 linear, mesmo
    conteúdo que o PNG após descodificação sRGB — ver ``exr_export``).

    Returns:
        Path do ficheiro gravado (.png ou .exr).
    """
    out_dir = output_dir or DEFAULT_OUTPUT_DIR
    out_dir.mkdir(parents=True, exist_ok=True)

    fmt = (image_format or "png").lower()
    if fmt not in ("png", "exr"):
        raise ValueError(f"image_format deve ser png ou exr, recebido: {image_format}")

    if filename is None:
        ts = int(datetime.now().timestamp())
        ext = ".exr" if fmt == "exr" else ".png"
        filename = f"skymap_{ts}{ext}"

    filepath = out_dir / filename
    if fmt == "exr":
        from .exr_export import pil_rgb_to_linear_f32, write_exr_rgb_linear

        linear = pil_rgb_to_linear_f32(image)
        write_exr_rgb_linear(filepath, linear, scale=exr_scale)
        logger.info(f"EXR gravado em {filepath}")
    else:
        image.save(filepath, "PNG")
        logger.info(f"Imagem gravada em {filepath}")

    metadata_path = filepath.with_suffix(".json")
    metadata_dict: Dict[str, Any] = {
        "timestamp": datetime.now().timestamp(),
        "prompt": prompt,
        "params": params,
        "image_path": str(filepath),
        "filename": filename,
        "image_format": fmt,
        "color_space": "linear_rgb" if fmt == "exr" else "srgb_png",
    }
    if metadata:
        metadata_dict.update(metadata)

    with open(metadata_path, "w", encoding="utf-8") as f:
        json.dump(metadata_dict, f, indent=2, ensure_ascii=False)

    return filepath


def create_thumbnail(
    image: Image.Image, size: tuple[int, int] = (512, 256)
) -> Image.Image:
    """Cria um thumbnail da imagem (2:1 por defeito para skymaps)."""
    thumb = image.copy()
    thumb.thumbnail(size, Image.Resampling.LANCZOS)
    return thumb


def create_zip(files: List[Path], output_path: Path) -> Path:
    """Cria um arquivo ZIP a partir de uma lista de ficheiros."""
    with zipfile.ZipFile(output_path, "w", zipfile.ZIP_DEFLATED) as zipf:
        for file in files:
            if file.exists():
                zipf.write(file, file.name)
    logger.info(f"ZIP criado em {output_path}")
    return output_path


def load_metadata(image_path: Path) -> Optional[Dict[str, Any]]:
    """Carrega metadata JSON de uma imagem."""
    metadata_path = image_path.with_suffix(".json")
    if not metadata_path.exists():
        return None
    try:
        with open(metadata_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception as e:
        logger.error(f"Erro a carregar metadata: {e}")
        return None
