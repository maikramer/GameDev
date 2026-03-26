#!/usr/bin/env python3
"""
Text2Sound — CLI principal (text-to-audio).
"""

import os
import sys
import time
from pathlib import Path
from typing import Optional

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

from .audio_processor import SUPPORTED_FORMATS, save_audio
from .generator import (
    DEFAULT_CFG_SCALE,
    DEFAULT_DURATION,
    DEFAULT_SAMPLER,
    DEFAULT_SIGMA_MAX,
    DEFAULT_SIGMA_MIN,
    DEFAULT_STEPS,
    AudioGenerator,
)
from .presets import AUDIO_PRESETS, get_preset, list_presets
from .utils import format_bytes, format_duration, generate_output_path

console = Console()

DEFAULT_OUTPUT_DIR = Path("outputs")
DEFAULT_AUDIO_DIR = DEFAULT_OUTPUT_DIR / "audio"


def ensure_dirs() -> None:
    DEFAULT_AUDIO_DIR.mkdir(parents=True, exist_ok=True)


@click.group()
@click.version_option(version="0.1.0", prog_name="text2sound")
@click.option("--verbose", "-v", is_flag=True, help="Logs detalhados")
@click.pass_context
def cli(ctx: click.Context, verbose: bool) -> None:
    """Text2Sound — text-to-audio · Stable Audio Open 1.0 (estéreo 44.1 kHz)."""
    ctx.ensure_object(dict)
    ctx.obj["VERBOSE"] = verbose


@cli.group("skill")
def skill_group() -> None:
    """Agent Skills Cursor (instalação no projeto do jogo)."""


@skill_group.command("install")
@click.option(
    "--target",
    "-t",
    type=click.Path(file_okay=False, writable=True, path_type=Path),
    default=".",
    help="Raiz do projeto do jogo (cria .cursor/skills/text2sound/)",
)
@click.option("--force", is_flag=True, help="Sobrescrever SKILL.md existente")
def skill_install_cmd(target: Path, force: bool) -> None:
    """Copia SKILL.md para .cursor/skills/text2sound/."""
    try:
        from gamedev_shared.skill_install import install_agent_skill

        skill_dir = Path(__file__).parent / "cursor_skill"
        dest = install_agent_skill(
            target,
            tool_name="text2sound",
            skill_source=skill_dir,
            force=force,
        )
    except ImportError:
        raise click.ClickException(
            "gamedev-shared não encontrado — instale com pip install -e ../Shared"
        ) from None
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


@cli.command("generate")
@click.argument("prompt")
@click.option("--output", "-o", type=click.Path(), help="Ficheiro de saída")
@click.option(
    "--duration",
    "-d",
    default=DEFAULT_DURATION,
    show_default=True,
    type=click.FloatRange(0.5, 47),
    help="Duração em segundos",
)
@click.option(
    "--steps",
    "-s",
    default=DEFAULT_STEPS,
    show_default=True,
    type=click.IntRange(10, 150),
    help="Passos de difusão",
)
@click.option(
    "--cfg-scale",
    "-c",
    default=DEFAULT_CFG_SCALE,
    show_default=True,
    type=click.FloatRange(1.0, 15.0),
    help="Guidance scale (CFG)",
)
@click.option("--seed", type=int, default=None, help="Seed (None = aleatório)")
@click.option(
    "--format",
    "-f",
    "fmt",
    default="wav",
    show_default=True,
    type=click.Choice(list(SUPPORTED_FORMATS), case_sensitive=False),
    help="Formato de saída",
)
@click.option(
    "--preset",
    "-p",
    default=None,
    type=click.Choice(["None"] + list_presets(), case_sensitive=False),
    help="Preset de áudio",
)
@click.option(
    "--sigma-min",
    default=DEFAULT_SIGMA_MIN,
    show_default=True,
    type=float,
    help="Sigma mínimo (noise schedule)",
)
@click.option(
    "--sigma-max",
    default=DEFAULT_SIGMA_MAX,
    show_default=True,
    type=float,
    help="Sigma máximo (noise schedule)",
)
@click.option(
    "--sampler",
    default=DEFAULT_SAMPLER,
    show_default=True,
    help="Tipo de sampler",
)
@click.option(
    "--trim/--no-trim",
    default=True,
    show_default=True,
    help="Remover silêncio trailing",
)
@click.option(
    "--model",
    "-m",
    "model_id",
    default=None,
    help="ID do modelo HF (default: stable-audio-open-1.0)",
)
@click.option(
    "--half/--no-half",
    "half_precision",
    default=None,
    help="Float16 (auto: ativado em GPUs <= 8 GB VRAM)",
)
@click.option(
    "--verbose",
    "-v",
    "verbose_flag",
    is_flag=True,
    help="Logs detalhados",
)
@click.pass_context
def generate_cmd(
    ctx: click.Context,
    prompt: str,
    output: Optional[str],
    duration: float,
    steps: int,
    cfg_scale: float,
    seed: Optional[int],
    fmt: str,
    preset: Optional[str],
    sigma_min: float,
    sigma_max: float,
    sampler: str,
    trim: bool,
    model_id: Optional[str],
    half_precision: Optional[bool],
    verbose_flag: bool,
) -> None:
    """Gera áudio a partir do PROMPT de texto."""
    verbose = bool(ctx.obj.get("VERBOSE")) or verbose_flag

    if preset and preset != "None":
        try:
            preset_data = get_preset(preset)
        except KeyError as e:
            raise click.ClickException(str(e)) from e
        prompt = f"{prompt}, {preset_data['prompt']}" if prompt.strip() else preset_data["prompt"]
        duration = preset_data.get("duration", duration)
        steps = preset_data.get("steps", steps)
        cfg_scale = preset_data.get("cfg_scale", cfg_scale)

    table = Table(show_header=False, box=box.SIMPLE)
    table.add_row("[bold]Prompt[/bold]", f"[cyan]{prompt}[/cyan]")
    table.add_row("[bold]Duração[/bold]", f"{duration}s ({format_duration(duration)})")
    table.add_row("[bold]Passos[/bold]", str(steps))
    table.add_row("[bold]CFG Scale[/bold]", str(cfg_scale))
    table.add_row("[bold]Formato[/bold]", fmt.upper())
    table.add_row("[bold]Sampler[/bold]", sampler)
    if seed is not None:
        table.add_row("[bold]Seed[/bold]", str(seed))
    if preset and preset != "None":
        table.add_row("[bold]Preset[/bold]", preset)
    console.print(
        Panel(table, title="[bold green]Configuração", border_style="green")
    )

    try:
        gen = AudioGenerator.get_instance(
            model_id=model_id or "stabilityai/stable-audio-open-1.0",
            half_precision=half_precision,
        )

        if output is None:
            ensure_dirs()
            out_path = generate_output_path(prompt, DEFAULT_AUDIO_DIR, fmt)
        else:
            out_path = Path(output)

        start = time.time()
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=console,
        ) as progress:
            task = progress.add_task("[cyan]Carregando modelo...", total=None)
            gen.load()
            progress.update(task, description="[cyan]Gerando áudio...")

            result = gen.generate(
                prompt=prompt,
                duration=duration,
                steps=steps,
                cfg_scale=cfg_scale,
                seed=seed,
                sigma_min=sigma_min,
                sigma_max=sigma_max,
                sampler_type=sampler,
            )

            progress.update(task, description="[cyan]Processando e gravando...")

            metadata = {
                "prompt": prompt,
                "duration": duration,
                "steps": steps,
                "cfg_scale": cfg_scale,
                "seed": seed,
                "sampler": sampler,
                "format": fmt,
                "sample_rate": result.sample_rate,
                "device": result.device,
            }
            if preset and preset != "None":
                metadata["preset"] = preset

            saved = save_audio(
                audio=result.audio,
                sample_rate=result.sample_rate,
                output_path=out_path,
                fmt=fmt,
                trim=trim,
                metadata=metadata,
            )

            progress.update(task, description="[green]Concluído")

        elapsed = time.time() - start
        try:
            sz = format_bytes(saved.stat().st_size)
        except OSError:
            sz = "?"

        console.print(Rule("[bold green]Resultado", style="green"))
        console.print(
            f"[bold green]\u2713[/bold green] Áudio: [cyan]{saved.resolve()}[/cyan] "
            f"[dim]({sz})[/dim]"
        )
        console.print(
            f"[dim]Sample rate: {result.sample_rate} Hz · "
            f"Duração: {format_duration(duration)} · "
            f"Seed: {seed or 'aleatório'}[/dim]"
        )
        console.print(f"[dim]Tempo: {elapsed:.1f}s[/dim]")

    except Exception as e:
        console.print(f"\n[bold red]\u2717 Erro:[/bold red] {e}")
        if verbose:
            console.print_exception()
        sys.exit(1)


@cli.command("batch")
@click.argument("file", type=click.Path(exists=True, path_type=Path))
@click.option("--output-dir", "-d", type=click.Path(path_type=Path), default=None)
@click.option("--preset", "-p", default=None, help="Preset aplicado a todos os prompts")
@click.option(
    "--duration",
    default=DEFAULT_DURATION,
    type=click.FloatRange(0.5, 47),
)
@click.option("--steps", "-s", default=DEFAULT_STEPS, type=click.IntRange(10, 150))
@click.option("--cfg-scale", "-c", default=DEFAULT_CFG_SCALE, type=float)
@click.option("--format", "-f", "fmt", default="wav", type=click.Choice(list(SUPPORTED_FORMATS)))
@click.option("--trim/--no-trim", default=True)
@click.option("--model", "-m", "model_id", default=None)
@click.pass_context
def batch_cmd(
    ctx: click.Context,
    file: Path,
    output_dir: Optional[Path],
    preset: Optional[str],
    duration: float,
    steps: int,
    cfg_scale: float,
    fmt: str,
    trim: bool,
    model_id: Optional[str],
) -> None:
    """Gera áudios em batch a partir de um ficheiro de prompts (um por linha)."""
    prompts = [
        line.strip()
        for line in file.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.startswith("#")
    ]

    if not prompts:
        raise click.ClickException("Ficheiro sem prompts válidos.")

    if preset and preset != "None":
        try:
            preset_data = get_preset(preset)
        except KeyError as e:
            raise click.ClickException(str(e)) from e
        duration = preset_data.get("duration", duration)
        steps = preset_data.get("steps", steps)
        cfg_scale = preset_data.get("cfg_scale", cfg_scale)

    console.print(f"[bold]Batch:[/bold] {len(prompts)} prompts de [cyan]{file}[/cyan]")

    out = output_dir or DEFAULT_AUDIO_DIR
    out.mkdir(parents=True, exist_ok=True)

    gen = AudioGenerator.get_instance(
        model_id=model_id or "stabilityai/stable-audio-open-1.0",
    )
    gen.load()

    ok_count = 0
    for idx, prompt_text in enumerate(prompts):
        if preset and preset != "None":
            full_prompt = f"{prompt_text}, {preset_data['prompt']}"
        else:
            full_prompt = prompt_text

        try:
            result = gen.generate(
                prompt=full_prompt,
                duration=duration,
                steps=steps,
                cfg_scale=cfg_scale,
            )

            out_path = generate_output_path(prompt_text, out, fmt)
            metadata = {
                "prompt": full_prompt,
                "duration": duration,
                "steps": steps,
                "cfg_scale": cfg_scale,
                "format": fmt,
                "sample_rate": result.sample_rate,
                "batch_index": idx,
            }
            if preset and preset != "None":
                metadata["preset"] = preset

            saved = save_audio(
                audio=result.audio,
                sample_rate=result.sample_rate,
                output_path=out_path,
                fmt=fmt,
                trim=trim,
                metadata=metadata,
            )
            ok_count += 1
            console.print(
                f"  [green]\u2713[/green] {idx + 1}/{len(prompts)}: [cyan]{saved.name}[/cyan]"
            )
        except Exception as e:
            console.print(
                f"  [red]\u2717[/red] {idx + 1}/{len(prompts)}: {e}"
            )

    console.print(
        Panel(
            f"[bold]{ok_count}/{len(prompts)}[/bold] áudios gerados em [cyan]{out.resolve()}[/cyan]",
            title="[bold green]Batch concluído",
            border_style="green",
        )
    )


@cli.command("presets")
def presets_cmd() -> None:
    """Lista presets de áudio disponíveis."""
    t = Table(title="[bold blue]Presets de Áudio (Game Dev)", box=box.ROUNDED)
    t.add_column("Nome", style="cyan", no_wrap=True)
    t.add_column("Prompt", style="white", max_width=55)
    t.add_column("Duração", style="green", justify="right")
    t.add_column("Steps", style="green", justify="right")
    t.add_column("CFG", style="green", justify="right")

    for name in list_presets():
        p = AUDIO_PRESETS[name]
        prompt_text = p["prompt"]
        if len(prompt_text) > 55:
            prompt_text = prompt_text[:52] + "..."
        t.add_row(
            name,
            prompt_text,
            f"{p['duration']}s",
            str(p["steps"]),
            str(p["cfg_scale"]),
        )
    console.print(t)


@cli.command("info")
def info_cmd() -> None:
    """Informações de configuração, ambiente e GPU."""
    console.print(
        Panel.fit(
            "[bold]text2sound info[/bold] — configuração e ambiente",
            border_style="blue",
        )
    )

    t = Table(title="[bold blue]Configuração", box=box.ROUNDED)
    t.add_column("Item", style="cyan", no_wrap=True)
    t.add_column("Valor", style="green")

    t.add_row("Modelo (default)", "stabilityai/stable-audio-open-1.0")
    t.add_row("Sample rate", "44100 Hz")
    t.add_row("Canais", "Estéreo (2)")

    token = os.environ.get("HF_TOKEN") or os.environ.get("HUGGINGFACEHUB_API_TOKEN")
    t.add_row("HF Token", "[green]configurado[/green]" if token else "[red]não definido[/red]")
    t.add_row(
        "HF_HOME (cache Hub)",
        os.environ.get("HF_HOME") or "[dim]~/.cache/huggingface (defeito)[/dim]",
    )
    t.add_row("Saída padrão", str(DEFAULT_AUDIO_DIR.resolve()))
    t.add_row("Presets disponíveis", str(len(AUDIO_PRESETS)))
    t.add_row("Python", sys.version.split()[0])

    try:
        import torch
        t.add_row("PyTorch", torch.__version__)
        t.add_row(
            "CUDA",
            f"{torch.version.cuda} (GPU: {torch.cuda.get_device_name(0)})"
            if torch.cuda.is_available()
            else "[yellow]não disponível (CPU)[/yellow]",
        )

        if torch.cuda.is_available():
            try:
                from gamedev_shared.gpu import get_gpu_info
                gpus = get_gpu_info()
                for gpu in gpus:
                    t.add_row(
                        f"GPU {gpu.get('index', '?')}",
                        f"{gpu.get('name', '?')} — "
                        f"{gpu.get('memory_free_str', '?')} livres / "
                        f"{gpu.get('memory_total_str', '?')} total",
                    )
            except (ImportError, Exception):
                pass
    except ImportError:
        t.add_row("PyTorch", "[red]não instalado[/red]")

    console.print(t)


def main() -> None:
    try:
        cli()
    except KeyboardInterrupt:
        console.print("\n[yellow]Cancelado.[/yellow]")
        sys.exit(130)


if __name__ == "__main__":
    main()
