#!/usr/bin/env python3
"""
Text3D - CLI Principal

Text-to-3D: Text2D (texto → imagem) + Hunyuan3D-2mini (imagem → mesh).
"""

import os
import sys
import time
from pathlib import Path

from . import cli_rich  # noqa: F401 — configura rich-click antes dos comandos

if cli_rich.RICH_CLICK:
    import rich_click as click
else:
    import click
from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.rule import Rule
from rich.table import Table

from . import defaults as _defaults
from .generator import HunyuanTextTo3DGenerator
from .utils.env import ensure_pytorch_cuda_alloc_conf
from .utils.memory import (
    clear_cuda_memory,
    enforce_exclusive_gpu,
    format_bytes,
    kill_gpu_compute_processes_aggressive,
)
from .cursor_skill_install import install_agent_skill

console = Console()

DEFAULT_OUTPUT_DIR = Path("outputs")


def _env_allow_shared_gpu() -> bool:
    return os.environ.get("TEXT3D_ALLOW_SHARED_GPU", "").strip().lower() in (
        "1",
        "true",
        "yes",
        "on",
    )


def _gpu_kill_others_effective(cli_wants: bool) -> bool:
    """TEXT3D_GPU_KILL_OTHERS=0 desliga; =1 força; vazio segue o CLI."""
    v = os.environ.get("TEXT3D_GPU_KILL_OTHERS", "").strip().lower()
    if v in ("0", "false", "no", "off"):
        return False
    if v in ("1", "true", "yes", "on"):
        return True
    return cli_wants


DEFAULT_MESH_DIR = DEFAULT_OUTPUT_DIR / "meshes"


def ensure_dirs():
    DEFAULT_MESH_DIR.mkdir(parents=True, exist_ok=True)


@click.group()
@click.version_option(version="0.1.0", prog_name="text3d")
@click.option("--verbose", "-v", is_flag=True, help="Modo verbose com logs detalhados")
@click.pass_context
def cli(ctx, verbose):
    """
    Text3D — mesh 3D a partir de texto (Text2D + Hunyuan3D-2mini).

    \b
        text3d generate "um robô futurista" --output robo.glb
        text3d generate "carro" --preset hq --final -o carro.glb
        text3d texture mesh.glb -i ref.png -o mesh_tex.glb
        text3d texture mesh.glb -i ref.png -o mesh_pbr.glb --materialize
        text3d doctor
        text3d generate -v "prompt" --low-vram
        text3d -v generate "prompt"
        text3d info

    Na primeira execução pode parecer parado: download dos pesos (HF) e duas
    fases (Text2D, depois Hunyuan). Use -v para ver o progresso no log.
    """
    ensure_pytorch_cuda_alloc_conf()
    ctx.ensure_object(dict)
    ctx.obj["VERBOSE"] = verbose
    # Não criar outputs/meshes aqui: só quando --output omite caminho (usa pasta por defeito).


@cli.group("skill")
def skill_group() -> None:
    """Agent Skills Cursor (instalação no projeto do jogo)."""


@skill_group.command("install")
@click.option(
    "--target",
    "-t",
    type=click.Path(file_okay=False, writable=True, path_type=Path),
    default=".",
    help="Raiz do projeto do jogo (cria .cursor/skills/text3d/)",
)
@click.option("--force", is_flag=True, help="Sobrescrever SKILL.md existente")
def skill_install_cmd(target: Path, force: bool) -> None:
    """Copia SKILL.md para .cursor/skills/text3d/."""
    try:
        dest = install_agent_skill(target, force=force)
    except FileNotFoundError as e:
        raise click.ClickException(str(e)) from e
    except FileExistsError as e:
        raise click.ClickException(f"{e} — usa --force para substituir.") from e
    console.print(
        Panel(
            f"Skill copiada para [bold cyan]{dest}[/bold cyan]",
            title="[bold green]OK[/bold green]",
            border_style="green",
        )
    )


@cli.command()
@click.argument("prompt", required=False)
@click.option(
    "--from-image",
    "-i",
    "from_image",
    type=click.Path(exists=True, dir_okay=False),
    default=None,
    help="Imagem já gerada: só corre Hunyuan3D (sem Text2D).",
)
@click.option("--output", "-o", type=click.Path(), help="Ficheiro de saída (.glb, .ply, .obj)")
@click.option(
    "--format",
    "-f",
    "output_format",
    default="glb",
    type=click.Choice(["glb", "ply", "obj"]),
    help="Formato de saída",
)
@click.option("--cpu", is_flag=True, help="Forçar CPU (muito mais lento)")
@click.option(
    "--low-vram",
    is_flag=True,
    help="Força Hunyuan3D em CPU (muito lento). O perfil por defeito já é para ~6GB VRAM em CUDA.",
)
@click.option(
    "--image-width",
    "-W",
    default=_defaults.DEFAULT_T2D_WIDTH,
    show_default=True,
    type=int,
    help="Largura Text2D (ex.: 1024 em GPU grande)",
)
@click.option(
    "--image-height",
    "-H",
    default=_defaults.DEFAULT_T2D_HEIGHT,
    show_default=True,
    type=int,
    help="Altura Text2D (ex.: 1024 em GPU grande)",
)
@click.option(
    "--t2d-steps",
    default=_defaults.DEFAULT_T2D_STEPS,
    show_default=True,
    type=int,
    help="Passos de inferência Text2D",
)
@click.option(
    "--t2d-guidance",
    default=_defaults.DEFAULT_T2D_GUIDANCE,
    show_default=True,
    type=float,
    help="Guidance Text2D (recomendado ~1.0 para SDNQ)",
)
@click.option(
    "--model",
    "-m",
    "text2d_model_id",
    default=None,
    help="Modelo Hugging Face Text2D (default: env TEXT2D_MODEL_ID ou Disty0)",
)
@click.option(
    "--t2d-full-gpu",
    is_flag=True,
    help="Text2D inteiro na GPU (precisa ~12GB+ VRAM). O defeito usa CPU offload no FLUX.",
)
@click.option("--seed", type=int, default=None, help="Seed para Text2D e Hunyuan (mesmo valor)")
@click.option(
    "--steps",
    "-s",
    default=_defaults.DEFAULT_HY_STEPS,
    show_default=True,
    type=int,
    help=f"Passos Hunyuan3D (alta qualidade HF: {_defaults.HUNYUAN_HQ_STEPS})",
)
@click.option(
    "--guidance",
    "-gs",
    default=_defaults.DEFAULT_HY_GUIDANCE,
    show_default=True,
    type=float,
    help="Guidance Hunyuan3D (image-to-3D)",
)
@click.option(
    "--octree-resolution",
    default=_defaults.DEFAULT_OCTREE_RESOLUTION,
    show_default=True,
    type=int,
    help=(
        "Octree Hunyuan (VRAM no decode). "
        f"HQ em GPU grande: {_defaults.HUNYUAN_HQ_OCTREE}"
    ),
)
@click.option(
    "--num-chunks",
    default=_defaults.DEFAULT_NUM_CHUNKS,
    show_default=True,
    type=int,
    help=(
        "Chunks extração de superfície. "
        f"HQ: {_defaults.HUNYUAN_HQ_NUM_CHUNKS}"
    ),
)
@click.option(
    "--preset",
    type=click.Choice(["fast", "balanced", "hq"]),
    default=None,
    help=(
        "Perfil Hunyuan (steps + octree + chunks): fast (rápido, menos VRAM), "
        "balanced (defeito), hq (alta qualidade, GPU grande). "
        "Substitui --steps, --octree-resolution e --num-chunks."
    ),
)
@click.option(
    "--mc-level",
    default=_defaults.DEFAULT_MC_LEVEL,
    show_default=True,
    type=float,
    help="Nível marching cubes Hunyuan (0 = defeito; ajustes finos à iso-superfície).",
)
@click.option(
    "--no-mesh-repair",
    "no_mesh_repair",
    is_flag=True,
    default=False,
    help="Desliga pós-processo: maior componente conexa + merge de vértices (ilhas/pés soltos).",
)
@click.option(
    "--mesh-smooth",
    default=_defaults.DEFAULT_MESH_SMOOTH,
    show_default=True,
    type=int,
    help="Suavização Laplaciana (1–2 reduz aspereza; pode arredondar detalhes finos).",
)
@click.option(
    "--texture",
    "--final",
    "--with-texture",
    "texture",
    is_flag=True,
    help="Após a mesh, aplica Hunyuan3D-Paint (textura; usa a imagem Text2D ou --from-image). "
    "Alias: --final, --with-texture.",
)
@click.option(
    "--model-subfolder",
    default=_defaults.DEFAULT_SUBFOLDER,
    show_default=True,
    help="Subpasta do modelo Hunyuan3D shape (ex.: hunyuan3d-dit-v2-mini-turbo).",
)
@click.option(
    "--paint-repo",
    default=_defaults.DEFAULT_PAINT_HF_REPO,
    show_default=True,
    help="Repo HF com delight + paint (subpastas hunyuan3d-*).",
)
@click.option(
    "--paint-subfolder",
    default=_defaults.DEFAULT_PAINT_SUBFOLDER,
    show_default=True,
    help="Subpasta Paint (ex.: hunyuan3d-paint-v2-0-turbo).",
)
@click.option(
    "--paint-full-gpu",
    is_flag=True,
    help="Mantém modelos Paint na GPU (VRAM alta). O defeito usa CPU offload.",
)
@click.option(
    "--materialize",
    is_flag=True,
    help="Após Paint, gera mapas PBR (Materialize CLI) e embute no GLB (normal, AO, metallic-roughness).",
)
@click.option(
    "--materialize-output-dir",
    type=click.Path(file_okay=False, path_type=Path),
    default=None,
    help="Guarda PNGs (mapas Materialize + baseColor/metallicRoughness/occlusion) nesta pasta.",
)
@click.option(
    "--materialize-bin",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default=None,
    help="Binário materialize (defeito: PATH ou MATERIALIZE_BIN).",
)
@click.option(
    "--materialize-no-invert",
    is_flag=True,
    help="Roughness = smoothness (sem 1−smoothness).",
)
@click.option(
    "-v",
    "--verbose",
    "generate_verbose",
    is_flag=True,
    help="Logs detalhados (equivale a: text3d -v generate ...)",
)
@click.option(
    "--allow-shared-gpu",
    "allow_shared_gpu",
    is_flag=True,
    help="Permite GPU com outros processos (desliga verificação: ~300 MiB máx. já ocupados).",
)
@click.option(
    "--gpu-kill-others/--no-gpu-kill-others",
    "gpu_kill_others",
    default=True,
    help="Termina outros processos GPU (nvidia-smi) antes de inferir; defeito: ligado.",
)
@click.pass_context
def generate(
    ctx,
    prompt,
    from_image,
    output,
    output_format,
    cpu,
    low_vram,
    image_width,
    image_height,
    t2d_steps,
    t2d_guidance,
    text2d_model_id,
    t2d_full_gpu,
    seed,
    steps,
    guidance,
    octree_resolution,
    num_chunks,
    preset,
    mc_level,
    no_mesh_repair,
    mesh_smooth,
    texture,
    paint_repo,
    paint_subfolder,
    paint_full_gpu,
    materialize,
    materialize_output_dir,
    materialize_bin,
    materialize_no_invert,
    generate_verbose,
    allow_shared_gpu,
    gpu_kill_others,
    model_subfolder,
):
    """Gera 3D: PROMPT (Text2D → Hunyuan) ou --from-image (só Hunyuan)."""
    verbose = bool(ctx.obj.get("VERBOSE")) or generate_verbose

    if materialize and not texture:
        raise click.UsageError("--materialize requer --texture (Hunyuan3D-Paint primeiro).")

    if preset is not None:
        pv = _defaults.PRESET_HUNYUAN[preset]
        steps = pv["steps"]
        octree_resolution = pv["octree"]
        num_chunks = pv["chunks"]

    if texture:
        output_format = "glb"

    if not from_image and not (prompt and str(prompt).strip()):
        raise click.UsageError("Indica um PROMPT em texto ou --from-image /path/to.png")

    allow_shared = bool(allow_shared_gpu) or _env_allow_shared_gpu()
    gpu_kill = _gpu_kill_others_effective(bool(gpu_kill_others))
    if not cpu and gpu_kill:
        console.print(
            Panel(
                "[bold]Terminar processos GPU alvo[/bold] (SIGTERM → espera → SIGKILL se vivo)\n"
                "[dim]Mantém o PID atual e Xorg / gnome-shell / Wayland. "
                "Desliga com [bold]--no-gpu-kill-others[/bold] ou TEXT3D_GPU_KILL_OTHERS=0[/dim]",
                border_style="yellow",
            )
        )
        for line in kill_gpu_compute_processes_aggressive(exclude_pid=os.getpid()):
            console.print(f"[dim]{line}[/dim]")
        clear_cuda_memory()
        time.sleep(0.5)
    if not cpu:
        try:
            enforce_exclusive_gpu(allow_shared=allow_shared)
        except RuntimeError as e:
            raise click.ClickException(str(e)) from e

    info_table = Table(show_header=False, box=box.SIMPLE)
    if from_image:
        info_table.add_row("[bold]Entrada[/bold]", f"[cyan]{from_image}[/cyan] (só Hunyuan3D)")
    else:
        info_table.add_row("[bold]Prompt[/bold]", f"[cyan]{prompt}[/cyan]")
        info_table.add_row("[bold]Imagem intermédia[/bold]", f"{image_width}x{image_height}")
        t2d_note = "CPU offload" if not t2d_full_gpu else "GPU inteira"
        info_table.add_row(
            "[bold]Text2D[/bold]",
            f"steps={t2d_steps}, guidance={t2d_guidance} ({t2d_note})",
        )
    hy_line = f"steps={steps}, guidance={guidance}"
    if preset:
        hy_line += f" [preset={preset}]"
    info_table.add_row("[bold]Hunyuan3D[/bold]", hy_line)
    info_table.add_row("[bold]Octree / chunks[/bold]", f"{octree_resolution} / {num_chunks}")
    if mc_level != 0.0:
        info_table.add_row("[bold]mc_level[/bold]", str(mc_level))
    rep = "desligado" if no_mesh_repair else f"maior componente + merge"
    if mesh_smooth > 0 and not no_mesh_repair:
        rep += f", smooth={mesh_smooth}"
    info_table.add_row("[bold]Pós-mesh[/bold]", rep)
    info_table.add_row("[bold]Formato[/bold]", output_format.upper())
    info_table.add_row("[bold]Modo[/bold]", "economia VRAM" if low_vram else "normal")
    if texture:
        info_table.add_row(
            "[bold]Textura[/bold]",
            f"Hunyuan3D-Paint ({paint_subfolder}) "
            f"{'GPU' if paint_full_gpu else 'CPU offload'}",
        )
        if materialize:
            info_table.add_row(
                "[bold]Materialize PBR[/bold]",
                "sim"
                + (f" → [cyan]{materialize_output_dir}[/cyan]" if materialize_output_dir else ""),
            )

    console.print(Panel(info_table, title="[bold green]Configuração", border_style="green"))

    try:
        with console.status("[bold yellow]A preparar gerador...", spinner="dots"):
            generator = HunyuanTextTo3DGenerator(
                device="cpu" if cpu else None,
                low_vram_mode=low_vram,
                verbose=verbose,
                hunyuan_subfolder=model_subfolder,
            )

        if output is None:
            ensure_dirs()
            timestamp = int(time.time())
            if from_image:
                stem = Path(from_image).stem[:30]
                safe = "".join(c if c.isalnum() else "_" for c in stem)
            else:
                safe = "".join(c if c.isalnum() else "_" for c in prompt[:30])
            output = DEFAULT_MESH_DIR / f"{safe}_{timestamp}.{output_format}"
        else:
            output = Path(output)
            if texture and output.suffix.lower() in (".ply", ".obj"):
                output = output.with_suffix(".glb")

        ref_for_paint = None
        start_time = time.time()

        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=console,
        ) as progress:
            if from_image:
                task = progress.add_task("[cyan]Hunyuan3D (imagem → mesh)...", total=None)
                result = generator.generate_from_image(
                    from_image,
                    num_inference_steps=steps,
                    guidance_scale=guidance,
                    octree_resolution=octree_resolution,
                    num_chunks=num_chunks,
                    hy_seed=seed,
                    mc_level=mc_level,
                )
                if texture:
                    ref_for_paint = Path(from_image)
            else:
                task = progress.add_task("[cyan]Text2D → Hunyuan3D...", total=None)
                if texture:
                    result, ref_img = generator.generate(
                        prompt=prompt,
                        t2d_width=image_width,
                        t2d_height=image_height,
                        t2d_steps=t2d_steps,
                        t2d_guidance=t2d_guidance,
                        text2d_model_id=text2d_model_id,
                        t2d_seed=seed,
                        num_inference_steps=steps,
                        guidance_scale=guidance,
                        octree_resolution=octree_resolution,
                        num_chunks=num_chunks,
                        hy_seed=seed,
                        mc_level=mc_level,
                        t2d_full_gpu=t2d_full_gpu,
                        return_reference_image=True,
                    )
                    ref_for_paint = ref_img
                else:
                    result = generator.generate(
                        prompt=prompt,
                        t2d_width=image_width,
                        t2d_height=image_height,
                        t2d_steps=t2d_steps,
                        t2d_guidance=t2d_guidance,
                        text2d_model_id=text2d_model_id,
                        t2d_seed=seed,
                        num_inference_steps=steps,
                        guidance_scale=guidance,
                        octree_resolution=octree_resolution,
                        num_chunks=num_chunks,
                        hy_seed=seed,
                        mc_level=mc_level,
                        t2d_full_gpu=t2d_full_gpu,
                    )
            progress.update(task, description="[green]Concluído")

        if not no_mesh_repair:
            from .utils.mesh_repair import repair_mesh

            result = repair_mesh(
                result,
                keep_largest=True,
                merge_vertices=True,
                smooth_iterations=max(0, mesh_smooth),
            )

        if texture:
            if ref_for_paint is None:
                raise click.UsageError("Estado interno: referência para Paint em falta.")
            from .painter import apply_hunyuan_paint

            generator.unload_hunyuan()
            clear_cuda_memory()
            with console.status("[bold yellow]Hunyuan3D-Paint (textura)...", spinner="dots"):
                result = apply_hunyuan_paint(
                    result,
                    ref_for_paint,
                    model_repo=paint_repo,
                    subfolder=paint_subfolder,
                    paint_cpu_offload=not paint_full_gpu,
                    verbose=verbose,
                )
            if materialize:
                from .materialize_pbr import apply_materialize_pbr

                with console.status("[bold yellow]Materialize (mapas PBR)...", spinner="dots"):
                    result = apply_materialize_pbr(
                        result,
                        materialize_bin=materialize_bin,
                        save_sidecar_maps_dir=materialize_output_dir,
                        roughness_from_one_minus_smoothness=not materialize_no_invert,
                        verbose=verbose,
                    )

        from .utils.export import save_mesh

        mesh_path = save_mesh(result, output, format=output_format)
        mp = Path(mesh_path).resolve()
        try:
            sz = format_bytes(mp.stat().st_size)
        except OSError:
            sz = "?"
        console.print(Rule("[bold green]Resultado", style="green"))
        console.print(
            f"[bold green]✓[/bold green] Mesh: [cyan]{mp}[/cyan] [dim]({sz})[/dim]"
        )

        elapsed = time.time() - start_time
        console.print(f"\n[dim]Tempo total: {elapsed:.1f}s[/dim]")
        console.print(f"[bold green]Sucesso.[/bold green]")

    except Exception as e:
        console.print(f"\n[bold red]✗ Erro:[/bold red] {str(e)}")
        if verbose:
            console.print_exception()
        sys.exit(1)


@cli.command("doctor")
def doctor():
    """Verifica ambiente: PyTorch, CUDA, VRAM e extensão Hunyuan3D-Paint (custom_rasterizer)."""
    from .painter import check_paint_rasterizer_available
    from .utils.memory import (
        DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB,
        get_system_info,
        gpu_bytes_in_use,
    )

    console.print(
        Panel.fit(
            "[bold]text3d doctor[/bold] — PyTorch, CUDA, Paint (custom_rasterizer)",
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
        alloc or "[dim](defeito: expandable_segments ao iniciar o CLI)[/dim]",
    )
    table.add_row("PyTorch", info_data.get("torch_version", "N/A"))
    table.add_row("CUDA (torch)", str(info_data.get("cuda_available", False)))
    if info_data.get("cuda_available"):
        table.add_row("CUDA (versão runtime)", info_data.get("cuda_version", "N/A"))
        for i, gpu in enumerate(info_data.get("gpus", [])):
            table.add_row(
                f"GPU {i}",
                f"{gpu['name']} — {format_bytes(gpu['total_memory'])} total, "
                f"{format_bytes(gpu['free_memory'])} livre",
            )
        used = gpu_bytes_in_use(0)
        if used is not None:
            table.add_row(
                "Política GPU exclusiva",
                f"~{used / (1024**2):.0f} MiB em uso agora — "
                f"generate/texture recusam se > {DEFAULT_EXCLUSIVE_GPU_MAX_USED_MIB} MiB "
                f"(ou TEXT3D_ALLOW_SHARED_GPU=1 / --allow-shared-gpu)",
            )

    try:
        check_paint_rasterizer_available()
        table.add_row("Hunyuan3D-Paint", "[green]custom_rasterizer importável[/green]")
    except RuntimeError as e:
        msg = str(e).split("\n")[0][:120]
        table.add_row("Hunyuan3D-Paint", f"[yellow]{msg}…[/yellow]")

    console.print(table)
    console.print(
        Panel(
            "[dim]Perfis: --preset fast | balanced | hq. "
            "Desempenho: o CLI define PYTORCH_CUDA_ALLOC_CONF se estiver vazio. "
            "Paint: ver docs/PAINT_SETUP.md[/dim]",
            border_style="dim",
        )
    )


@cli.command()
def info():
    """Informações do sistema e GPU."""
    from .utils.memory import get_system_info

    console.print(
        Panel.fit(
            "[bold]text3d info[/bold] — GPU, cache e pastas de saída",
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
            table.add_row(f"  └ VRAM total", format_bytes(gpu["total_memory"]))
            table.add_row(f"  └ VRAM livre", format_bytes(gpu["free_memory"]))

    table.add_row("Saída padrão", str(DEFAULT_OUTPUT_DIR.absolute()))
    hf = os.environ.get("HF_HOME")
    table.add_row("HF_HOME (cache Hub)", hf or "[dim]~/.cache/huggingface (defeito)[/dim]")
    console.print(table)

    if info_data.get("cuda_available"):
        total_vram = sum(g["total_memory"] for g in info_data.get("gpus", []))
        if total_vram < 6 * 1024**3:
            console.print(
                Panel(
                    "[yellow]VRAM modesta: os defeitos do CLI já são conservadores "
                    "(ver text3d.defaults). Se der OOM, baixa --octree-resolution / "
                    "--num-chunks ou usa --low-vram (Hunyuan em CPU).[/yellow]",
                    title="Aviso",
                    border_style="yellow",
                )
            )


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
@click.option(
    "--output",
    "-o",
    type=click.Path(),
    help="Ficheiro GLB de saída (textura embutida).",
)
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
    help="Mantém modelos Paint na GPU (VRAM alta).",
)
@click.option(
    "--materialize",
    is_flag=True,
    help="Após Paint, gera mapas PBR (Materialize CLI) e embute no GLB.",
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
@click.option(
    "--materialize-no-invert",
    is_flag=True,
    help="Roughness = smoothness (sem 1−smoothness).",
)
@click.option(
    "-v",
    "--verbose",
    "texture_verbose",
    is_flag=True,
    help="Logs detalhados.",
)
@click.option(
    "--allow-shared-gpu",
    "allow_shared_gpu",
    is_flag=True,
    help="Permite GPU com outros processos (desliga verificação: ~300 MiB máx. já ocupados).",
)
@click.option(
    "--gpu-kill-others/--no-gpu-kill-others",
    "gpu_kill_others",
    default=True,
    help="Termina outros processos GPU (nvidia-smi) antes de inferir; defeito: ligado.",
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
    materialize_output_dir,
    materialize_bin,
    materialize_no_invert,
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

    console.print(
        Panel(
            f"Mesh: [cyan]{mesh_path}[/cyan]\n"
            f"Imagem: [cyan]{image_file}[/cyan]\n"
            f"Saída: [cyan]{output}[/cyan]",
            title="[bold green]Hunyuan3D-Paint",
            border_style="green",
        )
    )
    allow_shared = bool(allow_shared_gpu) or _env_allow_shared_gpu()
    gpu_kill = _gpu_kill_others_effective(bool(gpu_kill_others))
    if gpu_kill:
        console.print(
            Panel(
                "[bold]Terminar processos GPU alvo[/bold] antes do Paint\n"
                "[dim]Desliga com [bold]--no-gpu-kill-others[/bold] ou TEXT3D_GPU_KILL_OTHERS=0[/dim]",
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
        out_p = Path(out).resolve()
        try:
            sz = format_bytes(out_p.stat().st_size)
        except OSError:
            sz = "?"
        console.print(Rule("[bold green]Resultado", style="green"))
        console.print(
            f"[bold green]✓[/bold green] GLB texturizado: [cyan]{out_p}[/cyan] [dim]({sz})[/dim]"
        )
        console.print(f"\n[dim]Tempo: {time.time() - start:.1f}s[/dim]")
    except Exception as e:
        console.print(f"\n[bold red]✗ Erro:[/bold red] {str(e)}")
        if verbose:
            console.print_exception()
        sys.exit(1)


@cli.command("materialize-pbr")
@click.argument(
    "mesh_file",
    type=click.Path(exists=True, dir_okay=False),
)
@click.option(
    "--output",
    "-o",
    type=click.Path(),
    required=True,
    help="GLB de saída (PBR embutido).",
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
)
@click.option(
    "--materialize-no-invert",
    is_flag=True,
    help="Roughness = smoothness (sem 1−smoothness).",
)
@click.option(
    "-v",
    "--verbose",
    "mat_verbose",
    is_flag=True,
)
@click.option(
    "--allow-shared-gpu",
    "allow_shared_gpu",
    is_flag=True,
)
@click.option(
    "--gpu-kill-others/--no-gpu-kill-others",
    "gpu_kill_others",
    default=True,
)
@click.pass_context
def materialize_pbr_cmd(
    ctx,
    mesh_file,
    output,
    materialize_output_dir,
    materialize_bin,
    materialize_no_invert,
    mat_verbose,
    allow_shared_gpu,
    gpu_kill_others,
):
    """Só Materialize PBR: GLB já texturizado (Paint) → mapas + GLB PBR (sem re-correr Paint)."""
    from .materialize_pbr import apply_materialize_pbr
    from .painter import load_mesh_trimesh
    from .utils.export import save_mesh

    verbose = bool(ctx.obj.get("VERBOSE")) or mat_verbose
    mesh_path = Path(mesh_file)
    out_path = Path(output)
    if out_path.suffix.lower() not in (".glb",):
        raise click.UsageError("Saída deve ser .glb")

    console.print(
        Panel(
            f"Entrada: [cyan]{mesh_path}[/cyan]\n"
            f"Saída: [cyan]{out_path}[/cyan]",
            title="[bold green]Materialize PBR",
            border_style="green",
        )
    )
    allow_shared = bool(allow_shared_gpu) or _env_allow_shared_gpu()
    gpu_kill = _gpu_kill_others_effective(bool(gpu_kill_others))
    if gpu_kill:
        console.print(
            Panel(
                "[bold]Terminar processos GPU alvo[/bold] antes do Materialize\n"
                "[dim]Desliga com [bold]--no-gpu-kill-others[/bold] ou TEXT3D_GPU_KILL_OTHERS=0[/dim]",
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

    try:
        start = time.time()
        with console.status("[bold yellow]Materialize (mapas PBR)...", spinner="dots"):
            mesh = load_mesh_trimesh(mesh_path)
            result = apply_materialize_pbr(
                mesh,
                materialize_bin=materialize_bin,
                save_sidecar_maps_dir=materialize_output_dir,
                roughness_from_one_minus_smoothness=not materialize_no_invert,
                verbose=verbose,
            )
            mp = save_mesh(result, out_path, format="glb")
        out_p = Path(mp).resolve()
        try:
            sz = format_bytes(out_p.stat().st_size)
        except OSError:
            sz = "?"
        console.print(Rule("[bold green]Resultado", style="green"))
        console.print(
            f"[bold green]✓[/bold green] GLB PBR: [cyan]{out_p}[/cyan] [dim]({sz})[/dim]"
        )
        console.print(f"\n[dim]Tempo: {time.time() - start:.1f}s[/dim]")
    except Exception as e:
        console.print(f"\n[bold red]✗ Erro:[/bold red] {str(e)}")
        if verbose:
            console.print_exception()
        sys.exit(1)


@cli.command()
@click.argument("input_file", type=click.Path(exists=True))
@click.option("--output", "-o", type=click.Path(), help="Ficheiro de saída")
@click.option("--rotate", "-r", is_flag=True, help="Aplicar rotação de orientação")
def convert(input_file, output, rotate):
    """Converte mesh entre formatos (PLY, OBJ, GLB)."""
    from .utils.export import convert_mesh

    input_path = Path(input_file)
    if output is None:
        output = input_path.with_suffix(".glb")

    try:
        with console.status(f"[yellow]A converter {input_path.suffix} → {Path(output).suffix}..."):
            convert_mesh(input_path, output, rotate=rotate)
        outp = Path(output).resolve()
        try:
            sz = format_bytes(outp.stat().st_size)
        except OSError:
            sz = "?"
        console.print(Rule("[bold green]Concluído", style="green"))
        console.print(f"[bold green]✓[/bold green] [cyan]{outp}[/cyan] [dim]({sz})[/dim]")
    except Exception as e:
        console.print(f"[bold red]✗[/bold red] {e}")
        sys.exit(1)


@cli.command("gpu-processes")
def gpu_processes_cmd() -> None:
    """Lista processos na GPU (via nvidia-smi) — útil quando a verificação de VRAM exclusiva falha."""
    import shutil
    import subprocess

    if not shutil.which("nvidia-smi"):
        console.print(
            "[yellow]Comando [bold]nvidia-smi[/bold] não encontrado no PATH. "
            "Com driver NVIDIA instalado, costuma estar em /usr/bin.[/yellow]"
        )
        sys.exit(1)
    try:
        r = subprocess.run(
            ["nvidia-smi"],
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    except OSError as e:
        raise click.ClickException(f"Falha ao executar nvidia-smi: {e}") from e
    except subprocess.TimeoutExpired:
        raise click.ClickException("nvidia-smi excedeu o tempo limite.") from None

    console.print(
        Panel.fit(
            "[bold]Uso da GPU[/bold] — procura a secção [bold]Processes[/bold] (PID, nome, memória)",
            border_style="cyan",
        )
    )
    console.print(r.stdout, end="")
    if r.stderr:
        console.print(f"[dim]{r.stderr}[/dim]")
    if r.returncode != 0:
        console.print(f"[yellow]nvidia-smi saiu com código {r.returncode}[/yellow]")

    console.print()
    console.print(
        Panel(
            "[bold]Parar um processo[/bold]\n"
            "• Na tabela [bold]Processes[/bold], anota o [bold]PID[/bold] da linha que consome VRAM.\n"
            "• [bold]kill PID[/bold] — pedido amigável; [bold]kill -9 PID[/bold] — forçar se não sair.\n"
            "• Sessões antigas de Python/Text2D/Text3D: [bold]pgrep -af 'text2d|text3d'[/bold] "
            "e [bold]pgrep -af python[/bold] (cuidado a não matar o que precisas).\n"
            "• Godot, browsers (WebGPU), outros modelos IA: fecha a app em vez de kill se possível.\n"
            "[dim]Em [bold]text3d generate/texture[/bold], por defeito [bold]--gpu-kill-others[/bold] "
            "termina processos listados aqui (exceto display). Desliga com [bold]--no-gpu-kill-others[/bold].\n"
            "Se a VRAM continua alta sem processos na lista, reiniciar o PC limpa o driver; "
            "ou [bold]TEXT3D_ALLOW_SHARED_GPU=1[/bold] só se aceitares OOM.[/dim]",
            border_style="dim",
            title="Dica",
        )
    )


@cli.command()
def models():
    """Modelos usados pelo Text3D."""
    table = Table(title="[bold blue]Modelos", box=box.ROUNDED)
    table.add_column("Componente", style="cyan")
    table.add_column("Descrição", style="magenta")
    table.add_column("Notas", style="dim")

    table.add_row(
        "Text2D",
        "FLUX.2 Klein (SDNQ) — texto → imagem",
        "Pacote text2d no monorepo",
    )
    table.add_row(
        "Hunyuan3D-2mini",
        "Image-to-3D (subpasta hunyuan3d-dit-v2-mini)",
        "hy3dgen; licença Tencent Hunyuan Community",
    )
    table.add_row(
        "Hunyuan3D-Paint",
        "Textura multivista (tencent/Hunyuan3D-2, delight + paint)",
        "Comando: text3d texture ou generate --texture",
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
