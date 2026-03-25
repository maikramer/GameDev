"""Utilitários de processamento de imagem para Texture2D."""

import json
import logging
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

from PIL import Image

logger = logging.getLogger(__name__)

DEFAULT_OUTPUT_DIR = Path("outputs") / "textures"


def save_image(
    image: Image.Image,
    prompt: str,
    params: Dict[str, Any],
    output_dir: Optional[Path] = None,
    filename: Optional[str] = None,
    metadata: Optional[Dict[str, Any]] = None,
) -> Path:
    """Grava uma imagem com metadata JSON ao lado.

    Returns:
        Path do ficheiro PNG gravado.
    """
    out_dir = output_dir or DEFAULT_OUTPUT_DIR
    out_dir.mkdir(parents=True, exist_ok=True)

    if filename is None:
        ts = int(datetime.now().timestamp())
        filename = f"texture_{ts}.png"

    filepath = out_dir / filename
    image.save(filepath, "PNG")
    logger.info(f"Imagem gravada em {filepath}")

    metadata_path = filepath.with_suffix(".json")
    metadata_dict: Dict[str, Any] = {
        "timestamp": datetime.now().timestamp(),
        "prompt": prompt,
        "params": params,
        "image_path": str(filepath),
        "filename": filename,
    }
    if metadata:
        metadata_dict.update(metadata)

    with open(metadata_path, "w", encoding="utf-8") as f:
        json.dump(metadata_dict, f, indent=2, ensure_ascii=False)

    return filepath


def create_thumbnail(
    image: Image.Image, size: tuple[int, int] = (256, 256)
) -> Image.Image:
    """Cria um thumbnail da imagem."""
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
