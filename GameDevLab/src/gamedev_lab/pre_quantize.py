"""Pré-quantização SDNQ (Part3D DiT / Paint3D UNet)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

from rich.console import Console
from rich.panel import Panel

from gamedev_lab.paths import gamedev_repo_root

console = Console()


def _ensure_part3d_path() -> None:
    root = gamedev_repo_root()
    p = root / "Part3D" / "src"
    sp = str(p)
    if sp not in sys.path:
        sys.path.insert(0, sp)


def quantizar_part3d_dit() -> bool:
    """Pré-quantiza o DiT do Part3D (Hunyuan3D-Part) para SDNQ uint8."""
    _ensure_part3d_path()
    console.print(
        Panel.fit(
            "[bold blue]Part3D DiT Quantization[/bold blue]\nModelo: tencent/Hunyuan3D-Part\nDestino: uint8 (SDNQ)",
            title="Pré-quantização",
        )
    )

    try:
        import torch
        from easydict import EasyDict
        from huggingface_hub import snapshot_download
        from safetensors.torch import load_file

        console.print("[yellow]1. Download do modelo...[/yellow]")
        model_dir = snapshot_download(
            repo_id="tencent/Hunyuan3D-Part",
            repo_type="model",
            allow_patterns=["model/*", "model/config.json"],
        )
        console.print(f"   [green]✓[/green] Modelo em: {model_dir}")

        console.print("[yellow]2. Carregando DiT em FP16...[/yellow]")
        cfg_path = Path(model_dir) / "model" / "config.json"
        with open(cfg_path, encoding="utf-8") as f:
            model_cfg = json.load(f)

        sys.path.insert(0, str(Path(model_dir)))
        from partgen.utils.misc import instantiate_from_config

        dit = instantiate_from_config(EasyDict(model_cfg))
        ckpt_path = Path(model_dir) / "model" / "model.safetensors"
        console.print(f"   Carregando pesos de {ckpt_path.name}...")
        ckpt = load_file(str(ckpt_path), device="cpu")
        dit.load_state_dict(ckpt)
        del ckpt

        dit.to(dtype=torch.float16)
        dit.eval()

        total_params = sum(p.numel() for p in dit.parameters())
        console.print(f"   [green]✓[/green] DiT carregado: {total_params / 1e6:.0f}M params")

        console.print("[yellow]3. Aplicando SDNQ uint8...[/yellow]")
        from gamedev_shared.sdnq import quantize_model

        dit_quantizado = quantize_model(dit, preset="sdnq-uint8")
        dit_quantizado.eval()
        console.print("   [green]✓[/green] Quantização aplicada")

        console.print("[yellow]4. Salvando modelo quantizado...[/yellow]")
        output_dir = Path(model_dir) / "model_sdnq_uint8"
        output_dir.mkdir(exist_ok=True)

        from safetensors.torch import save_file

        state_dict = dit_quantizado.state_dict()
        save_file(state_dict, str(output_dir / "model.safetensors"))
        with open(output_dir / "config.json", "w") as f:
            json.dump(model_cfg, f, indent=2)

        meta = {
            "quantization": "sdnq-uint8",
            "group_size": 0,
            "use_svd": False,
            "original_model": "tencent/Hunyuan3D-Part",
            "parameters": total_params,
        }
        with open(output_dir / "quantization_meta.json", "w") as f:
            json.dump(meta, f, indent=2)

        console.print(f"   [green]✓[/green] Salvo em: {output_dir}")
        del dit, dit_quantizado, state_dict
        if torch.cuda.is_available():
            torch.cuda.empty_cache()

        console.print(
            Panel.fit(
                f"[bold green]✓ Part3D DiT quantizado com sucesso![/bold green]\nLocal: {output_dir}",
                title="Concluído",
            )
        )
        return True

    except Exception as e:
        console.print(f"[bold red]✗ Erro:[/bold red] {e}")
        import traceback

        console.print(traceback.format_exc())
        return False


def quantizar_paint3d_unet() -> bool:
    """Pré-quantiza o UNet do Paint3D para SDNQ uint8."""
    console.print(
        Panel.fit(
            "[bold blue]Paint3D UNet Quantization[/bold blue]\n"
            "Modelo: tencent/Hunyuan3D-2.1/paint\n"
            "Destino: uint8 (SDNQ)",
            title="Pré-quantização",
        )
    )

    try:
        import torch
        from diffusers import DiffusionPipeline
        from huggingface_hub import snapshot_download

        root = gamedev_repo_root()
        custom_pipeline = str(root / "Paint3D" / "src" / "paint3d" / "hy3dpaint")

        console.print("[yellow]1. Download e carregamento do pipeline...[/yellow]")
        snapshot = snapshot_download(
            repo_id="tencent/Hunyuan3D-2.1",
            allow_patterns=["hunyuan3d-paintpbr-v2-1/*"],
        )
        model_dir = Path(snapshot) / "hunyuan3d-paintpbr-v2-1"

        pipe = DiffusionPipeline.from_pretrained(
            str(model_dir),
            custom_pipeline=custom_pipeline,
            torch_dtype=torch.float16,
        )
        unet = pipe.unet
        unet.eval()

        total_params = sum(p.numel() for p in unet.parameters())
        console.print(f"   [green]✓[/green] UNet carregado: {total_params / 1e6:.0f}M params")

        console.print("[yellow]2. Aplicando SDNQ uint8 ao UNet...[/yellow]")
        from gamedev_shared.sdnq import quantize_model

        unet_quantizado = quantize_model(unet, preset="sdnq-uint8")
        unet_quantizado.eval()
        console.print("   [green]✓[/green] Quantização aplicada")

        console.print("[yellow]3. Salvando UNet quantizado...[/yellow]")
        output_dir = model_dir / "unet_sdnq_uint8"
        output_dir.mkdir(exist_ok=True)

        from safetensors.torch import save_file

        state_dict = unet_quantizado.state_dict()
        save_file(state_dict, str(output_dir / "diffusion_pytorch_model.safetensors"))
        config = {"_class_name": "UNet2DConditionModel", "quantization": "sdnq-uint8"}
        with open(output_dir / "config.json", "w") as f:
            json.dump(config, f, indent=2)

        console.print(f"   [green]✓[/green] Salvo em: {output_dir}")
        del pipe, unet, unet_quantizado, state_dict
        torch.cuda.empty_cache()

        console.print(
            Panel.fit(
                "[bold green]✓ Paint3D UNet quantizado com sucesso![/bold green]",
                title="Concluído",
            )
        )
        return True

    except Exception as e:
        console.print(f"[bold red]✗ Erro:[/bold red] {e}")
        import traceback

        console.print(traceback.format_exc())
        return False


def run_pre_quantize_cli(modelo: str, dry_run: bool) -> int:
    from gamedev_shared.sdnq import is_available as sdnq_available

    if not sdnq_available():
        console.print("[red]✗ SDNQ não instalado. Execute: pip install sdnq[/red]")
        return 1

    console.print("[green]✓ SDNQ disponível[/green]")

    if dry_run:
        console.print("[dim]Dry-run: apenas verificando...[/dim]")
        return 0

    resultados: dict[str, bool] = {}
    if modelo in ("part3d", "todos"):
        resultados["part3d"] = quantizar_part3d_dit()
    if modelo in ("paint3d", "todos"):
        resultados["paint3d"] = quantizar_paint3d_unet()

    console.print("\n" + "=" * 60)
    console.print("[bold]RESUMO[/bold]")
    console.print("=" * 60)
    for m, sucesso in resultados.items():
        status = "[green]✓ SUCESSO[/green]" if sucesso else "[red]✗ FALHA[/red]"
        console.print(f"{m}: {status}")

    return 0 if resultados and all(resultados.values()) else 1
