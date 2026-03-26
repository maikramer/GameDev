"""Text2Sound — processamento e exportação de áudio.

Normalização de pico, conversão de formatos e remoção de silêncio trailing.
Usa soundfile (libsndfile) para escrita — portável, sem dependência de CUDA.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Optional

import numpy as np
import soundfile as sf
import torch

SUPPORTED_FORMATS = ("wav", "flac", "ogg")
DEFAULT_FORMAT = "wav"

_SF_SUBTYPES = {
    "wav": "PCM_16",
    "flac": "PCM_16",
    "ogg": "VORBIS",
}


def peak_normalize(audio: torch.Tensor) -> torch.Tensor:
    """Normaliza áudio pelo valor de pico para a gama [-1, 1]."""
    peak = torch.max(torch.abs(audio))
    if peak > 0:
        audio = audio / peak
    return audio.clamp(-1, 1)


def to_int16(audio: torch.Tensor) -> torch.Tensor:
    """Converte tensor float [-1, 1] para int16."""
    return (
        audio.to(torch.float32)
        .clamp(-1, 1)
        .mul(32767)
        .to(torch.int16)
    )


def trim_silence(
    audio: torch.Tensor,
    sample_rate: int,
    threshold_db: float = -60.0,
    min_silence_ms: int = 200,
) -> torch.Tensor:
    """Remove silêncio no final do áudio.

    Procura de trás para frente pelo último sample acima do limiar
    e corta o áudio mantendo um buffer mínimo.

    Args:
        audio: Tensor (channels, samples) float.
        sample_rate: Taxa de amostragem.
        threshold_db: Limiar em dB abaixo do qual se considera silêncio.
        min_silence_ms: Buffer mínimo de silêncio a manter (ms).
    """
    threshold_linear = 10 ** (threshold_db / 20.0)
    mono = audio.abs().max(dim=0).values

    above_threshold = torch.nonzero(mono > threshold_linear, as_tuple=True)[0]
    if len(above_threshold) == 0:
        return audio

    last_sound = above_threshold[-1].item()
    buffer_samples = int(sample_rate * min_silence_ms / 1000)
    end_idx = min(last_sound + buffer_samples, audio.shape[-1])

    return audio[:, :end_idx]


def save_audio(
    audio: torch.Tensor,
    sample_rate: int,
    output_path: Path,
    fmt: str = DEFAULT_FORMAT,
    as_int16: bool = True,
    normalize: bool = True,
    trim: bool = False,
    metadata: Optional[dict[str, Any]] = None,
) -> Path:
    """Processa e grava áudio num ficheiro.

    Args:
        audio: Tensor (channels, samples).
        sample_rate: Taxa de amostragem.
        output_path: Caminho de saída (extensão será ajustada ao formato).
        fmt: Formato de saída (wav, flac, ogg).
        as_int16: Converter para int16 antes de gravar (WAV).
        normalize: Aplicar normalização de pico.
        trim: Remover silêncio trailing.
        metadata: Metadados para gravar num .json ao lado do áudio.

    Returns:
        Caminho do ficheiro de áudio gravado.
    """
    fmt = fmt.lower()
    if fmt not in SUPPORTED_FORMATS:
        raise ValueError(
            f"Formato '{fmt}' não suportado. "
            f"Opções: {', '.join(SUPPORTED_FORMATS)}"
        )

    audio = audio.cpu().to(torch.float32)

    if normalize:
        audio = peak_normalize(audio)

    if trim:
        audio = trim_silence(audio, sample_rate)

    output_path = output_path.with_suffix(f".{fmt}")
    output_path.parent.mkdir(parents=True, exist_ok=True)

    # (channels, samples) → (samples, channels) for soundfile
    audio_np: np.ndarray = audio.numpy().T
    subtype = _SF_SUBTYPES.get(fmt, "PCM_16")
    sf.write(str(output_path), audio_np, sample_rate, subtype=subtype)

    if metadata:
        meta_path = output_path.with_suffix(output_path.suffix + ".json")
        meta_path.write_text(
            json.dumps(metadata, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

    return output_path
