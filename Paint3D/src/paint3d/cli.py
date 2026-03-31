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
@click.option(
    "--max-views",
    default=_defaults.DEFAULT_PAINT_MAX_VIEWS,
    show_default=True,
    type=int,
    help="Número de vistas multiview (menos = mais rápido; mín 2).",
)
@click.option(
    "--view-resolution",
    default=_defaults.DEFAULT_PAINT_VIEW_RESOLUTION,
    show_default=True,
    type=int,
    help="Resolução das vistas internas (menor = mais rápido; recomendado: 256-512).",
)
@click.option("-v", "--verbose", "texture_verbose", is_flag=True, help="Logs detalhados.")
@click.option("--allow-shared-gpu", "allow_shared_gpu", is_flag=True, help="Permite GPU com outros processos.")
@click.option(
    "--gpu-kill-others/--no-gpu-kill-others",
    "gpu_kill_others",
    default=True,
    help="Termina outros processos GPU antes de inferir; defeito: ligado.",
)
# --- Otimizações de VRAM ---
@click.option(
    "--quantization",
    "-q",
    type=click.Choice(["auto", "none", "fp8", "int8", "int4", "quanto-int8", "quanto-int4"], case_sensitive=False),
    default=_defaults.DEFAULT_QUANTIZATION_MODE,
    show_default=True,
    help="Modo de quantização: auto (detecta VRAM), fp8 (RTX 40+), int8/int4 (bitsandbytes), quanto-*.",
)
@click.option(
    "--tiny-vae/--no-tiny-vae",
    default=_defaults.DEFAULT_USE_TINY_VAE,
    show_default=True,
    help="Usar TAESD (Tiny VAE) para reduzir VRAM do VAE em ~50%.",
)
@click.option(
    "--vae-slicing/--no-vae-slicing",
    default=_defaults.DEFAULT_ENABLE_VAE_SLICING,
    show_default=True,
    help="Habilitar slicing do VAE para batch processing.",
)
@click.option(
    "--vae-tiling/--no-vae-tiling",
    default=_defaults.DEFAULT_ENABLE_VAE_TILING,
    show_default=True,
    help="Habilitar tiling do VAE para imagens grandes.",
)
@click.option(
    "--torch-compile/--no-torch-compile",
    default=_defaults.DEFAULT_TORCH_COMPILE,
    show_default=True,
    help="Compilar UNet com torch.compile para speedup de inferência.",
)
@click.option(
    "--attention-slicing/--no-attention-slicing",
    default=_defaults.DEFAULT_ENABLE_ATTENTION_SLICING,
    show_default=True,
    help="Habilitar attention slicing para reduzir pico de VRAM.",
)
@click.option(
    "--low-vram-mode",
    is_flag=True,
    help="Ativa todas as otimizações para GPUs com <8GB VRAM (quantização int4, tiny VAE, slicing).",
)
@click.option(
    "--rtx4050-mode",
    is_flag=True,
    help="Modo específico RTX 4050 6GB: BF16, int4, xformers, tiny VAE, tile 128, no compile.",
)
@click.option(
    "--xformers/--no-xformers",
    default=_defaults.DEFAULT_USE_XFORMERS,
    show_default=True,
    help="Usar xformers memory efficient attention (quando disponível).",
)
@click.option(
    "--dtype",
    type=click.Choice(["float16", "bfloat16", "float32"], case_sensitive=False),
    default=_defaults.DEFAULT_DTYPE,
    show_default=True,
    help="Tipo de dados: bfloat16 recomendado para RTX 40 series, float16 para outras.",
)
@click.option(
    "--vae-tile-size",
    default=_defaults.DEFAULT_VAE_TILE_SIZE,
    show_default=True,
    type=int,
    help="Tamanho do tile VAE (menor = menos VRAM, mais lento).",
)
@click.option(
    "--profile",
    is_flag=True,
    help="Medir tempos, CPU, RAM e VRAM (JSONL opcional: GAMEDEV_PROFILE_LOG).",
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
    max_views,
    view_resolution,
    texture_verbose,
    allow_shared_gpu,
    gpu_kill_others,
    quantization,
    tiny_vae,
    vae_slicing,
    vae_tiling,
    torch_compile,
    attention_slicing,
    low_vram_mode,
    rtx4050_mode,
    xformers,
    dtype,
    vae_tile_size,
    profile,
):
    """Aplica Hunyuan3D-Paint 2.1 a uma mesh GLB/OBJ + imagem de referência → GLB texturizado (PBR baked)."""
    from gamedev_shared.quantization import (
        format_quantization_info,
        get_quantization_config,
        suggest_environment_variables,
    )

    from .painter import paint_file_to_file

    verbose = bool(ctx.obj.get("VERBOSE")) or texture_verbose
    mesh_path = Path(mesh_file)
    if output is None:
        output = mesh_path.with_name(f"{mesh_path.stem}_textured.glb")

    # Aplicar modo específico RTX 4050 6GB
    if rtx4050_mode:
        console.print("[bold cyan]Modo RTX 4050 6GB ativado - Aplicando otimizações máximas[/bold cyan]")
        quantization = "int4"
        tiny_vae = True
        vae_slicing = True
        vae_tiling = True
        vae_tile_size = 128  # Tiles menores para economizar VRAM
        attention_slicing = True
        xformers = True
        dtype = "bfloat16"  # BF16 é melhor em Ada Lovelace
        torch_compile = False  # Compilação usa VRAM extra, desabilitar em 6GB
        paint_full_gpu = False
        # Configurar ambiente CUDA específico para 6GB
        os.environ["PYTORCH_CUDA_ALLOC_CONF"] = "expandable_segments:True,max_split_size_mb:64,garbage_collection_threshold:0.6"

    # Aplicar low-vram-mode se solicitado (genérico)
    elif low_vram_mode:
        quantization = "int4"
        tiny_vae = True
        vae_slicing = True
        vae_tiling = True
        attention_slicing = True
        paint_full_gpu = False

    # Configurar variáveis de ambiente para otimização de memória
    try:
        import torch

        if torch.cuda.is_available():
            vram_gb = torch.cuda.get_device_properties(0).total_memory / (1024**3)
            gpu_name = torch.cuda.get_device_properties(0).name

            # Auto-detectar RTX 4050 se não foi manualmente configurado
            if not rtx4050_mode and ("rtx 4050" in gpu_name.lower() or vram_gb <= 6.5):
                console.print(f"[yellow]Detectada GPU {gpu_name} com {vram_gb:.1f}GB VRAM[/yellow]")
                console.print("[dim]Sugestão: use --rtx4050-mode para otimizações automáticas[/dim]")

            env_vars = suggest_environment_variables(vram_gb)
            for key, value in env_vars.items():
                if key not in os.environ:
                    os.environ[key] = value
                    if verbose:
                        console.print(f"[dim]Configurado {key}={value}[/dim]")
    except Exception:
        pass

    # Obter configuração de quantização
    quant_config = get_quantization_config(quantization)
    quant_str = format_quantization_info(quant_config)

    info_table = Table(show_header=False, box=box.SIMPLE)
    info_table.add_row("[bold]Mesh[/bold]", f"[cyan]{mesh_path}[/cyan]")
    info_table.add_row("[bold]Imagem[/bold]", f"[cyan]{image_file}[/cyan]")
    info_table.add_row("[bold]Saída[/bold]", f"[cyan]{output}[/cyan]")
    mode_str = "VRAM alta" if paint_full_gpu else f"otimizado ({quant_str})"
    info_table.add_row(
        "[bold]Paint 2.1[/bold]",
        f"{paint_repo} / {paint_subfolder} — {mode_str} — {max_views} vistas @ {view_resolution}px",
    )
    if tiny_vae:
        info_table.add_row("[bold]VAE[/bold]", "TAESD (Tiny VAE)")
    if torch_compile:
        info_table.add_row("[bold]Compile[/bold]", "torch.compile ativo")
    if xformers:
        info_table.add_row("[bold]Attention[/bold]", "xformers memory efficient")
    info_table.add_row("[bold]Dtype[/bold]", dtype.upper())
    if rtx4050_mode:
        info_table.add_row("[bold]Perfil[/bold]", "[bold cyan]RTX 4050 6GB[/bold cyan]")
    if upscale:
        info_table.add_row("[bold]Upscale[/bold]", f"Real-ESRGAN {upscale_factor}x")
    if profile:
        info_table.add_row("[bold]Profiler[/bold]", "activo (spans + resumo no fim)")
    console.print(Panel(info_table, title="[bold green]Hunyuan3D-Paint 2.1", border_style="green"))

    _prepare_gpu(allow_shared_gpu, gpu_kill_others)

    from gamedev_shared.profiler import ProfilerSession
    from gamedev_shared.profiler.env import env_profile_log_path

    log_p = env_profile_log_path()
    prof_log = Path(log_p) if log_p else None

    try:
        start = time.time()
        with ProfilerSession("paint3d", log_path=prof_log, cli_profile=profile) as prof:
            with console.status("[bold yellow]A carregar modelos Paint (1ª vez: download HF)...", spinner="dots"):
                if prof.enabled:
                    with prof.span("texture_total", sync_cuda=True):
                        out = paint_file_to_file(
                            mesh_path,
                            image_file,
                            output,
                            model_repo=paint_repo,
                            subfolder=paint_subfolder,
                            paint_cpu_offload=not paint_full_gpu,
                            max_num_view=max_views,
                            view_resolution=view_resolution,
                            verbose=verbose,
                            quantization_mode=quantization,
                            use_tiny_vae=tiny_vae,
                            enable_vae_slicing=vae_slicing,
                            enable_vae_tiling=vae_tiling,
                            vae_tile_size=vae_tile_size,
                            enable_torch_compile=torch_compile,
                            enable_attention_slicing=attention_slicing,
                            use_xformers=xformers,
                            dtype=dtype,
                        )
                else:
                    out = paint_file_to_file(
                        mesh_path,
                        image_file,
                        output,
                        model_repo=paint_repo,
                        subfolder=paint_subfolder,
                        paint_cpu_offload=not paint_full_gpu,
                        max_num_view=max_views,
                        view_resolution=view_resolution,
                        verbose=verbose,
                        quantization_mode=quantization,
                        use_tiny_vae=tiny_vae,
                        enable_vae_slicing=vae_slicing,
                        enable_vae_tiling=vae_tiling,
                        vae_tile_size=vae_tile_size,
                        enable_torch_compile=torch_compile,
                        enable_attention_slicing=attention_slicing,
                        use_xformers=xformers,
                        dtype=dtype,
                    )

            if upscale:
                from .texture_upscale import upscale_trimesh_texture
                from .utils.mesh_io import load_mesh_trimesh, save_glb

                clear_cuda_memory()
                with console.status(
                    f"[bold yellow]Upscale textura (Real-ESRGAN {upscale_factor}x)...",
                    spinner="dots",
                ):
                    if prof.enabled:
                        with prof.span("upscale", sync_cuda=True):
                            mesh = load_mesh_trimesh(out)
                            mesh = upscale_trimesh_texture(
                                mesh,
                                scale=int(upscale_factor),
                                verbose=verbose,
                            )
                            save_glb(mesh, out)
                    else:
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
