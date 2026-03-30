#!/usr/bin/env python3
"""
Paint3D - CLI Principal

Texturização 3D: Hunyuan3D-Paint 2.1 (PBR no GLB) + Upscale IA.
"""

import os
import sys
import time
from pathlib import Path

from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.rule import Rule
from rich.table import Table

from gamedev_shared.gpu import (
    clear_cuda_memory,
    enforce_exclusive_gpu,
    format_bytes,
    kill_gpu_compute_processes_aggressive,
)
from gamedev_shared.hf import hf_home_display_rich

from . import defaults as _defaults
from .cli_rich import click

console = Console()


def _env_allow_shared_gpu() -> bool:
    return os.environ.get("PAINT3D_ALLOW_SHARED_GPU", "").strip().lower() in (
        "1",
        "true",
        "yes",
        "on",
    )


def _gpu_kill_others_effective(cli_wants: bool) -> bool:
    v = os.environ.get("PAINT3D_GPU_KILL_OTHERS", "").strip().lower()
    if v in ("0", "false", "no", "off"):
        return False
    if v in ("1", "true", "yes", "on"):
        return True
    return cli_wants


def _prepare_gpu(allow_shared_gpu: bool, gpu_kill_others: bool) -> None:
    """Kill outros processos GPU e verifica exclusividade."""
    allow_shared = bool(allow_shared_gpu) or _env_allow_shared_gpu()
    gpu_kill = _gpu_kill_others_effective(bool(gpu_kill_others))
    if gpu_kill:
        console.print(
            Panel(
                "[bold]Terminar processos GPU alvo[/bold]\n"
                "[dim]Desliga com [bold]--no-gpu-kill-others[/bold] ou PAINT3D_GPU_KILL_OTHERS=0[/dim]",
                border_style="yellow",
            )
        )
        for line in kill_gpu_compute_processes_aggressive(exclude_pid=os.getpid()):
            console.print(f"[dim]{line}[/dim]")
        clear_cuda_memory()
        time.sleep(0.5)
    try:
        enforce_exclusive_gpu(allow_shared=allow_shared)
    except RuntimeError as e:
        raise click.ClickException(str(e)) from e


@click.group()
@click.version_option(version="0.1.0", prog_name="paint3d")
@click.option("--verbose", "-v", is_flag=True, help="Modo verbose com logs detalhados")
@click.pass_context
def cli(ctx, verbose):
    """
    Paint3D — texturização 3D com Hunyuan3D-Paint 2.1 (PBR).

    Pipeline: mesh + imagem de referência → mesh texturizada (GLB).

    \b
        paint3d texture mesh.glb -i ref.png -o mesh_tex.glb
        paint3d doctor
    """
    ctx.ensure_object(dict)
    ctx.obj["VERBOSE"] = verbose


@cli.command()
@click.argument("mesh_file", type=click.Path(exists=True, dir_okay=False))
@click.option(
    "--image",
    "-i",
    "image_file",
    required=True,
    type=click.Path(exists=True, dir_okay=False),
    help="Imagem de referência (alinhada com a mesh).",
)
@click.option("--output", "-o", type=click.Path(), help="Ficheiro GLB de saída (textura embutida).")
@click.option(
    "--paint-repo",
    default=_defaults.DEFAULT_PAINT_HF_REPO,
    show_default=True,
)
@click.option(
    "--paint-subfolder",
    default=_defaults.DEFAULT_PAINT_SUBFOLDER,
    show_default=True,
)
@click.option(
    "--paint-full-gpu",
    is_flag=True,
    help="Resoluções internas máximas (VRAM alta; recomendado para Paint 2.1).",
)
@click.option(
    "--upscale/--no-upscale",
    "upscale",
    default=_defaults.DEFAULT_UPSCALE,
    show_default=True,
    help="Upscale IA da textura (Real-ESRGAN). Requer: pip install spandrel.",
)
@click.option(
    "--upscale-factor",
    default=_defaults.DEFAULT_UPSCALE_FACTOR,
    show_default=True,
    type=click.Choice(["2", "4"], case_sensitive=False),
    help="Factor de upscale (2 = 1024→2048, 4 = 1024→4096).",
)
@click.option("-v", "--verbose", "texture_verbose", is_flag=True, help="Logs detalhados.")
@click.option("--allow-shared-gpu", "allow_shared_gpu", is_flag=True, help="Permite GPU com outros processos.")
@click.option(
    "--gpu-kill-others/--no-gpu-kill-others",
    "gpu_kill_others",
    default=True,
    help="Termina outros processos GPU antes de inferir; defeito: ligado.",
)
@click.pass_context
def texture(
    ctx,
    mesh_file,
    image_file,
    output,
    paint_repo,
    paint_subfolder,
    paint_full_gpu,
    upscale,
    upscale_factor,
    texture_verbose,
    allow_shared_gpu,
    gpu_kill_others,
):
    """Aplica Hunyuan3D-Paint 2.1 a uma mesh GLB/OBJ + imagem de referência → GLB texturizado (PBR baked)."""
    from .painter import paint_file_to_file

    verbose = bool(ctx.obj.get("VERBOSE")) or texture_verbose
    mesh_path = Path(mesh_file)
    if output is None:
        output = mesh_path.with_name(f"{mesh_path.stem}_textured.glb")

    info_table = Table(show_header=False, box=box.SIMPLE)
    info_table.add_row("[bold]Mesh[/bold]", f"[cyan]{mesh_path}[/cyan]")
    info_table.add_row("[bold]Imagem[/bold]", f"[cyan]{image_file}[/cyan]")
    info_table.add_row("[bold]Saída[/bold]", f"[cyan]{output}[/cyan]")
    info_table.add_row(
        "[bold]Paint 2.1[/bold]",
        f"{paint_repo} / {paint_subfolder} — {'VRAM alta' if paint_full_gpu else 'modo económico'}",
    )
    if upscale:
        info_table.add_row("[bold]Upscale[/bold]", f"Real-ESRGAN {upscale_factor}x")
    console.print(Panel(info_table, title="[bold green]Hunyuan3D-Paint 2.1", border_style="green"))

    _prepare_gpu(allow_shared_gpu, gpu_kill_others)

    try:
        start = time.time()
        with console.status("[bold yellow]A carregar modelos Paint (1ª vez: download HF)...", spinner="dots"):
            out = paint_file_to_file(
                mesh_path,
                image_file,
                output,
                model_repo=paint_repo,
                subfolder=paint_subfolder,
                paint_cpu_offload=not paint_full_gpu,
                verbose=verbose,
            )

        if upscale:
            from .texture_upscale import upscale_trimesh_texture
            from .utils.mesh_io import load_mesh_trimesh, save_glb

            clear_cuda_memory()
            with console.status(
                f"[bold yellow]Upscale textura (Real-ESRGAN {upscale_factor}x)...",
                spinner="dots",
            ):
                mesh = load_mesh_trimesh(out)
                mesh = upscale_trimesh_texture(
                    mesh,
                    scale=int(upscale_factor),
                    verbose=verbose,
                )
                save_glb(mesh, out)

        out_p = Path(out).resolve()
        try:
            sz = format_bytes(out_p.stat().st_size)
        except OSError:
            sz = "?"
        console.print(Rule("[bold green]Resultado", style="green"))
        console.print(f"[bold green]✓[/bold green] GLB texturizado: [cyan]{out_p}[/cyan] [dim]({sz})[/dim]")
        console.print(f"\n[dim]Tempo: {time.time() - start:.1f}s[/dim]")
    except Exception as e:
        console.print(f"\n[bold red]✗ Erro:[/bold red] {e!s}")
        if verbose:
            console.print_exception()
        sys.exit(1)


@cli.command("doctor")
def doctor():
    """Verifica ambiente: PyTorch, CUDA, VRAM, hy3dpaint 2.1, Real-ESRGAN e rasterizador."""
    from gamedev_shared.gpu import (
        DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB,
        get_system_info,
        gpu_bytes_in_use,
    )

    from .painter import check_hunyuan3d21_environment, check_paint_rasterizer_available

    console.print(
        Panel.fit(
            "[bold]paint3d doctor[/bold] — PyTorch, CUDA, Paint 2.1 (hy3dpaint, Real-ESRGAN, rasterizador)",
            border_style="blue",
        )
    )
    info_data = get_system_info()
    table = Table(title="[bold blue]Diagnóstico", box=box.ROUNDED)
    table.add_column("Item", style="cyan", no_wrap=True)
    table.add_column("Estado", style="green")

    alloc = os.environ.get("PYTORCH_CUDA_ALLOC_CONF", "")
    table.add_row(
        "PYTORCH_CUDA_ALLOC_CONF",
        alloc or "[dim](não definido)[/dim]",
    )
    table.add_row("PyTorch", info_data.get("torch_version", "N/A"))
    table.add_row("CUDA (torch)", str(info_data.get("cuda_available", False)))
    if info_data.get("cuda_available"):
        table.add_row("CUDA (versão runtime)", info_data.get("cuda_version", "N/A"))
        for i, gpu in enumerate(info_data.get("gpus", [])):
            table.add_row(
                f"GPU {i}",
                f"{gpu['name']} — {format_bytes(gpu['total_memory'])} total, {format_bytes(gpu['free_memory'])} livre",
            )
        used = gpu_bytes_in_use(0)
        if used is not None:
            table.add_row(
                "Política GPU exclusiva",
                f"~{used / (1024**2):.0f} MiB em uso agora — "
                f"texture/materialize recusam se > {DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB} MiB "
                f"(ou PAINT3D_ALLOW_SHARED_GPU=1 / --allow-shared-gpu)",
            )

    try:
        check_paint_rasterizer_available()
        import custom_rasterizer

        backend = "nvdiffrast (shim)" if getattr(custom_rasterizer, "IS_NVDIFFRAST_SHIM", False) else "nativo"
        table.add_row("Rasterizador (custom_rasterizer)", f"[green]OK — {backend}[/green]")
    except RuntimeError as e:
        msg = str(e).split("\n")[0][:120]
        table.add_row("Rasterizador (custom_rasterizer)", f"[yellow]{msg}…[/yellow]")

    ok21, msg21 = check_hunyuan3d21_environment()
    if ok21:
        table.add_row("Hunyuan3D-2.1 (hy3dpaint)", f"[green]{msg21}[/green]")
    else:
        short = msg21.split("\n")[0][:160]
        table.add_row("Hunyuan3D-2.1 (hy3dpaint)", f"[yellow]{short}[/yellow]")

    console.print(table)
    console.print(
        Panel(
            "[dim]Paint: ver docs/PAINT_SETUP.md[/dim]",
            border_style="dim",
        )
    )


@cli.command()
def info():
    """Informações do sistema e GPU."""
    from gamedev_shared.gpu import get_system_info

    console.print(
        Panel.fit(
            "[bold]paint3d info[/bold] — GPU e cache",
            border_style="blue",
        )
    )
    info_data = get_system_info()

    table = Table(title="[bold blue]Sistema", box=box.ROUNDED)
    table.add_column("Componente", style="cyan", no_wrap=True)
    table.add_column("Valor", style="green")

    table.add_row("Python", info_data.get("python_version", "N/A"))
    table.add_row("PyTorch", info_data.get("torch_version", "N/A"))
    table.add_row("CUDA", str(info_data.get("cuda_available", False)))

    if info_data.get("cuda_available"):
        table.add_row("CUDA (versão)", info_data.get("cuda_version", "N/A"))
        for i, gpu in enumerate(info_data.get("gpus", [])):
            table.add_row(f"GPU {i}", f"{gpu['name']}")
            table.add_row("  └ VRAM total", format_bytes(gpu["total_memory"]))
            table.add_row("  └ VRAM livre", format_bytes(gpu["free_memory"]))

    table.add_row("HF_HOME (cache Hub)", hf_home_display_rich())
    console.print(table)


@cli.command()
def models():
    """Modelos usados pelo Paint3D."""
    table = Table(title="[bold blue]Modelos", box=box.ROUNDED)
    table.add_column("Componente", style="cyan")
    table.add_column("Descrição", style="magenta")
    table.add_column("Notas", style="dim")

    table.add_row(
        "Hunyuan3D-Paint 2.1",
        "PBR multivista (tencent/Hunyuan3D-2.1, hunyuan3d-paintpbr-v2-1)",
        "Comando: paint3d texture",
    )
    table.add_row(
        "Real-ESRGAN",
        "Upscale IA de texturas (4x)",
        "Opcional: pip install spandrel",
    )

    console.print(table)
    console.print(
        Panel(
            "[dim]Primeira execução: descarrega pesos do Hugging Face (~vários GB).\n"
            "Cache: ~/.cache/huggingface/hub/[/dim]",
            title="Nota",
            border_style="dim",
        )
    )


def main():
    try:
        cli()
    except KeyboardInterrupt:
        console.print("\n[yellow]Cancelado.[/yellow]")
        sys.exit(130)


if __name__ == "__main__":
    main()
