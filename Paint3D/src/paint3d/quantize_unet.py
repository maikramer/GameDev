"""CLI: pré-quantizar o UNet Hunyuan3D-Paint (qint8) para cache HF.

Uso::

    python -m paint3d.quantize_unet
    python -m paint3d.quantize_unet --force

O instalador universal chama isto após instalar o Paint3D (salvo ``--skip-models``).
"""

from __future__ import annotations

import argparse
import sys


def main() -> int:
    parser = argparse.ArgumentParser(description="Pré-quantizar UNet Hunyuan3D-Paint (qint8)")
    parser.add_argument(
        "--repo-id",
        default="tencent/Hunyuan3D-2.1",
        help="Repo HF do modelo Paint (defeito: tencent/Hunyuan3D-2.1)",
    )
    parser.add_argument(
        "--subfolder",
        default="hunyuan3d-paintpbr-v2-1",
        help="Subfolder no repo HF (defeito: hunyuan3d-paintpbr-v2-1)",
    )
    parser.add_argument("--force", action="store_true", help="Regenerar mesmo se artefactos já existem")
    args = parser.parse_args()

    from paint3d.utils.unet_quantization import quantize_and_save_unet

    ok = quantize_and_save_unet(repo_id=args.repo_id, subfolder=args.subfolder, force=args.force)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
