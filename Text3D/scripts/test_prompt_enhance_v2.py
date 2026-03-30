#!/usr/bin/env python3
"""Teste visual: gera imagens 2D com o novo prompt enhancement v2.

Gera 4 imagens com prompts variados (personagem, prop, criatura, veículo)
e grava numa pasta timestamped. Usa seeds fixas para reprodutibilidade.

Uso:
    python scripts/test_prompt_enhance_v2.py [--steps 8] [--guidance 3.5]
"""

from __future__ import annotations

import argparse
import time
from pathlib import Path

from text3d.utils.prompt_enhance import create_optimized_prompt

TEST_PROMPTS: list[dict] = [
    {"label": "dragon", "prompt": "a cute baby dragon with small wings", "seed": 42},
    {"label": "robot", "prompt": "a futuristic robot warrior with a laser sword", "seed": 123},
    {"label": "sword", "prompt": "medieval sword with glowing runes on the blade", "seed": 777},
    {"label": "tree", "prompt": "a stylized fantasy tree with purple leaves", "seed": 2024},
]


def main() -> None:
    parser = argparse.ArgumentParser(description="Teste visual prompt enhance v2")
    parser.add_argument("--steps", type=int, default=4, help="Inference steps (4=fast, 8=quality)")
    parser.add_argument("--guidance", type=float, default=3.5, help="Guidance scale")
    parser.add_argument("--width", type=int, default=768)
    parser.add_argument("--height", type=int, default=768)
    parser.add_argument("--no-enhance", action="store_true", help="Raw prompts (sem enhancement)")
    args = parser.parse_args()

    from text2d.generator import KleinFluxGenerator

    ts = int(time.time())
    mode = "raw" if args.no_enhance else "v2"
    out_dir = Path(f"outputs/prompt_test_{mode}_{ts}")
    out_dir.mkdir(parents=True, exist_ok=True)

    print(f"=== Prompt Enhancement Test ({mode}) ===")
    print(f"Steps: {args.steps}, Guidance: {args.guidance}, Size: {args.width}x{args.height}")
    print(f"Output: {out_dir.resolve()}\n")

    gen = KleinFluxGenerator(low_vram=True, verbose=True)
    gen.warmup()

    for test in TEST_PROMPTS:
        label = test["label"]
        raw_prompt = test["prompt"]
        seed = test["seed"]

        final_prompt = raw_prompt if args.no_enhance else create_optimized_prompt(raw_prompt)

        print(f"\n--- {label} (seed={seed}) ---")
        print(f"Raw:      {raw_prompt}")
        print(f"Enhanced: {final_prompt[:120]}...")

        t0 = time.time()
        img = gen.generate(
            prompt=final_prompt,
            height=args.height,
            width=args.width,
            guidance_scale=args.guidance,
            num_inference_steps=args.steps,
            seed=seed,
        )
        elapsed = time.time() - t0

        out_path = out_dir / f"{label}_s{seed}.png"
        img.save(str(out_path), format="PNG")
        print(f"  -> {out_path} ({elapsed:.1f}s)")

    # Salvar resumo dos prompts usados
    summary = out_dir / "prompts.txt"
    with open(summary, "w") as f:
        f.write(f"mode={mode} steps={args.steps} guidance={args.guidance} size={args.width}x{args.height}\n\n")
        for test in TEST_PROMPTS:
            raw = test["prompt"]
            enhanced = create_optimized_prompt(raw) if not args.no_enhance else raw
            f.write(f"[{test['label']}] seed={test['seed']}\n")
            f.write(f"  raw: {raw}\n")
            f.write(f"  final: {enhanced}\n\n")

    gen.unload()
    print(f"\n=== Concluído: {out_dir.resolve()} ===")


if __name__ == "__main__":
    main()
