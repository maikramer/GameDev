"""
Pré-quantização e carregamento do UNet Hunyuan3D-Paint com optimum-quanto (qint8 weight-only).

Artefactos ficam junto ao snapshot HuggingFace::

    <model_dir>/unet/unet-qint8.safetensors
    <model_dir>/unet/unet-qint8-quantization_map.json

Reduz o footprint do UNet na VRAM em ~50% vs FP16 (~2.5 GB vs ~5 GB),
permitindo inferir em GPUs de 6 GB (com DINO e RealESRGAN em CPU).
"""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

import torch

UNET_QUANT_SAFETENSORS = "unet-qint8.safetensors"
UNET_QUANT_MAP_JSON = "unet-qint8-quantization_map.json"


def unet_quantized_paths(model_dir: str | Path) -> tuple[Path, Path]:
    base = Path(model_dir) / "unet"
    return base / UNET_QUANT_SAFETENSORS, base / UNET_QUANT_MAP_JSON


def quantized_unet_artifacts_exist(model_dir: str | Path) -> bool:
    st, jm = unet_quantized_paths(model_dir)
    return st.is_file() and st.stat().st_size > 0 and jm.is_file() and jm.stat().st_size > 0


def want_quantized_unet(device: str, model_dir: str | Path) -> bool:
    """Decide se deve usar UNet quantizado (auto + env ``PAINT3D_USE_QUANTIZED_UNET``)."""
    env = os.environ.get("PAINT3D_USE_QUANTIZED_UNET", "").strip().lower()
    if env in ("0", "false", "no", "off"):
        return False
    if not quantized_unet_artifacts_exist(model_dir):
        return False
    if env in ("1", "true", "yes", "on"):
        return True
    if device != "cuda" or not torch.cuda.is_available():
        return False
    try:
        vram_gb = float(torch.cuda.get_device_properties(0).total_memory) / (1024**3)
    except Exception:
        return False
    return vram_gb < 10.0


def load_unet_quantized(pipeline, model_dir: str | Path) -> bool:
    """Requantiza o UNet já instanciado no pipeline com os artefactos qint8."""
    st_path, map_path = unet_quantized_paths(model_dir)
    if not st_path.is_file() or not map_path.is_file():
        return False

    from optimum.quanto.quantize import requantize
    from safetensors.torch import load_file

    with open(map_path, encoding="utf-8") as f:
        qmap: dict[str, Any] = json.load(f)

    state_dict = load_file(str(st_path), device="cpu")
    requantize(pipeline.unet, state_dict, qmap, device=torch.device("cpu"))
    pipeline.unet.eval()

    # Quantizar text_encoder on-the-fly (qint8 weight-only); poupa ~200 MB VRAM.
    if hasattr(pipeline, "text_encoder") and pipeline.text_encoder is not None:
        try:
            from optimum.quanto.quantize import freeze, quantize
            from optimum.quanto.tensor.qtype import qint8

            quantize(pipeline.text_encoder, weights=qint8, activations=None)
            freeze(pipeline.text_encoder)
        except Exception:
            pass

    return True


def quantize_and_save_unet(
    *,
    repo_id: str = "tencent/Hunyuan3D-2.1",
    subfolder: str = "hunyuan3d-paintpbr-v2-1",
    force: bool = False,
    log: Any | None = None,
) -> bool:
    """
    Descarrega o modelo HF, carrega pipeline em CPU, quantiza UNet (qint8) e grava artefactos.
    """
    import gc

    import huggingface_hub
    from safetensors.torch import save_file

    def _log(msg: str) -> None:
        if log is not None:
            log(msg)
        else:
            print(f"[quantize_unet] {msg}", flush=True)

    snapshot = huggingface_hub.snapshot_download(
        repo_id=repo_id,
        allow_patterns=[f"{subfolder}/*"],
    )
    model_dir = os.path.join(snapshot, subfolder)

    st_out, map_out = unet_quantized_paths(model_dir)
    if (st_out.is_file() or map_out.is_file()) and not quantized_unet_artifacts_exist(model_dir):
        _log("Artefactos incompletos — a remover antes de regenerar.")
        st_out.unlink(missing_ok=True)
        map_out.unlink(missing_ok=True)
    if quantized_unet_artifacts_exist(model_dir) and not force:
        _log(f"Artefactos já existem em {st_out.parent}; usa --force para regenerar.")
        return True

    st_out.parent.mkdir(parents=True, exist_ok=True)

    custom_pipeline = os.path.join(
        os.path.dirname(os.path.abspath(__file__)),
        "..",
        "hy3dpaint",
        "hunyuanpaintpbr",
    )

    _log("A carregar pipeline em CPU (fp16)...")
    from diffusers import DiffusionPipeline

    pipeline = DiffusionPipeline.from_pretrained(
        model_dir,
        custom_pipeline=custom_pipeline,
        torch_dtype=torch.float16,
    )
    unet = pipeline.unet
    unet.eval()

    _log("A quantizar UNet (qint8 weight-only)...")
    from optimum.quanto.quantize import freeze, quantization_map, quantize
    from optimum.quanto.tensor.qtype import qint8

    quantize(unet, weights=qint8, activations=None)
    freeze(unet)
    qmap = quantization_map(unet)

    _log(f"A gravar {st_out.name} ({len(qmap)} módulos quantizados)...")
    try:
        save_file(unet.state_dict(), str(st_out))
        with open(map_out, "w", encoding="utf-8") as f:
            json.dump(qmap, f, indent=2)
    except Exception as e:
        _log(f"Erro ao gravar artefactos quantizados: {e}")
        for p in (st_out, map_out):
            if p.is_file():
                p.unlink(missing_ok=True)
        return False

    del pipeline, unet
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.empty_cache()
    _log("Concluído.")
    return quantized_unet_artifacts_exist(model_dir)
