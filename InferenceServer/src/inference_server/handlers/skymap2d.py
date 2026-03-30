from __future__ import annotations

from pathlib import Path

from ..schemas import Skymap2DParams


def run_skymap2d(_job_id: str, params: Skymap2DParams, out_dir: Path) -> list[str]:
    from skymap2d.generator import SkymapGenerator
    from skymap2d.image_processor import save_image

    out_dir.mkdir(parents=True, exist_ok=True)
    gen = SkymapGenerator(model_id=params.model_id)
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
    ext = ".exr" if params.image_format == "exr" else ".png"
    filename = f"{params.output_basename}{ext}"
    save_image(
        image,
        prompt=params.prompt,
        params=metadata,
        output_dir=out_dir,
        filename=filename,
        metadata=metadata,
        image_format=params.image_format,
        exr_scale=params.exr_scale,
    )
    json_name = Path(filename).with_suffix(".json").name
    return [filename, json_name]
