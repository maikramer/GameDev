from __future__ import annotations

from pathlib import Path

from ..schemas import Texture2DParams


def run_texture2d(_job_id: str, params: Texture2DParams, out_dir: Path) -> list[str]:
    from texture2d.generator import TextureGenerator

    out_dir.mkdir(parents=True, exist_ok=True)
    gen = TextureGenerator(model_id=params.model_id)
    image, metadata = gen.generate(
        prompt=params.prompt,
        negative_prompt=params.negative_prompt,
        guidance_scale=params.guidance_scale,
        num_inference_steps=params.steps,
        seed=params.seed,
        width=params.width,
        height=params.height,
        cfg_scale=params.cfg_scale,
        lora_strength=params.lora_strength,
        preset=params.preset,
    )
    name = f"{params.output_basename}.png"
    path = out_dir / name
    image.save(path, "PNG")
    meta_path = out_dir / f"{params.output_basename}.json"
    import json

    meta_path.write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")
    return [name, meta_path.name]
