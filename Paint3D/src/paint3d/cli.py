#!/usr/bin/env python3
"""
Paint3D - CLI Principal

Texturização 3D: Hunyuan3D-Paint + Materialize PBR + Upscale IA.
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
    Paint3D — texturização 3D com Hunyuan3D-Paint.

    Pipeline: mesh + imagem de referência → mesh texturizada (GLB).

    \b
        paint3d texture mesh.glb -i ref.png -o mesh_tex.glb
        paint3d texture mesh.glb -i ref.png -o mesh_pbr.glb --materialize
        paint3d materialize-pbr mesh_tex.glb -o mesh_pbr.glb
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
@click.option("--paint-full-gpu", is_flag=True, help="Mantém modelos Paint na GPU (VRAM alta).")
@click.option("--materialize", is_flag=True, help="Após Paint, gera mapas PBR (Materialize CLI) e embute no GLB.")
@click.option(
    "--materialize-preset",
    "materialize_preset",
    default="default",
    show_default=True,
    type=click.Choice(_defaults.MATERIALIZE_PRESETS),
    help="Preset Materialize: ajusta parâmetros PBR ao tipo de superfície.",
)
@click.option(
    "--materialize-output-dir",
    type=click.Path(file_okay=False, path_type=Path),
    default=None,
    help="Guarda PNGs dos mapas nesta pasta.",
)
@click.option(
    "--materialize-bin",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default=None,
    help="Binário materialize (defeito: PATH ou MATERIALIZE_BIN).",
)
@click.option("--materialize-no-invert", is_flag=True, help="Roughness = smoothness (sem 1-smoothness).")
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
    materialize,
    materialize_preset,
    materialize_output_dir,
    materialize_bin,
    materialize_no_invert,
    upscale,
    upscale_factor,
    texture_verbose,
    allow_shared_gpu,
    gpu_kill_others,
):
    """Aplica Hunyuan3D-Paint a uma mesh GLB/OBJ + imagem de referência → GLB texturizado."""
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
        "[bold]Paint[/bold]",
        f"{paint_subfolder} {'GPU' if paint_full_gpu else 'CPU offload'}",
    )
    if upscale:
        info_table.add_row("[bold]Upscale[/bold]", f"Real-ESRGAN {upscale_factor}x")
    if materialize:
        info_table.add_row(
            "[bold]Materialize PBR[/bold]",
            f"preset={materialize_preset}"
            + (f" → [cyan]{materialize_output_dir}[/cyan]" if materialize_output_dir else ""),
        )
    console.print(Panel(info_table, title="[bold green]Hunyuan3D-Paint", border_style="green"))

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
                materialize=materialize,
                materialize_output_dir=materialize_output_dir,
                materialize_bin=materialize_bin,
                materialize_no_invert=materialize_no_invert,
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


@cli.command("materialize-pbr")
@click.argument("mesh_file", type=click.Path(exists=True, dir_okay=False))
@click.option("--output", "-o", type=click.Path(), required=True, help="GLB de saída (PBR embutido).")
@click.option(
    "--materialize-output-dir",
    type=click.Path(file_okay=False, path_type=Path),
    default=None,
    help="Guarda PNGs dos mapas nesta pasta.",
)
@click.option(
    "--materialize-bin",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default=None,
)
@click.option("--materialize-no-invert", is_flag=True, help="Roughness = smoothness (sem 1-smoothness).")
@click.option(
    "--preset",
    "-p",
    "mat_preset",
    default="default",
    show_default=True,
    type=click.Choice(_defaults.MATERIALIZE_PRESETS),
    help="Preset Materialize: ajusta parâmetros PBR ao tipo de superfície.",
)
@click.option("-v", "--verbose", "mat_verbose", is_flag=True)
@click.option("--allow-shared-gpu", "allow_shared_gpu", is_flag=True)
@click.option("--gpu-kill-others/--no-gpu-kill-others", "gpu_kill_others", default=True)
@click.pass_context
def materialize_pbr_cmd(
    ctx,
    mesh_file,
    output,
    materialize_output_dir,
    materialize_bin,
    materialize_no_invert,
    mat_preset,
    mat_verbose,
    allow_shared_gpu,
    gpu_kill_others,
):
    """Só Materialize PBR: GLB já texturizado (Paint) → mapas + GLB PBR (sem re-correr Paint)."""
    from .materialize_pbr import apply_materialize_pbr
    from .utils.mesh_io import load_mesh_trimesh, save_glb

    verbose = bool(ctx.obj.get("VERBOSE")) or mat_verbose
    mesh_path = Path(mesh_file)
    out_path = Path(output)
    if out_path.suffix.lower() not in (".glb",):
        raise click.UsageError("Saída deve ser .glb")

    console.print(
        Panel(
            f"Entrada: [cyan]{mesh_path}[/cyan]\nSaída: [cyan]{out_path}[/cyan]",
            title="[bold green]Materialize PBR",
            border_style="green",
        )
    )

    _prepare_gpu(allow_shared_gpu, gpu_kill_others)

    try:
        start = time.time()
        with console.status(f"[bold yellow]Materialize PBR (preset={mat_preset})...", spinner="dots"):
            mesh = load_mesh_trimesh(mesh_path)
            result = apply_materialize_pbr(
                mesh,
                materialize_bin=materialize_bin,
                save_sidecar_maps_dir=materialize_output_dir,
                roughness_from_one_minus_smoothness=not materialize_no_invert,
                preset=mat_preset,
                verbose=verbose,
            )
            save_glb(result, out_path)

        out_p = out_path.resolve()
        try:
            sz = format_bytes(out_p.stat().st_size)
        except OSError:
            sz = "?"
        console.print(Rule("[bold green]Resultado", style="green"))
        console.print(f"[bold green]✓[/bold green] GLB PBR: [cyan]{out_p}[/cyan] [dim]({sz})[/dim]")
        console.print(f"\n[dim]Tempo: {time.time() - start:.1f}s[/dim]")
    except Exception as e:
        console.print(f"\n[bold red]✗ Erro:[/bold red] {e!s}")
        if verbose:
            console.print_exception()
        sys.exit(1)


@cli.command("doctor")
def doctor():
    """Verifica ambiente: PyTorch, CUDA, VRAM e extensão Hunyuan3D-Paint (custom_rasterizer)."""
    from gamedev_shared.gpu import (
        DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB,
        get_system_info,
        gpu_bytes_in_use,
    )

    from .painter import check_paint_rasterizer_available

    console.print(
        Panel.fit(
            "[bold]paint3d doctor[/bold] — PyTorch, CUDA, Paint (custom_rasterizer)",
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
        table.add_row("Hunyuan3D-Paint", f"[green]rasterizador OK — {backend}[/green]")
    except RuntimeError as e:
        msg = str(e).split("\n")[0][:120]
        table.add_row("Hunyuan3D-Paint", f"[yellow]{msg}…[/yellow]")

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
        "Hunyuan3D-Paint",
        "Textura multivista (tencent/Hunyuan3D-2, delight + paint)",
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
