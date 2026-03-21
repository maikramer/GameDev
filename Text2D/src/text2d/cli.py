#!/usr/bin/env python3
"""
Text2D — CLI principal (text-to-2D).
"""

import os
import sys
import time
from pathlib import Path
from typing import Optional

import click
from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.table import Table

from .generator import KleinFluxGenerator, default_model_id
from .utils.memory import format_bytes, get_system_info

console = Console()

DEFAULT_OUTPUT_DIR = Path("outputs")
DEFAULT_IMAGE_DIR = DEFAULT_OUTPUT_DIR / "images"


def ensure_dirs() -> None:
    DEFAULT_IMAGE_DIR.mkdir(parents=True, exist_ok=True)


@click.group()
@click.version_option(version="0.1.0", prog_name="text2d")
@click.option("--verbose", "-v", is_flag=True, help="Logs detalhados")
@click.pass_context
def cli(ctx: click.Context, verbose: bool) -> None:
    """Text2D — imagens a partir de texto (FLUX.2 Klein 4B SDNQ)."""
    ctx.ensure_object(dict)
    ctx.obj["VERBOSE"] = verbose
    ensure_dirs()


@cli.command("generate")
@click.argument("prompt")
@click.option("--output", "-o", type=click.Path(), help="Ficheiro de saída (.png ou .jpg)")
@click.option("--width", "-W", default=1024, show_default=True, type=int)
@click.option("--height", "-H", default=1024, show_default=True, type=int)
@click.option("--steps", "-s", default=4, show_default=True, help="Passos de inferência")
@click.option(
    "--guidance",
    "-g",
    "guidance_scale",
    default=1.0,
    show_default=True,
    help="Guidance (recomendado 1.0 para checkpoint SDNQ)",
)
@click.option("--seed", type=int, default=None, help="Seed para reprodutibilidade")
@click.option("--cpu", is_flag=True, help="Forçar CPU")
@click.option("--low-vram", is_flag=True, help="CPU offload (menos VRAM)")
@click.option(
    "--model",
    "-m",
    "model_id",
    default=None,
    help="ID Hugging Face (default: Disty0 SDNQ ou TEXT2D_MODEL_ID)",
)
@click.option(
    "--verbose",
    "-v",
    "verbose_flag",
    is_flag=True,
    help="Logs detalhados (ou use: text2d -v generate ...)",
)
@click.pass_context
def generate_cmd(
    ctx: click.Context,
    prompt: str,
    output: Optional[str],
    width: int,
    height: int,
    steps: int,
    guidance_scale: float,
    seed: Optional[int],
    cpu: bool,
    low_vram: bool,
    model_id: Optional[str],
    verbose_flag: bool,
) -> None:
    """Gera uma imagem a partir do PROMPT."""
    verbose = bool(ctx.obj.get("VERBOSE")) or verbose_flag

    table = Table(show_header=False, box=box.SIMPLE)
    table.add_row("[bold]Prompt[/bold]", f"[cyan]{prompt}[/cyan]")
    table.add_row("[bold]Resolução[/bold]", f"{width}x{height}")
    table.add_row("[bold]Passos[/bold]", str(steps))
    table.add_row("[bold]Guidance[/bold]", str(guidance_scale))
    table.add_row("[bold]Modelo[/bold]", model_id or default_model_id())
    console.print(
        Panel(table, title="[bold green]Configuração", border_style="green")
    )

    device = "cpu" if cpu else None
    low = low_vram or cpu

    try:
        gen = KleinFluxGenerator(
            device=device,
            low_vram=low,
            verbose=verbose,
            model_id=model_id,
        )

        # O trabalho pesado é from_pretrained (rede/disco), não o __init__
        with console.status(
            "[bold yellow]1/2 — Download HF + carregamento de pesos "
            "(1ª vez: vários GB/minutos; GPU pode mostrar 0% até ao passo 3/3)",
            spinner="dots",
        ):
            gen.warmup()

        if output is None:
            ts = int(time.time())
            safe = "".join(c if c.isalnum() else "_" for c in prompt[:40])
            output = str(DEFAULT_IMAGE_DIR / f"{safe}_{ts}.png")
        out_path = Path(output)

        start = time.time()
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=console,
        ) as progress:
            task = progress.add_task("[cyan]2/2 — Inferência na GPU...", total=None)
            image = gen.generate(
                prompt=prompt,
                height=height,
                width=width,
                guidance_scale=guidance_scale,
                num_inference_steps=steps,
                seed=seed,
            )
            progress.update(task, description="[green]Concluído")

        ext = out_path.suffix.lower().lstrip(".")
        img_format = "JPEG" if ext in ("jpg", "jpeg") else "PNG"
        KleinFluxGenerator.save_image(
            image,
            out_path,
            image_format=img_format if img_format == "JPEG" else "PNG",
        )

        elapsed = time.time() - start
        console.print(f"\n[bold green]✓[/bold green] Imagem: [cyan]{out_path.resolve()}[/cyan]")
        console.print(f"[dim]Tempo: {elapsed:.1f}s[/dim]")
    except ImportError as e:
        console.print(f"\n[bold red]✗[/bold red] {e}")
        sys.exit(1)
    except Exception as e:
        console.print(f"\n[bold red]✗ Erro:[/bold red] {e}")
        if verbose:
            console.print_exception()
        sys.exit(1)


@cli.command("info")
def info_cmd() -> None:
    """Informações do sistema e GPU."""
    data = get_system_info()
    t = Table(title="[bold blue]Sistema", box=box.ROUNDED)
    t.add_column("Componente", style="cyan", no_wrap=True)
    t.add_column("Valor", style="green")
    t.add_row("Python", data.get("python_version", "N/A"))
    t.add_row("PyTorch", data.get("torch_version", "N/A"))
    t.add_row("CUDA", str(data.get("cuda_available", False)))
    if data.get("cuda_available"):
        t.add_row("CUDA (versão)", str(data.get("cuda_version", "N/A")))
        for i, gpu in enumerate(data.get("gpus", [])):
            t.add_row(f"GPU {i}", str(gpu.get("name", "")))
            t.add_row("  └ VRAM total", format_bytes(gpu.get("total_memory", 0)))
            t.add_row("  └ VRAM livre", format_bytes(gpu.get("free_memory", 0)))
    t.add_row("HF cache", os.environ.get("HF_HOME", "~/.cache/huggingface"))
    t.add_row("Modelo (default)", default_model_id())
    console.print(t)


@cli.command("models")
def models_cmd() -> None:
    """Modelos suportados."""
    t = Table(title="[bold blue]Modelos", box=box.ROUNDED)
    t.add_column("ID", style="cyan")
    t.add_column("Notas", style="white")
    t.add_row(
        "Disty0/FLUX.2-klein-4B-SDNQ-4bit-dynamic",
        "Padrão (~2.5 GB pesos), SDNQ 4-bit, Apache 2.0 (base BFL)",
    )
    t.add_row(
        "black-forest-labs/FLUX.2-klein-4B",
        "Alternativa: BF16 completo, mais VRAM (TEXT2D_MODEL_ID)",
    )
    console.print(t)
    console.print(
        Panel(
            "[dim]Pesos GGUF (Unsloth) em geral usam ComfyUI-GGUF, não este CLI.[/dim]",
            border_style="dim",
        )
    )


def main() -> None:
    try:
        cli()
    except KeyboardInterrupt:
        console.print("\n[yellow]Cancelado.[/yellow]")
        sys.exit(130)


if __name__ == "__main__":
    main()
