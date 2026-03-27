"""Texture2D — gerador de texturas seamless via HF Inference API."""

from __future__ import annotations

import io
import logging
import os
import re
from typing import Any, Dict, Generator, List, Optional

from PIL import Image

from gamedev_shared.hf import get_hf_token

from .presets import get_preset_params, get_preset_prompt
from .utils import generate_seed, validate_params, validate_prompt

logger = logging.getLogger(__name__)

DEFAULT_MODEL_ID = "gokaygokay/Flux-Seamless-Texture-LoRA"

BASE_TEXTURE_INSTRUCTIONS = (
    "seamless, tileable, repeatable, repeating pattern, perfectly looping texture, "
    "no visible seams, no borders, no frame, no text, no watermark"
)

DEFAULT_PARAMS: Dict[str, Any] = {
    "guidance_scale": 7.5,
    "num_inference_steps": 50,
    "seed": None,
    "width": 1024,
    "height": 1024,
    "cfg_scale": 7.5,
    "negative_prompt": "",
    "lora_strength": 1.0,
}


def default_model_id() -> str:
    return os.environ.get("TEXTURE2D_MODEL_ID", DEFAULT_MODEL_ID)


def augment_prompt_for_seamless(prompt: str) -> str:
    """Acrescenta instruções de textura seamless/tileable automaticamente.

    Se o utilizador já menciona seamless/tileable/repeatable, não duplica.
    """
    p = (prompt or "").strip()
    if not p:
        return p
    if re.search(
        r"\b(seamless|tileable|tiling|repeatable|repeating|repeat)\b",
        p,
        flags=re.IGNORECASE,
    ):
        return p
    return f"{BASE_TEXTURE_INSTRUCTIONS}, {p}"


def merge_negative_prompt(preset_neg: str, user_neg: str) -> str:
    """Combina negative prompt do preset com o do utilizador."""
    preset_neg = (preset_neg or "").strip()
    user_neg = (user_neg or "").strip()
    if not preset_neg:
        return user_neg
    if not user_neg:
        return preset_neg
    if preset_neg.lower() in user_neg.lower():
        return user_neg
    if user_neg.lower() in preset_neg.lower():
        return preset_neg
    return f"{preset_neg}, {user_neg}"


class TextureGenerator:
    """Gerador de texturas seamless via HF Inference API."""

    def __init__(self, model_id: Optional[str] = None) -> None:
        self.model_id = self._normalize_model_id(model_id or default_model_id())
        self.client = self._init_client()

    @staticmethod
    def _normalize_model_id(model_id: str) -> str:
        if model_id.startswith("models/"):
            return model_id[len("models/"):]
        return model_id

    def _init_client(self):  # noqa: ANN202
        from huggingface_hub import InferenceClient

        token = get_hf_token()
        client = InferenceClient(token=token)
        logger.info(f"InferenceClient inicializado — modelo: {self.model_id}")
        return client

    def generate(
        self,
        prompt: str,
        negative_prompt: str = "",
        guidance_scale: float = 7.5,
        num_inference_steps: int = 50,
        seed: Optional[int] = None,
        width: int = 1024,
        height: int = 1024,
        cfg_scale: Optional[float] = None,
        lora_strength: float = 1.0,
        preset: Optional[str] = None,
    ) -> tuple[Image.Image, Dict[str, Any]]:
        """Gera uma textura seamless.

        Returns:
            Tuple (imagem PIL, metadata dict).
        """
        if not self.client:
            raise RuntimeError("Inference client não inicializado")

        # Merge preset
        if preset and preset != "None":
            preset_prompt = get_preset_prompt(preset)
            preset_params = get_preset_params(preset)
            if preset_prompt:
                prompt = f"{preset_prompt}, {prompt}" if prompt else preset_prompt
            if preset_params:
                guidance_scale = float(preset_params.get("guidance_scale", guidance_scale))
                num_inference_steps = int(
                    preset_params.get("num_inference_steps", num_inference_steps)
                )
                width = int(preset_params.get("width", width))
                height = int(preset_params.get("height", height))
                if "negative_prompt" in preset_params:
                    negative_prompt = merge_negative_prompt(
                        str(preset_params.get("negative_prompt") or ""),
                        negative_prompt,
                    )

        # Augment seamless
        prompt = augment_prompt_for_seamless(prompt)

        is_valid, error = validate_prompt(prompt, max_length=1200)
        if not is_valid:
            prompt = prompt[:1200]

        if cfg_scale is None:
            cfg_scale = guidance_scale

        if seed is None or seed < 0:
            seed = generate_seed()

        params = {
            "prompt": prompt,
            "negative_prompt": negative_prompt,
            "guidance_scale": guidance_scale,
            "num_inference_steps": num_inference_steps,
            "seed": seed,
            "width": width,
            "height": height,
            "cfg_scale": cfg_scale,
            "lora_strength": lora_strength,
        }

        is_valid, error = validate_params(params)
        if not is_valid:
            raise ValueError(f"Parâmetros inválidos: {error}")

        hf_params: Dict[str, Any] = {
            "negative_prompt": negative_prompt,
            "guidance_scale": float(guidance_scale),
            "num_inference_steps": int(num_inference_steps),
            "width": int(width),
            "height": int(height),
            "seed": int(seed),
            "cfg_scale": float(cfg_scale),
            "lora_scale": float(lora_strength),
        }

        image: Optional[Image.Image] = None

        try:
            image = self.client.text_to_image(
                prompt=prompt,
                model=self.model_id,
                **hf_params,
            )
        except TypeError:
            image = self.client.text_to_image(
                prompt=prompt,
                model=self.model_id,
            )
        except Exception as e:
            logger.warning(f"text_to_image falhou ({e}); fallback para POST raw.")
            payload = {"inputs": prompt, "parameters": hf_params}
            raw = self.client.post(model=self.model_id, json=payload)
            image = Image.open(io.BytesIO(raw)).convert("RGB")

        if image is None:
            raise RuntimeError("Nenhuma imagem devolvida pela HF Inference API")

        image = image.convert("RGB")

        metadata = {
            "seed": seed,
            "prompt_final": prompt,
            **params,
        }

        return image, metadata

    def generate_batch(
        self,
        prompts: List[str],
        base_params: Optional[Dict[str, Any]] = None,
    ) -> Generator[tuple[Optional[Image.Image], Dict[str, Any], int], None, None]:
        """Gera múltiplas texturas em batch.

        Yields:
            Tuple (imagem | None, metadata, index).
        """
        if base_params is None:
            base_params = {}

        total = len(prompts)
        logger.info(f"Batch: {total} imagens")

        for idx, prompt in enumerate(prompts):
            try:
                merged = {**DEFAULT_PARAMS, **base_params}
                merged.pop("seed", None)
                merged.pop("prompt", None)

                image, metadata = self.generate(prompt=prompt, **merged)
                yield image, metadata, idx
            except Exception as e:
                logger.error(f"Erro na imagem {idx + 1}/{total}: {e}")
                yield None, {"error": str(e), "index": idx}, idx
