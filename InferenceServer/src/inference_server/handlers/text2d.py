from __future__ import annotations

from pathlib import Path

from ..schemas import Text2DParams


def run_text2d(_job_id: str, params: Text2DParams, out_dir: Path) -> list[str]:
    from text2d.generator import KleinFluxGenerator

    out_dir.mkdir(parents=True, exist_ok=True)
    device = "cpu" if params.cpu else None
    gen = KleinFluxGenerator(
        device=device,
        low_vram=params.low_vram,
        verbose=False,
        model_id=params.model_id,
    )
    try:
        gen.warmup()
        image = gen.generate(
            prompt=params.prompt,
            height=params.height,
            width=params.width,
            guidance_scale=params.guidance_scale,
            num_inference_steps=params.steps,
            seed=params.seed,
        )
    finally:
        gen.unload()

    ext = "png"
    name = f"{params.output_basename}.{ext}"
    path = out_dir / name
    KleinFluxGenerator.save_image(image, path, image_format="PNG")
    return [name]
