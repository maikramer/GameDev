#!/usr/bin/env python3
"""
Aplica patches mínimos ao hy3dpaint (Hunyuan3D-2.1) necessários ao Paint3D.

Idempotente: pode correr várias vezes após ``git submodule update``.
"""

from __future__ import annotations

import sys
from pathlib import Path


def _hy3dpaint_root(monorepo: Path) -> Path:
    return monorepo / "third_party" / "Hunyuan3D-2.1" / "hy3dpaint"


def apply_patches(monorepo: Path) -> list[str]:
    """Devolve lista de mensagens (ficheiros alterados)."""
    hy = _hy3dpaint_root(monorepo)
    if not hy.is_dir():
        return []

    changed: list[str] = []

    mv = hy / "utils" / "multiview_utils.py"
    if mv.is_file():
        t = mv.read_text(encoding="utf-8")
        old = """        model_path = huggingface_hub.snapshot_download(
            repo_id=config.multiview_pretrained_path,
            allow_patterns=["hunyuan3d-paintpbr-v2-1/*"],
        )

        model_path = os.path.join(model_path, "hunyuan3d-paintpbr-v2-1")"""
        new = """        weights_subfolder = getattr(
            config, "multiview_weights_subfolder", "hunyuan3d-paintpbr-v2-1"
        )
        model_path = huggingface_hub.snapshot_download(
            repo_id=config.multiview_pretrained_path,
            allow_patterns=[f"{weights_subfolder}/*"],
        )

        model_path = os.path.join(model_path, weights_subfolder)"""
        if old in t and new not in t:
            mv.write_text(t.replace(old, new), encoding="utf-8")
            changed.append(str(mv.relative_to(monorepo)))

    tg = hy / "textureGenPipeline.py"
    if tg.is_file():
        t = tg.read_text(encoding="utf-8")
        rs = "(self.config.render_size, self.config.render_size)"
        old = f"""        for i in range(len(enhance_images)):
            enhance_images["albedo"][i] = enhance_images["albedo"][i].resize(
                {rs}
            )
            enhance_images["mr"][i] = enhance_images["mr"][i].resize({rs})"""
        new = f"""        for i in range(len(enhance_images["albedo"])):
            enhance_images["albedo"][i] = enhance_images["albedo"][i].resize(
                {rs}
            )
            enhance_images["mr"][i] = enhance_images["mr"][i].resize({rs})"""
        if old in t:
            tg.write_text(t.replace(old, new), encoding="utf-8")
            changed.append(str(tg.relative_to(monorepo)))

    return changed


def main() -> int:
    here = Path(__file__).resolve().parent
    monorepo = here.parent.parent
    if not (monorepo / "Shared").is_dir():
        print("Raiz do monorepo não encontrada (esperado Shared/).", file=sys.stderr)
        return 1
    out = apply_patches(monorepo)
    if out:
        print("Patches aplicados:")
        for line in out:
            print(f"  {line}")
    else:
        print("Nada a alterar (já aplicado ou hy3dpaint em falta).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
