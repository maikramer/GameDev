#!/usr/bin/env python3
"""GameAssets — CLI principal."""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
import zlib
from pathlib import Path
from typing import Any

from . import cli_rich  # noqa: F401 — configura rich-click antes dos comandos

if cli_rich.RICH_CLICK:
    import rich_click as click
else:
    import click
from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.progress import BarColumn, Progress, SpinnerColumn, TextColumn, TimeElapsedColumn
from rich.table import Table

from . import __version__
from .batch_guard import batch_directory_lock, query_gpu_free_mib, subprocess_gpu_env
from .manifest import ManifestRow, load_manifest
from .presets import get_preset, load_presets_bundle
from .profile import GameProfile, load_profile
from .prompt_builder import build_prompt
from .runner import merge_subprocess_output, resolve_binary, run_cmd
from .templates import GAME_YAML, MANIFEST_CSV
from .cursor_skill_install import install_agent_skill

console = Console()

EPILOG = """
Exemplo rápido:
  gameassets init
  gameassets prompts --profile game.yaml --manifest manifest.csv
  gameassets batch --profile game.yaml --manifest manifest.csv --with-3d

Preset só num ficheiro teu (ex.: galaxy_orbital em presets-local.yaml):
  gameassets batch --profile game.yaml --manifest manifest.csv --with-3d \\
    --presets-local presets-local.yaml --log run.jsonl

Define TEXT2D_BIN / TEXT3D_BIN (e MATERIALIZE_BIN se usares text3d.materialize) se não estiverem no PATH.
"""


def _seed_for_row(profile: GameProfile, row_id: str) -> int | None:
    if profile.seed_base is None:
        return None
    h = zlib.adler32(row_id.encode("utf-8")) & 0x7FFFFFFF
    return profile.seed_base + h


def _safe_row_dirname(row_id: str) -> str:
    """Parte do id do manifest segura para nome de pasta (ex.: Props/crate → Props__crate_01)."""
    return row_id.replace("/", "__").replace("\\", "_")


def _materialize_maps_path(profile: GameProfile, row: ManifestRow) -> Path:
    t3 = profile.text3d
    sub = (t3.materialize_maps_subdir if t3 else None) or "pbr_maps"
    return Path(profile.output_dir) / sub / _safe_row_dirname(row.id)


def _materialize_maps_path_manifest(
    profile: GameProfile,
    manifest_dir: Path,
    row: ManifestRow,
) -> Path:
    """Destino dos mapas PBR com output_dir relativo resolvido face à pasta do manifest."""
    rel = _materialize_maps_path(profile, row)
    if rel.is_absolute():
        return rel.resolve()
    return (manifest_dir / rel).resolve()


def _paths_for_row_manifest(
    profile: GameProfile,
    manifest_dir: Path,
    row: ManifestRow,
) -> tuple[Path, Path]:
    """
    PNG/GLB absolutos. O perfil usa muitas vezes output_dir: '.' — sem isto, caminhos relativos
    dependem do CWD do processo e o Text3D pode ler ficheiros errados (GPU “parada”).
    """
    img, mesh = _paths_for_row(profile, row)
    if not img.is_absolute():
        img = (manifest_dir / img).resolve()
    else:
        img = img.resolve()
    if not mesh.is_absolute():
        mesh = (manifest_dir / mesh).resolve()
    else:
        mesh = mesh.resolve()
    return img, mesh


def _path_for_log(path: Path, manifest_dir: Path) -> str:
    """Caminho para run.jsonl: relativo ao manifest quando possível."""
    try:
        return str(path.resolve().relative_to(manifest_dir.resolve()))
    except ValueError:
        return str(path.resolve())


def _install_file(src: Path, dst: Path) -> None:
    """Copia ficheiro para destino final (pasta do jogo); cria pais se necessário."""
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)


def _install_maps_dir(src: Path, dst: Path) -> None:
    """Copia mapas PBR de uma pasta de trabalho (tmp) para a pasta final no jogo."""
    if not src.is_dir():
        return
    dst.mkdir(parents=True, exist_ok=True)
    for p in src.iterdir():
        if p.is_file():
            shutil.copy2(p, dst / p.name)
        elif p.is_dir():
            _install_maps_dir(p, dst / p.name)


def _append_text2d_profile_args(profile: GameProfile, argv: list[str]) -> None:
    """Extensões do perfil (resolução, VRAM) para `text2d generate`."""
    t2 = profile.text2d
    if not t2:
        return
    if t2.width is not None:
        argv.extend(["-W", str(t2.width)])
    if t2.height is not None:
        argv.extend(["-H", str(t2.height)])
    if t2.low_vram:
        argv.append("--low-vram")
    if t2.cpu:
        argv.append("--cpu")


def _paths_for_row(profile: GameProfile, row: ManifestRow) -> tuple[Path, Path]:
    root = Path(profile.output_dir)
    ext = profile.image_ext
    rid = row.id
    if profile.path_layout == "flat":
        parts = rid.split("/")
        if len(parts) >= 2:
            sub = Path(*parts[:-1])
            base = parts[-1]
            dir_ = root / sub
        else:
            dir_ = root
            base = rid
        img = dir_ / f"{base}.{ext}"
        mesh = dir_ / f"{base}.glb"
    else:
        img = root / profile.images_subdir / f"{rid}.{ext}"
        mesh = root / profile.meshes_subdir / f"{rid}.glb"
    return img, mesh


def _build_context(
    profile_path: Path,
    manifest_path: Path,
    presets_local: Path | None,
) -> tuple[GameProfile, list[ManifestRow], dict[str, Any], dict[str, Any]]:
    profile = load_profile(profile_path)
    rows = load_manifest(manifest_path)
    bundle = load_presets_bundle(presets_local)
    preset = get_preset(bundle, profile.style_preset)
    return profile, rows, bundle, preset


@click.group(epilog=EPILOG)
@click.version_option(version=__version__, prog_name="gameassets")
def main() -> None:
    """Batch de prompts e assets alinhados ao estilo do teu jogo."""


@main.group("skill")
def skill_group() -> None:
    """Agent Skills Cursor (instalação no projeto do jogo)."""


@skill_group.command("install")
@click.option(
    "--target",
    "-t",
    type=click.Path(file_okay=False, writable=True, path_type=Path),
    default=".",
    help="Raiz do projeto do jogo (cria .cursor/skills/gameassets/)",
)
@click.option("--force", is_flag=True, help="Sobrescrever SKILL.md existente")
def skill_install_cmd(target: Path, force: bool) -> None:
    """Copia SKILL.md para .cursor/skills/gameassets/."""
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


@main.command("init")
@click.option(
    "--path",
    "target_dir",
    type=click.Path(file_okay=False, writable=True, path_type=Path),
    default=".",
    help="Diretório onde criar game.yaml e manifest.csv",
)
@click.option("--force", is_flag=True, help="Sobrescrever ficheiros existentes")
def init_cmd(target_dir: Path, force: bool) -> None:
    """Cria game.yaml e manifest.csv de exemplo."""
    target_dir = target_dir.resolve()
    target_dir.mkdir(parents=True, exist_ok=True)
    gy = target_dir / "game.yaml"
    mc = target_dir / "manifest.csv"
    if gy.exists() and not force:
        raise click.ClickException(f"Já existe {gy} (usa --force para sobrescrever)")
    if mc.exists() and not force:
        raise click.ClickException(f"Já existe {mc} (usa --force para sobrescrever)")
    gy.write_text(GAME_YAML, encoding="utf-8")
    mc.write_text(MANIFEST_CSV, encoding="utf-8")
    console.print(
        Panel(
            f"Criados [bold cyan]{gy}[/bold cyan] e [bold cyan]{mc}[/bold cyan].\n\n"
            "Seguinte: edita o perfil, preenche o manifest, depois "
            "[bold]gameassets prompts[/bold] ou [bold]gameassets batch[/bold].",
            title="[bold green]init[/bold green]",
            border_style="green",
        )
    )


@main.command("prompts")
@click.option(
    "--profile",
    "profile_path",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default="game.yaml",
    help="Ficheiro de perfil YAML",
)
@click.option(
    "--manifest",
    "manifest_path",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default="manifest.csv",
    help="CSV com id, idea e colunas opcionais",
)
@click.option(
    "--presets-local",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default=None,
    help="YAML opcional para sobrescrever/estender presets",
)
@click.option(
    "--output",
    "-o",
    type=click.Path(dir_okay=False, path_type=Path),
    default=None,
    help="Se definido, grava prompts em JSONL (uma linha por id)",
)
def prompts_cmd(
    profile_path: Path,
    manifest_path: Path,
    presets_local: Path | None,
    output: Path | None,
) -> None:
    """Mostra (ou grava) os prompts finais sem usar GPU."""
    profile, rows, _bundle, preset = _build_context(
        profile_path, manifest_path, presets_local
    )
    entries: list[dict[str, Any]] = []
    for row in rows:
        prompt_2d = build_prompt(profile, preset, row, for_3d=False)
        prompt_3d_hint = build_prompt(profile, preset, row, for_3d=True)
        entries.append(
            {
                "id": row.id,
                "prompt": prompt_2d,
                "prompt_3d_hint": prompt_3d_hint,
                "generate_3d": row.generate_3d,
            }
        )
    if output:
        with output.open("w", encoding="utf-8") as f:
            for e in entries:
                f.write(json.dumps(e, ensure_ascii=False) + "\n")
        console.print(
            Panel(
                f"[green]Gravado[/green] [bold]{output}[/bold] — {len(entries)} linha(s).",
                title="prompts",
                border_style="green",
            )
        )
        return
    table = Table(
        title="[bold]Prompts[/bold] (pré-visualização)",
        box=box.ROUNDED,
        show_header=True,
        header_style="bold magenta",
    )
    table.add_column("id", style="cyan", no_wrap=True)
    table.add_column("3D?", justify="center")
    table.add_column("prompt (início)", overflow="ellipsis", max_width=72)
    for e in entries:
        p = e["prompt"]
        preview = p if len(p) <= 72 else p[:69] + "..."
        flag = "sim" if e["generate_3d"] else "não"
        table.add_row(e["id"], flag, preview)
    console.print(table)


@main.command("batch")
@click.option(
    "--profile",
    "profile_path",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default="game.yaml",
)
@click.option(
    "--manifest",
    "manifest_path",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default="manifest.csv",
)
@click.option(
    "--presets-local",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default=None,
)
@click.option(
    "--with-3d",
    "with_3d",
    is_flag=True,
    help="Gera GLB quando generate_3d=true no manifest",
)
@click.option(
    "--dry-run",
    is_flag=True,
    help="Mostra comandos sem executar",
)
@click.option(
    "--fail-fast",
    is_flag=True,
    help="Parar no primeiro erro (defeito: continuar)",
)
@click.option(
    "--log",
    "log_path",
    type=click.Path(dir_okay=False, path_type=Path),
    default=None,
    help="Ficheiro JSONL com um registo por asset",
)
@click.option(
    "--skip-batch-lock",
    is_flag=True,
    help="Permite dois batches na mesma pasta (não recomendado: disputa de VRAM / OOM).",
)
@click.option(
    "--skip-gpu-preflight",
    is_flag=True,
    help="Não avisar quando a VRAM livre (nvidia-smi) estiver baixa.",
)
@click.option(
    "--skip-text2d",
    "skip_text2d",
    is_flag=True,
    help="Não executar Text2D: usa os PNG já em output_dir (exige --with-3d; valida PNG por linha com generate_3d).",
)
def batch_cmd(
    profile_path: Path,
    manifest_path: Path,
    presets_local: Path | None,
    with_3d: bool,
    dry_run: bool,
    fail_fast: bool,
    log_path: Path | None,
    skip_batch_lock: bool,
    skip_gpu_preflight: bool,
    skip_text2d: bool,
) -> None:
    """Gera imagens (e opcionalmente meshes) para cada linha do manifest."""
    profile, rows, _bundle, preset = _build_context(
        profile_path, manifest_path, presets_local
    )

    if skip_text2d and not with_3d:
        raise click.ClickException(
            "--skip-text2d só é válido com --with-3d (PNGs já existem; só corre Text3D)."
        )

    if not with_3d and any(r.generate_3d for r in rows):
        console.print(
            Panel(
                "[yellow]Há linhas com generate_3d=true mas 3D está desligado.[/yellow]\n"
                "Usa [bold]--with-3d[/bold] para gerar meshes.",
                title="Aviso",
                border_style="yellow",
            )
        )

    text2d_bin: str | None = None
    if not skip_text2d:
        try:
            text2d_bin = resolve_binary("TEXT2D_BIN", "text2d")
        except FileNotFoundError as e:
            raise click.ClickException(str(e)) from e
    text3d_bin: str | None = None
    if with_3d:
        try:
            text3d_bin = resolve_binary("TEXT3D_BIN", "text3d")
        except FileNotFoundError as e:
            raise click.ClickException(str(e)) from e

    meta = Table(show_header=False, box=box.SIMPLE, title="[bold]Batch[/bold]")
    meta.add_row("Perfil", str(profile_path.resolve()))
    meta.add_row("Manifest", str(manifest_path.resolve()))
    meta.add_row("Linhas", str(len(rows)))
    meta.add_row(
        "text2d",
        "[dim]omitido[/dim] (PNG existentes)" if skip_text2d else (text2d_bin or ""),
    )
    meta.add_row("text3d", text3d_bin or "[dim](desligado)[/dim]")
    meta.add_row("Modo", "[cyan]dry-run[/cyan]" if dry_run else "execução")
    if skip_text2d:
        meta.add_row(
            "Ordem",
            "Text2D omitido → Text3D (só generate_3d, PNG no output_dir)",
        )
    else:
        meta.add_row(
            "Ordem",
            "Text2D (todas as linhas) → Text3D (só generate_3d, imagens já gravadas)",
        )
    meta.add_row(
        "Lock",
        "[dim]desligado[/dim]" if (dry_run or skip_batch_lock) else f"{manifest_path.parent / '.gameassets_batch.lock'}",
    )
    console.print(Panel(meta, border_style="cyan", title="[bold]Plano[/bold]"))

    manifest_dir = manifest_path.parent.resolve()

    continue_on_error = not fail_fast
    failures = 0
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)

    def append_log(rec: dict[str, Any]) -> None:
        if log_path is None:
            return
        with log_path.open("a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")

    if dry_run:
        if not skip_text2d:
            console.print("[dim]--- Fase 1: Text2D (todas as linhas) ---[/dim]")
            for row in rows:
                prompt = build_prompt(profile, preset, row, for_3d=False)
                img_path, mesh_path = _paths_for_row_manifest(
                    profile, manifest_dir, row
                )
                seed = _seed_for_row(profile, row.id)
                t2d_args = [
                    text2d_bin or "",
                    "generate",
                    prompt,
                    "-o",
                    str(img_path),
                ]
                if seed is not None:
                    t2d_args.extend(["--seed", str(seed)])
                _append_text2d_profile_args(profile, t2d_args)
                console.print(f"[dim]{' '.join(t2d_args)}[/dim]")
        else:
            console.print(
                "[dim]--- Text2D omitido (--skip-text2d); comandos só para Text3D ---[/dim]"
            )
        if with_3d and text3d_bin and any(r.generate_3d for r in rows):
            t3d = profile.text3d
            phased = bool(t3d and t3d.phased_batch and t3d.texture)
            if phased:
                console.print(
                    "[dim]--- Text3D em fases (phased_batch): shape → Paint → materialize-pbr ---[/dim]"
                )
                for row in rows:
                    if not row.generate_3d:
                        continue
                    img_path, mesh_path = _paths_for_row_manifest(
                        profile, manifest_dir, row
                    )
                    seed = _seed_for_row(profile, row.id)
                    tw = "<tmp>/shape.glb"
                    a1 = _text3d_argv(
                        text3d_bin,
                        profile,
                        img_path,
                        Path(tw),
                        row,
                        shape_only=True,
                    )
                    if seed is not None:
                        a1 = [*a1, "--seed", str(seed)]
                    console.print(f"[dim]{' '.join(a1)}[/dim]")
                    out_paint = (
                        str(mesh_path)
                        if not (t3d and t3d.materialize)
                        else "<tmp>/painted.glb"
                    )
                    a2 = _text3d_texture_argv(
                        text3d_bin,
                        profile,
                        Path(tw),
                        img_path,
                        Path(out_paint),
                        with_materialize=False,
                        materialize_maps_dir=None,
                        row=row,
                    )
                    console.print(f"[dim]{' '.join(a2)}[/dim]")
                    if t3d and t3d.materialize:
                        maps_ph = _materialize_maps_path(profile, row)
                        a3 = _text3d_materialize_pbr_argv(
                            text3d_bin,
                            profile,
                            Path("<tmp>/painted.glb"),
                            mesh_path,
                            materialize_maps_dir=maps_ph,
                            row=row,
                        )
                        console.print(f"[dim]{' '.join(a3)}[/dim]")
            else:
                console.print("[dim]--- Fase 2: Text3D (generate_3d=true) ---[/dim]")
                for row in rows:
                    if not row.generate_3d:
                        continue
                    img_path, mesh_path = _paths_for_row_manifest(
                        profile, manifest_dir, row
                    )
                    seed = _seed_for_row(profile, row.id)
                    t3d_args = _text3d_argv(
                        text3d_bin, profile, img_path, mesh_path, row
                    )
                    if seed is not None:
                        t3d_args.extend(["--seed", str(seed)])
                    console.print(f"[dim]{' '.join(t3d_args)}[/dim]")
        console.print(
            Panel("[green]dry-run concluído[/green]", border_style="green", title="Batch")
        )
        return

    if not rows:
        console.print("[yellow]Manifest sem linhas.[/yellow]")
        return

    if not skip_gpu_preflight:
        free_mib = query_gpu_free_mib()
        if free_mib is not None and free_mib < 1800:
            console.print(
                Panel(
                    f"[yellow]VRAM livre na GPU 0: ~{free_mib} MiB[/yellow] (recomendável ≥2 GiB "
                    "para Text2D/Text3D sem OOM). Fecha outro [bold]gameassets batch[/bold], o "
                    "[bold]Godot[/bold], ou [bold]text3d[/bold] órfão; ou activa "
                    "[bold]text2d.cpu: true[/bold] no perfil. "
                    "O batch define [dim]PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True[/dim] "
                    "se ainda não estiver no ambiente.",
                    title="Aviso GPU",
                    border_style="yellow",
                )
            )

    child_env = subprocess_gpu_env()

    with batch_directory_lock(manifest_path, skip=skip_batch_lock):
        batch_tmp = Path(tempfile.mkdtemp(prefix="gameassets_"))
        try:
            with Progress(
                SpinnerColumn(),
                TextColumn("[progress.description]{task.description}"),
                BarColumn(bar_width=None),
                TextColumn("[progress.percentage]{task.percentage:>3.0f}%"),
                TimeElapsedColumn(),
                console=console,
            ) as progress:
                task1 = progress.add_task(
                    "[cyan]Fase 1: PNGs existentes[/cyan]"
                    if skip_text2d
                    else "[cyan]Fase 1: Text2D[/cyan]",
                    total=len(rows),
                )
                results: list[dict[str, Any]] = []
                pending_3d_indices: list[int] = []

                for idx, row in enumerate(rows):
                    img_final, mesh_final = _paths_for_row_manifest(
                        profile, manifest_dir, row
                    )
                    rec: dict[str, Any] = {
                        "id": row.id,
                        "status": "ok",
                        "image_path": _path_for_log(img_final, manifest_dir),
                        "mesh_path": None,
                        "error": None,
                    }

                    if skip_text2d:
                        progress.update(
                            task1,
                            description=f"[cyan]{row.id}[/cyan] · PNG",
                        )
                        if row.generate_3d and with_3d and not img_final.is_file():
                            failures += 1
                            rec["status"] = "error"
                            rec["error"] = f"PNG em falta (esperado: {img_final})"
                            console.print(
                                f"[red]PNG em falta[/red] {row.id}: {img_final}"
                            )
                            results.append(rec)
                            append_log(rec)
                            if not continue_on_error:
                                raise click.Abort()
                            progress.advance(task1)
                            continue
                        results.append(rec)
                        do_3d = with_3d and row.generate_3d
                        if do_3d and text3d_bin:
                            pending_3d_indices.append(idx)
                        else:
                            append_log(rec)
                        progress.advance(task1)
                        continue

                    progress.update(task1, description=f"[cyan]{row.id}[/cyan] · Text2D")
                    row_work = batch_tmp / f"{idx:04d}_{_safe_row_dirname(row.id)}"
                    row_work.mkdir(parents=True, exist_ok=True)
                    try:
                        prompt = build_prompt(profile, preset, row, for_3d=False)
                        ext = profile.image_ext
                        img_tmp = row_work / f"ref.{ext}"
                        seed = _seed_for_row(profile, row.id)

                        t2d_args = [
                            text2d_bin or "",
                            "generate",
                            prompt,
                            "-o",
                            str(img_tmp),
                        ]
                        if seed is not None:
                            t2d_args.extend(["--seed", str(seed)])
                        _append_text2d_profile_args(profile, t2d_args)

                        r2 = run_cmd(
                            t2d_args, extra_env=child_env, cwd=manifest_dir
                        )
                        if r2.returncode != 0:
                            failures += 1
                            err = merge_subprocess_output(r2) or "text2d falhou"
                            rec["status"] = "error"
                            rec["error"] = err
                            preview = merge_subprocess_output(r2, max_chars=4000) or err
                            console.print(f"[red]text2d falhou[/red] {row.id}: {preview}")
                            results.append(rec)
                            append_log(rec)
                            if not continue_on_error:
                                raise click.Abort()
                            continue

                        if not img_tmp.is_file():
                            failures += 1
                            rec["status"] = "error"
                            rec["error"] = "text2d não produziu ficheiro de imagem"
                            console.print(f"[red]text2d sem saída[/red] {row.id}")
                            results.append(rec)
                            append_log(rec)
                            if not continue_on_error:
                                raise click.Abort()
                            continue

                        _install_file(img_tmp, img_final)

                        results.append(rec)
                        do_3d = with_3d and row.generate_3d
                        if do_3d and text3d_bin:
                            pending_3d_indices.append(idx)
                        else:
                            append_log(rec)
                    finally:
                        shutil.rmtree(row_work, ignore_errors=True)
                        progress.advance(task1)

                if with_3d and text3d_bin and pending_3d_indices:
                    console.print(
                        Panel(
                            "[bold]Fase 2 (Text3D)[/bold]: fecha o Godot e apps que usem a GPU; "
                            "`nvidia-smi` deve mostrar VRAM livre. Em ~6 GB, "
                            "[bold]text3d.low_vram: true[/bold] no [cyan]game.yaml[/cyan] "
                            "evita OOM (malha pode ser mais grosseira). "
                            "Com [bold]phased_batch: true[/bold], o batch corre: shape (todos) → "
                            "Paint (todos) → materialize-pbr (todos), libertando VRAM entre passos.",
                            border_style="yellow",
                            title="Antes do 3D",
                        )
                    )
                    t3_opts = profile.text3d
                    use_phased = bool(t3_opts and t3_opts.phased_batch and t3_opts.texture)

                    def _finalize_mesh_ok(
                        rec: dict[str, Any],
                        mesh_final: Path,
                        maps_tmp: Path | None,
                        row: ManifestRow,
                    ) -> None:
                        if (
                            maps_tmp is not None
                            and t3_opts
                            and t3_opts.materialize_export_maps_to_output
                        ):
                            dst_maps = _materialize_maps_path_manifest(
                                profile, manifest_dir, row
                            )
                            dst_maps.mkdir(parents=True, exist_ok=True)
                            _install_maps_dir(maps_tmp, dst_maps)
                        rec["mesh_path"] = _path_for_log(mesh_final, manifest_dir)

                    if use_phased:
                        task_shape = progress.add_task(
                            "[cyan]Text3D: shape (todos)[/cyan]",
                            total=len(pending_3d_indices),
                        )
                        shape_ok: list[int] = []
                        for idx in pending_3d_indices:
                            row = rows[idx]
                            rec = results[idx]
                            progress.update(
                                task_shape,
                                description=f"[cyan]{row.id}[/cyan] · shape",
                            )
                            row_work = batch_tmp / f"{idx:04d}_{_safe_row_dirname(row.id)}_3d"
                            row_work.mkdir(parents=True, exist_ok=True)
                            try:
                                img_final, mesh_final = _paths_for_row_manifest(
                                    profile, manifest_dir, row
                                )
                                mesh_shape = row_work / "shape.glb"
                                seed = _seed_for_row(profile, row.id)
                                t3d_args = _text3d_argv(
                                    text3d_bin,
                                    profile,
                                    img_final,
                                    mesh_shape,
                                    row,
                                    shape_only=True,
                                )
                                if seed is not None:
                                    t3d_args.extend(["--seed", str(seed)])
                                r3 = run_cmd(
                                    t3d_args, extra_env=child_env, cwd=manifest_dir
                                )
                                if r3.returncode != 0:
                                    failures += 1
                                    err = (
                                        merge_subprocess_output(r3)
                                        or "text3d generate (shape) falhou"
                                    )
                                    rec["status"] = "error"
                                    rec["error"] = err
                                    preview = (
                                        merge_subprocess_output(r3, max_chars=4000) or err
                                    )
                                    console.print(
                                        f"[red]text3d shape falhou[/red] {row.id}: {preview}"
                                    )
                                    append_log(rec)
                                    if not continue_on_error:
                                        raise click.Abort()
                                elif not mesh_shape.is_file():
                                    failures += 1
                                    rec["status"] = "error"
                                    rec["error"] = "text3d não produziu shape.glb"
                                    console.print(
                                        f"[red]text3d sem shape.glb[/red] {row.id}"
                                    )
                                    append_log(rec)
                                    if not continue_on_error:
                                        raise click.Abort()
                                else:
                                    shape_ok.append(idx)
                            finally:
                                if idx not in shape_ok:
                                    shutil.rmtree(row_work, ignore_errors=True)
                                progress.advance(task_shape)

                        task_paint = progress.add_task(
                            "[cyan]Text3D: Paint (todos)[/cyan]", total=len(shape_ok)
                        )
                        texture_ok: list[int] = []
                        for idx in shape_ok:
                            row = rows[idx]
                            rec = results[idx]
                            progress.update(
                                task_paint,
                                description=f"[cyan]{row.id}[/cyan] · Paint",
                            )
                            row_work = batch_tmp / f"{idx:04d}_{_safe_row_dirname(row.id)}_3d"
                            mesh_shape = row_work / "shape.glb"
                            img_final, mesh_final = _paths_for_row_manifest(
                                profile, manifest_dir, row
                            )
                            painted = row_work / "painted.glb"
                            mesh_out = mesh_final if not (t3_opts and t3_opts.materialize) else painted
                            maps_tmp: Path | None = None
                            if t3_opts and t3_opts.materialize and t3_opts.materialize_save_maps:
                                maps_tmp = row_work / "pbr_maps"
                            try:
                                t_tex = _text3d_texture_argv(
                                    text3d_bin,
                                    profile,
                                    mesh_shape,
                                    img_final,
                                    mesh_out,
                                    with_materialize=False,
                                    materialize_maps_dir=maps_tmp,
                                    row=row,
                                )
                                r4 = run_cmd(
                                    t_tex, extra_env=child_env, cwd=manifest_dir
                                )
                                if r4.returncode != 0:
                                    failures += 1
                                    err = (
                                        merge_subprocess_output(r4)
                                        or "text3d texture falhou"
                                    )
                                    rec["status"] = "error"
                                    rec["error"] = err
                                    preview = (
                                        merge_subprocess_output(r4, max_chars=4000) or err
                                    )
                                    console.print(
                                        f"[red]text3d texture falhou[/red] {row.id}: {preview}"
                                    )
                                    append_log(rec)
                                    if not continue_on_error:
                                        raise click.Abort()
                                elif not mesh_out.is_file():
                                    failures += 1
                                    rec["status"] = "error"
                                    rec["error"] = "text3d texture não produziu GLB"
                                    console.print(
                                        f"[red]text3d texture sem GLB[/red] {row.id}"
                                    )
                                    append_log(rec)
                                    if not continue_on_error:
                                        raise click.Abort()
                                else:
                                    if t3_opts and t3_opts.materialize:
                                        texture_ok.append(idx)
                                    else:
                                        append_log(rec)
                                        _finalize_mesh_ok(rec, mesh_final, None, row)
                            finally:
                                keep_for_mat = (
                                    t3_opts
                                    and t3_opts.materialize
                                    and idx in texture_ok
                                )
                                if not keep_for_mat:
                                    shutil.rmtree(row_work, ignore_errors=True)
                                progress.advance(task_paint)

                        if t3_opts and t3_opts.materialize:
                            task_mat = progress.add_task(
                                "[cyan]Text3D: Materialize PBR (todos)[/cyan]",
                                total=len(texture_ok),
                            )
                            for idx in texture_ok:
                                row = rows[idx]
                                rec = results[idx]
                                progress.update(
                                    task_mat,
                                    description=f"[cyan]{row.id}[/cyan] · Materialize",
                                )
                                row_work = (
                                    batch_tmp
                                    / f"{idx:04d}_{_safe_row_dirname(row.id)}_3d"
                                )
                                painted = row_work / "painted.glb"
                                img_final, mesh_final = _paths_for_row_manifest(
                                    profile, manifest_dir, row
                                )
                                maps_tmp = row_work / "pbr_maps"
                                try:
                                    t_mat = _text3d_materialize_pbr_argv(
                                        text3d_bin,
                                        profile,
                                        painted,
                                        mesh_final,
                                        materialize_maps_dir=maps_tmp,
                                        row=row,
                                    )
                                    r5 = run_cmd(
                                        t_mat, extra_env=child_env, cwd=manifest_dir
                                    )
                                    if r5.returncode != 0:
                                        failures += 1
                                        err = (
                                            merge_subprocess_output(r5)
                                            or "text3d materialize-pbr falhou"
                                        )
                                        rec["status"] = "error"
                                        rec["error"] = err
                                        preview = (
                                            merge_subprocess_output(r5, max_chars=4000)
                                            or err
                                        )
                                        console.print(
                                            f"[red]materialize-pbr falhou[/red] {row.id}: {preview}"
                                        )
                                    elif not mesh_final.is_file():
                                        failures += 1
                                        rec["status"] = "error"
                                        rec["error"] = "materialize-pbr não produziu GLB"
                                        console.print(
                                            f"[red]sem GLB após materialize-pbr[/red] {row.id}"
                                        )
                                    else:
                                        _finalize_mesh_ok(rec, mesh_final, maps_tmp, row)
                                finally:
                                    shutil.rmtree(row_work, ignore_errors=True)
                                    append_log(rec)
                                    if (
                                        not continue_on_error
                                        and rec["status"] == "error"
                                    ):
                                        raise click.Abort()
                                    progress.advance(task_mat)
                    else:
                        task2 = progress.add_task(
                            "[cyan]Fase 2: Text3D[/cyan]",
                            total=len(pending_3d_indices),
                        )
                        for idx in pending_3d_indices:
                            row = rows[idx]
                            rec = results[idx]
                            progress.update(
                                task2, description=f"[cyan]{row.id}[/cyan] · Text3D"
                            )
                            row_work = batch_tmp / f"{idx:04d}_{_safe_row_dirname(row.id)}_3d"
                            row_work.mkdir(parents=True, exist_ok=True)
                            try:
                                img_final, mesh_final = _paths_for_row_manifest(
                                    profile, manifest_dir, row
                                )
                                mesh_tmp = row_work / "out.glb"
                                maps_tmp: Path | None = None
                                seed = _seed_for_row(profile, row.id)
                                if (
                                    t3_opts
                                    and t3_opts.materialize
                                    and t3_opts.materialize_save_maps
                                ):
                                    maps_tmp = row_work / "pbr_maps"

                                t3d_args = _text3d_argv(
                                    text3d_bin,
                                    profile,
                                    img_final,
                                    mesh_tmp,
                                    row,
                                    materialize_maps_dir=maps_tmp,
                                )
                                if seed is not None:
                                    t3d_args.extend(["--seed", str(seed)])
                                r3 = run_cmd(
                                    t3d_args, extra_env=child_env, cwd=manifest_dir
                                )
                                if r3.returncode != 0:
                                    failures += 1
                                    err = merge_subprocess_output(r3) or "text3d falhou"
                                    rec["status"] = "error"
                                    rec["error"] = err
                                    preview = merge_subprocess_output(r3, max_chars=4000) or err
                                    console.print(f"[red]text3d falhou[/red] {row.id}: {preview}")
                                elif not mesh_tmp.is_file():
                                    failures += 1
                                    rec["status"] = "error"
                                    rec["error"] = "text3d não produziu ficheiro GLB"
                                    console.print(f"[red]text3d sem GLB[/red] {row.id}")
                                else:
                                    _install_file(mesh_tmp, mesh_final)
                                    if (
                                        maps_tmp is not None
                                        and t3_opts
                                        and t3_opts.materialize_export_maps_to_output
                                    ):
                                        dst_maps = _materialize_maps_path_manifest(
                                            profile, manifest_dir, row
                                        )
                                        dst_maps.mkdir(parents=True, exist_ok=True)
                                        _install_maps_dir(maps_tmp, dst_maps)
                                    rec["mesh_path"] = _path_for_log(
                                        mesh_final, manifest_dir
                                    )
                                append_log(rec)
                                if not continue_on_error and rec["status"] == "error":
                                    raise click.Abort()
                            finally:
                                shutil.rmtree(row_work, ignore_errors=True)
                                progress.advance(task2)
        finally:
            shutil.rmtree(batch_tmp, ignore_errors=True)

        summary = Table(box=box.SIMPLE, show_header=False, title="[bold]Resumo[/bold]")
        summary.add_row("Linhas processadas", str(len(rows)))
        summary.add_row("Falhas", f"[red]{failures}[/red]" if failures else "[green]0[/green]")
        if log_path is not None:
            summary.add_row("Log JSONL", str(log_path))
        console.print(Panel(summary, border_style="dim", title="[bold]Batch[/bold]"))

        if failures:
            console.print(f"[yellow]Concluído com {failures} falha(s).[/yellow]")
            sys.exit(1)
        console.print("[green]Batch concluído com sucesso.[/green]")


def _text3d_argv(
    text3d_bin: str,
    profile: GameProfile,
    image_path: Path,
    mesh_path: Path,
    row: ManifestRow | None = None,
    *,
    materialize_maps_dir: Path | None = None,
    shape_only: bool = False,
) -> list[str]:
    """
    ``shape_only=True``: só Hunyuan (imagem → mesh), sem --texture/--materialize
    (batch em fases: shape → Paint → materialize-pbr).
    """
    args = [
        text3d_bin,
        "generate",
        "--from-image",
        str(image_path),
        "-o",
        str(mesh_path),
    ]
    t3 = profile.text3d
    if not t3:
        return args

    explicit_hunyuan = (
        t3.steps is not None
        or t3.octree_resolution is not None
        or t3.num_chunks is not None
    )
    if t3.preset and not explicit_hunyuan:
        args.extend(["--preset", t3.preset])
    if t3.steps is not None:
        args.extend(["--steps", str(t3.steps)])
    if t3.octree_resolution is not None:
        args.extend(["--octree-resolution", str(t3.octree_resolution)])
    if t3.num_chunks is not None:
        args.extend(["--num-chunks", str(t3.num_chunks)])
    if t3.model_subfolder:
        args.extend(["--model-subfolder", t3.model_subfolder])
    if t3.low_vram:
        args.append("--low-vram")
    if t3.texture and not shape_only:
        args.append("--texture")
    if t3.mc_level is not None:
        args.extend(["--mc-level", str(t3.mc_level)])
    if t3.no_mesh_repair:
        args.append("--no-mesh-repair")
    if t3.mesh_smooth is not None:
        args.extend(["--mesh-smooth", str(t3.mesh_smooth)])
    if t3.materialize and not shape_only:
        args.append("--materialize")
        if t3.materialize_save_maps:
            if row is None:
                raise ValueError(
                    "text3d.materialize_save_maps requer id do manifest (uso interno batch)"
                )
            maps_dir = (
                materialize_maps_dir
                if materialize_maps_dir is not None
                else _materialize_maps_path(profile, row)
            )
            args.extend(["--materialize-output-dir", str(maps_dir)])
        if t3.materialize_bin:
            args.extend(["--materialize-bin", t3.materialize_bin])
        if t3.materialize_no_invert:
            args.append("--materialize-no-invert")
    if t3.allow_shared_gpu:
        args.append("--allow-shared-gpu")
    if not t3.gpu_kill_others:
        args.append("--no-gpu-kill-others")
    # GPU pura: desativa CPU offload no Text2D e Paint (quando aplicável)
    if t3.full_gpu:
        args.append("--t2d-full-gpu")
        if t3.texture and not shape_only:
            args.append("--paint-full-gpu")
    return args


def _text3d_texture_argv(
    text3d_bin: str,
    profile: GameProfile,
    mesh_in: Path,
    image_path: Path,
    mesh_out: Path,
    *,
    with_materialize: bool,
    materialize_maps_dir: Path | None,
    row: ManifestRow,
) -> list[str]:
    """Subcomando ``text3d texture`` (Hunyuan3D-Paint); PBR opcional no mesmo passo."""
    args = [
        text3d_bin,
        "texture",
        str(mesh_in),
        "--image",
        str(image_path),
        "-o",
        str(mesh_out),
    ]
    t3 = profile.text3d
    if not t3:
        return args
    if with_materialize and t3.materialize:
        args.append("--materialize")
        if t3.materialize_save_maps:
            maps_dir = (
                materialize_maps_dir
                if materialize_maps_dir is not None
                else _materialize_maps_path(profile, row)
            )
            args.extend(["--materialize-output-dir", str(maps_dir)])
        if t3.materialize_bin:
            args.extend(["--materialize-bin", t3.materialize_bin])
        if t3.materialize_no_invert:
            args.append("--materialize-no-invert")
    if t3.allow_shared_gpu:
        args.append("--allow-shared-gpu")
    if not t3.gpu_kill_others:
        args.append("--no-gpu-kill-others")
    if t3.full_gpu:
        args.append("--paint-full-gpu")
    return args


def _text3d_materialize_pbr_argv(
    text3d_bin: str,
    profile: GameProfile,
    mesh_in: Path,
    mesh_out: Path,
    *,
    materialize_maps_dir: Path | None,
    row: ManifestRow,
) -> list[str]:
    """Subcomando ``text3d materialize-pbr`` (só Materialize, GLB já pintado)."""
    args = [
        text3d_bin,
        "materialize-pbr",
        str(mesh_in),
        "-o",
        str(mesh_out),
    ]
    t3 = profile.text3d
    if not t3:
        return args
    if t3.materialize_save_maps:
        maps_dir = (
            materialize_maps_dir
            if materialize_maps_dir is not None
            else _materialize_maps_path(profile, row)
        )
        args.extend(["--materialize-output-dir", str(maps_dir)])
    if t3.materialize_bin:
        args.extend(["--materialize-bin", t3.materialize_bin])
    if t3.materialize_no_invert:
        args.append("--materialize-no-invert")
    if t3.allow_shared_gpu:
        args.append("--allow-shared-gpu")
    if not t3.gpu_kill_others:
        args.append("--no-gpu-kill-others")
    return args


@main.command("resume")
@click.option(
    "--profile",
    "profile_path",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default="game.yaml",
)
@click.option(
    "--manifest",
    "manifest_path",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default="manifest.csv",
)
@click.option(
    "--presets-local",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default=None,
)
@click.option(
    "--log",
    "log_path",
    type=click.Path(dir_okay=False, path_type=Path),
    default=None,
    help="Ficheiro JSONL de log",
)
@click.option("--dry-run", is_flag=True, help="Mostra plano sem executar")
@click.option("--fail-fast", is_flag=True, help="Parar no primeiro erro")
@click.option(
    "--work-dir",
    type=click.Path(file_okay=False, path_type=Path),
    default=None,
    help="Pasta de trabalho persistente para shapes/painted (defeito: .gameassets_work/ junto ao manifest)",
)
def resume_cmd(
    profile_path: Path,
    manifest_path: Path,
    presets_local: Path | None,
    log_path: Path | None,
    dry_run: bool,
    fail_fast: bool,
    work_dir: Path | None,
) -> None:
    """Batch inteligente: analisa o estado de cada asset e executa apenas as fases pendentes.

    \b
    Detecta automaticamente por item:
      - PNG em falta  → text2d
      - shape em falta → text3d generate (shape)
      - paint em falta → text3d texture
      - GLB final em falta → materialize-pbr + copiar
      - tudo OK       → skip
    """
    import os

    profile, rows, _bundle, preset = _build_context(
        profile_path, manifest_path, presets_local
    )
    manifest_dir = manifest_path.resolve().parent
    t3_opts = profile.text3d

    try:
        text2d_bin: str | None = resolve_binary("TEXT2D_BIN", "text2d")
    except FileNotFoundError:
        text2d_bin = None
    try:
        text3d_bin: str | None = resolve_binary("TEXT3D_BIN", "text3d")
    except FileNotFoundError:
        text3d_bin = None

    if work_dir is None:
        work_dir = manifest_dir / ".gameassets_work"
    else:
        work_dir = work_dir.resolve()
    work_dir.mkdir(parents=True, exist_ok=True)

    child_env = subprocess_gpu_env()

    log_file = None
    if log_path:
        log_file = open(log_path, "a", encoding="utf-8")

    def append_log(rec: dict) -> None:
        if log_file:
            log_file.write(json.dumps(rec, ensure_ascii=False) + "\n")
            log_file.flush()

    # --- Análise de estado ---
    NEED_IMAGE = "need_image"
    NEED_SHAPE = "need_shape"
    NEED_PAINT = "need_paint"
    NEED_MATERIALIZE = "need_materialize"
    DONE = "done"

    want_texture = bool(t3_opts and t3_opts.texture)
    want_materialize = bool(t3_opts and t3_opts.materialize)

    items: list[dict[str, Any]] = []
    for idx, row in enumerate(rows):
        if not row.generate_3d:
            continue
        img_final, mesh_final = _paths_for_row_manifest(profile, manifest_dir, row)
        row_work = work_dir / _safe_row_dirname(row.id)
        shape_path = row_work / "shape.glb"
        painted_path = row_work / "painted.glb"

        maps_dir: Path | None = None
        if want_materialize and t3_opts and t3_opts.materialize_save_maps:
            maps_dir = _materialize_maps_path_manifest(profile, manifest_dir, row)

        if mesh_final.is_file():
            state = DONE
        elif want_materialize and painted_path.is_file():
            state = NEED_MATERIALIZE
        elif want_texture and shape_path.is_file():
            state = NEED_PAINT
        elif img_final.is_file():
            state = NEED_SHAPE
        else:
            state = NEED_IMAGE

        items.append({
            "idx": idx,
            "row": row,
            "state": state,
            "img_final": img_final,
            "mesh_final": mesh_final,
            "row_work": row_work,
            "shape_path": shape_path,
            "painted_path": painted_path,
            "maps_dir": maps_dir,
        })

    # --- Relatório ---
    counts = {NEED_IMAGE: 0, NEED_SHAPE: 0, NEED_PAINT: 0, NEED_MATERIALIZE: 0, DONE: 0}
    for it in items:
        counts[it["state"]] += 1

    plan_table = Table(title="[bold]Plano de execução[/bold]", box=box.ROUNDED, show_header=True)
    plan_table.add_column("Fase", style="bold")
    plan_table.add_column("Pendentes", justify="right")
    plan_table.add_column("Ação")
    plan_table.add_row("1. Imagem (text2d)", str(counts[NEED_IMAGE]),
                       "text2d generate" if counts[NEED_IMAGE] > 0 else "[green]OK[/green]")
    plan_table.add_row("2. Shape (hunyuan)", str(counts[NEED_SHAPE] + counts[NEED_IMAGE]),
                       "text3d generate --from-image" if (counts[NEED_SHAPE] + counts[NEED_IMAGE]) > 0 else "[green]OK[/green]")
    paint_pending = counts[NEED_PAINT] + counts[NEED_SHAPE] + counts[NEED_IMAGE]
    plan_table.add_row("3. Paint (textura)", str(paint_pending),
                       "text3d texture" if paint_pending > 0 else "[green]OK[/green]")
    mat_pending = counts[NEED_MATERIALIZE] + paint_pending
    if want_materialize:
        plan_table.add_row("4. Materialize PBR", str(mat_pending),
                           "text3d materialize-pbr" if mat_pending > 0 else "[green]OK[/green]")
    plan_table.add_row("[green]Concluídos[/green]", str(counts[DONE]), "[green]skip[/green]")
    console.print(plan_table)

    if all(it["state"] == DONE for it in items):
        console.print("[bold green]Todos os assets estão completos.[/bold green]")
        return

    if counts[NEED_IMAGE] > 0 and not text2d_bin:
        console.print("[yellow]AVISO: text2d não encontrado — 27 imagens serão saltadas.[/yellow]")
    if (counts[NEED_SHAPE] + counts[NEED_PAINT] + counts[NEED_MATERIALIZE]) > 0 and not text3d_bin:
        raise click.ClickException("text3d não encontrado. Define TEXT3D_BIN ou instala o pacote.")

    if dry_run:
        for it in items:
            if it["state"] != DONE:
                console.print(f"  [yellow]{it['state']}[/yellow] {it['row'].id}")
        return

    continue_on_error = not fail_fast
    failures = 0

    # --- Fase 1: Imagens ---
    need_img = [it for it in items if it["state"] == NEED_IMAGE]
    if need_img and text2d_bin:
        console.print(f"\n[bold cyan]Fase 1: Text2D ({len(need_img)} imagens)[/bold cyan]")
        with Progress(SpinnerColumn(), TextColumn("{task.description}"), BarColumn(),
                       TextColumn("{task.completed}/{task.total}"), TimeElapsedColumn(),
                       console=console) as progress:
            task = progress.add_task("[cyan]Text2D[/cyan]", total=len(need_img))
            for it in need_img:
                row = it["row"]
                progress.update(task, description=f"[cyan]{row.id}[/cyan] · Text2D")
                it["row_work"].mkdir(parents=True, exist_ok=True)
                tmp_img = it["row_work"] / f"image.{profile.image_ext}"
                prompt_2d = build_prompt(profile, preset, row, for_3d=False)
                argv = [text2d_bin, "generate", prompt_2d, "-o", str(tmp_img)]
                _append_text2d_profile_args(profile, argv)
                seed = _seed_for_row(profile, row.id)
                if seed is not None:
                    argv.extend(["--seed", str(seed)])
                r = run_cmd(argv, extra_env=child_env, cwd=manifest_dir)
                if r.returncode == 0 and tmp_img.is_file():
                    _install_file(tmp_img, it["img_final"])
                    it["state"] = NEED_SHAPE
                    console.print(f"  [green]OK[/green] {row.id}")
                else:
                    failures += 1
                    console.print(f"  [red]FAIL[/red] {row.id}")
                    if not continue_on_error:
                        break
                progress.advance(task)

    # --- Fase 2: Shape ---
    need_shape = [it for it in items if it["state"] == NEED_SHAPE]
    if need_shape and text3d_bin:
        console.print(f"\n[bold cyan]Fase 2: Shape ({len(need_shape)} meshes)[/bold cyan]")
        with Progress(SpinnerColumn(), TextColumn("{task.description}"), BarColumn(),
                       TextColumn("{task.completed}/{task.total}"), TimeElapsedColumn(),
                       console=console) as progress:
            task = progress.add_task("[cyan]Shape[/cyan]", total=len(need_shape))
            for it in need_shape:
                row = it["row"]
                progress.update(task, description=f"[cyan]{row.id}[/cyan] · shape")
                it["row_work"].mkdir(parents=True, exist_ok=True)
                seed = _seed_for_row(profile, row.id)
                t3d_args = _text3d_argv(
                    text3d_bin, profile, it["img_final"], it["shape_path"], row,
                    shape_only=True,
                )
                if seed is not None:
                    t3d_args.extend(["--seed", str(seed)])
                r = run_cmd(t3d_args, extra_env=child_env, cwd=manifest_dir)
                if r.returncode == 0 and it["shape_path"].is_file():
                    it["state"] = NEED_PAINT if want_texture else DONE
                    console.print(f"  [green]OK[/green] {row.id}")
                else:
                    failures += 1
                    err = merge_subprocess_output(r, max_chars=200) or "shape falhou"
                    console.print(f"  [red]FAIL[/red] {row.id}: {err}")
                    append_log({"id": row.id, "status": "error", "error": err})
                    if not continue_on_error:
                        break
                progress.advance(task)

    # --- Fase 3: Paint ---
    need_paint = [it for it in items if it["state"] == NEED_PAINT]
    if need_paint and text3d_bin:
        console.print(f"\n[bold cyan]Fase 3: Paint ({len(need_paint)} texturas)[/bold cyan]")
        with Progress(SpinnerColumn(), TextColumn("{task.description}"), BarColumn(),
                       TextColumn("{task.completed}/{task.total}"), TimeElapsedColumn(),
                       console=console) as progress:
            task = progress.add_task("[cyan]Paint[/cyan]", total=len(need_paint))
            for it in need_paint:
                row = it["row"]
                progress.update(task, description=f"[cyan]{row.id}[/cyan] · Paint")
                mesh_out = it["mesh_final"] if not want_materialize else it["painted_path"]
                t_tex = _text3d_texture_argv(
                    text3d_bin, profile, it["shape_path"], it["img_final"], mesh_out,
                    with_materialize=False, materialize_maps_dir=None, row=row,
                )
                r = run_cmd(t_tex, extra_env=child_env, cwd=manifest_dir)
                if r.returncode == 0 and mesh_out.is_file():
                    if want_materialize:
                        it["state"] = NEED_MATERIALIZE
                    else:
                        it["state"] = DONE
                        append_log({"id": row.id, "status": "ok", "mesh_path": str(it["mesh_final"])})
                    console.print(f"  [green]OK[/green] {row.id}")
                else:
                    failures += 1
                    err = merge_subprocess_output(r, max_chars=200) or "paint falhou"
                    console.print(f"  [red]FAIL[/red] {row.id}: {err}")
                    append_log({"id": row.id, "status": "error", "error": err})
                    if not continue_on_error:
                        break
                progress.advance(task)

    # --- Fase 4: Materialize PBR ---
    need_mat = [it for it in items if it["state"] == NEED_MATERIALIZE]
    if need_mat and text3d_bin and want_materialize:
        console.print(f"\n[bold cyan]Fase 4: Materialize PBR ({len(need_mat)} assets)[/bold cyan]")
        with Progress(SpinnerColumn(), TextColumn("{task.description}"), BarColumn(),
                       TextColumn("{task.completed}/{task.total}"), TimeElapsedColumn(),
                       console=console) as progress:
            task = progress.add_task("[cyan]Materialize[/cyan]", total=len(need_mat))
            for it in need_mat:
                row = it["row"]
                progress.update(task, description=f"[cyan]{row.id}[/cyan] · Materialize")
                maps_tmp = it["row_work"] / "pbr_maps"
                t_mat = _text3d_materialize_pbr_argv(
                    text3d_bin, profile, it["painted_path"], it["mesh_final"],
                    materialize_maps_dir=maps_tmp, row=row,
                )
                r = run_cmd(t_mat, extra_env=child_env, cwd=manifest_dir)
                if r.returncode == 0 and it["mesh_final"].is_file():
                    it["state"] = DONE
                    if it["maps_dir"] and maps_tmp.is_dir():
                        it["maps_dir"].mkdir(parents=True, exist_ok=True)
                        _install_maps_dir(maps_tmp, it["maps_dir"])
                    append_log({"id": row.id, "status": "ok", "mesh_path": str(it["mesh_final"])})
                    console.print(f"  [green]OK[/green] {row.id}")
                else:
                    failures += 1
                    err = merge_subprocess_output(r, max_chars=200) or "materialize falhou"
                    console.print(f"  [red]FAIL[/red] {row.id}: {err}")
                    append_log({"id": row.id, "status": "error", "error": err})
                    if not continue_on_error:
                        break
                progress.advance(task)

    if log_file:
        log_file.close()

    # --- Resumo final ---
    done_count = sum(1 for it in items if it["state"] == DONE)
    console.print(f"\n[bold green]Concluídos: {done_count}/{len(items)}[/bold green]"
                  f"  [red]Falhas: {failures}[/red]" if failures else "")
    if failures:
        sys.exit(1)


if __name__ == "__main__":
    main()
