"""Rigging3D — CLI principal."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Sequence
from pathlib import Path

import rich_click as click
from rich.console import Console

from . import __version__

console = Console()

DEFAULT_SKELETON_TASK = "configs/task/quick_inference_skeleton_articulationxl_ar_256.yaml"
DEFAULT_SKIN_TASK = "configs/task/quick_inference_unirig_skin.yaml"

_WIN32 = sys.platform == "win32"

# ---------------------------------------------------------------------------
# Resolução de caminhos
# ---------------------------------------------------------------------------


def _package_root() -> Path:
    """Raiz da árvore UniRig empacotada (configs/, src/, launch/)."""
    return Path(__file__).resolve().parent / "unirig"


def _resolve_root(explicit: Path | None) -> Path:
    if explicit is not None:
        root = explicit.expanduser().resolve()
    else:
        env = os.environ.get("RIGGING3D_ROOT", "").strip()
        root = Path(env).expanduser().resolve() if env else _package_root()

    if not (root / "configs").is_dir() or not (root / "src").is_dir():
        raise FileNotFoundError(
            f"Árvore de inferência não encontrada em {root} (falta configs/ ou src/). "
            "Define RIGGING3D_ROOT ou usa --root."
        )
    return root


def _resolve_python(explicit: str | None) -> str:
    if explicit:
        return explicit
    env = os.environ.get("RIGGING3D_PYTHON", "").strip()
    return env if env else sys.executable


def _shell_path(path: Path) -> str:
    s = str(path.expanduser().resolve())
    if _WIN32:
        s = s.replace("\\", "/")
    return s


# ---------------------------------------------------------------------------
# Bash
# ---------------------------------------------------------------------------


def _find_bash() -> str | None:
    if _WIN32:
        for c in (
            Path(r"C:\Program Files\Git\bin\bash.exe"),
            Path(r"C:\Program Files\Git\usr\bin\bash.exe"),
            Path(r"C:\Program Files (x86)\Git\bin\bash.exe"),
            Path(r"C:\msys64\usr\bin\bash.exe"),
        ):
            if c.is_file():
                return str(c)
        w = shutil.which("bash")
        if w:
            low = w.lower().replace("/", "\\")
            if "\\system32\\bash.exe" in low or "\\windowsapps\\" in low:
                return None
            return w
        return None
    return shutil.which("bash")


def _require_bash() -> None:
    if not _find_bash():
        raise click.ClickException(
            "bash não encontrado. No Windows: Git Bash ou MSYS2; noutros sistemas: bash no PATH."
        )


# ---------------------------------------------------------------------------
# Subprocess
# ---------------------------------------------------------------------------


def _make_env(root: Path, extra: dict[str, str] | None = None) -> dict[str, str]:
    merged = {**os.environ, **(extra or {})}
    root_s = str(root)
    pp = merged.get("PYTHONPATH", "")
    merged["PYTHONPATH"] = root_s if not pp else root_s + os.pathsep + pp
    return merged


def _run(cmd: list[str], *, root: Path, env: dict[str, str] | None = None) -> int:
    return subprocess.run(cmd, cwd=str(root), env=_make_env(root, env)).returncode


def _run_bash(root: Path, script: str, args: Sequence[str]) -> int:
    bash = _find_bash()
    if not bash:
        raise RuntimeError("bash não encontrado")
    full = root / script
    if not full.is_file():
        raise FileNotFoundError(f"Script em falta: {full}")
    return _run([bash, _shell_path(full), *args], root=root)


def _run_module(root: Path, py: str, module: str, args: Sequence[str]) -> int:
    return _run([py, "-m", module, *args], root=root)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


@click.group()
@click.version_option(version=__version__, prog_name="rigging3d")
@click.option(
    "--root",
    type=click.Path(file_okay=False, path_type=Path),
    default=None,
    envvar="RIGGING3D_ROOT",
    help="Raiz da árvore de inferência (configs/ + src/). Por defeito: pacote ou RIGGING3D_ROOT",
)
@click.option(
    "--python",
    "python_cmd",
    default=None,
    envvar="RIGGING3D_PYTHON",
    help="Interpretador Python (conda/venv).",
)
@click.pass_context
def cli(ctx: click.Context, root: Path | None, python_cmd: str | None) -> None:
    """Rigging3D — auto-rigging 3D (skeleton, skinning, merge)."""
    ctx.ensure_object(dict)


def _ctx_root_py(ctx: click.Context) -> tuple[Path, str]:
    p = ctx.parent.params if ctx.parent is not None else ctx.params
    try:
        root = _resolve_root(p.get("root"))
    except FileNotFoundError as e:
        raise click.ClickException(str(e)) from e
    return root, _resolve_python(p.get("python_cmd"))


# --- skeleton ---


@cli.command("skeleton")
@click.option("--input", "-i", "input_path", type=click.Path(path_type=Path), default=None)
@click.option("--output", "-o", "output_path", type=click.Path(path_type=Path), default=None)
@click.option("--seed", type=int, default=12345, show_default=True)
@click.option("--skeleton-task", default=DEFAULT_SKELETON_TASK, show_default=True, help="YAML de task")
@click.option("--input-dir", type=click.Path(file_okay=False, path_type=Path), default=None)
@click.option("--output-dir", type=click.Path(file_okay=False, path_type=Path), default=None)
@click.pass_context
def skeleton_cmd(
    ctx: click.Context,
    input_path: Path | None,
    output_path: Path | None,
    seed: int,
    skeleton_task: str,
    input_dir: Path | None,
    output_dir: Path | None,
) -> None:
    """Gera skeleton (FBX) a partir de mesh (.glb/.obj/…)."""
    root, _py = _ctx_root_py(ctx)
    _require_bash()
    _validate_io(input_path, output_path, input_dir, output_dir)
    args: list[str] = ["--seed", str(seed), "--skeleton_task", skeleton_task]
    args += _io_args(input_path, output_path, input_dir, output_dir)
    rc = _run_bash(root, "launch/inference/generate_skeleton.sh", args)
    if rc != 0:
        raise click.ClickException(f"generate_skeleton.sh terminou com código {rc}")
    console.print("[green]Skeleton concluído.[/green]")


# --- skin ---


@cli.command("skin")
@click.option("--input", "-i", "input_path", type=click.Path(path_type=Path), default=None)
@click.option("--output", "-o", "output_path", type=click.Path(path_type=Path), default=None)
@click.option("--seed", type=int, default=12345, show_default=True)
@click.option("--skin-task", default=DEFAULT_SKIN_TASK, show_default=True, help="YAML de task")
@click.option("--input-dir", type=click.Path(file_okay=False, path_type=Path), default=None)
@click.option("--output-dir", type=click.Path(file_okay=False, path_type=Path), default=None)
@click.option("--data-name", default="raw_data.npz", show_default=True)
@click.pass_context
def skin_cmd(
    ctx: click.Context,
    input_path: Path | None,
    output_path: Path | None,
    seed: int,
    skin_task: str,
    input_dir: Path | None,
    output_dir: Path | None,
    data_name: str,
) -> None:
    """Prevê pesos de skinning a partir do FBX com skeleton."""
    root, _py = _ctx_root_py(ctx)
    _require_bash()
    _validate_io(input_path, output_path, input_dir, output_dir)
    args: list[str] = ["--seed", str(seed), "--skin_task", skin_task, "--data_name", data_name]
    args += _io_args(input_path, output_path, input_dir, output_dir)
    rc = _run_bash(root, "launch/inference/generate_skin.sh", args)
    if rc != 0:
        raise click.ClickException(f"generate_skin.sh terminou com código {rc}")
    console.print("[green]Skinning concluído.[/green]")


# --- merge ---


@cli.command("merge")
@click.option("--source", "-s", type=click.Path(path_type=Path), required=True, help="FBX skin")
@click.option("--target", "-t", type=click.Path(path_type=Path), required=True, help="Mesh original")
@click.option("--output", "-o", type=click.Path(path_type=Path), required=True)
@click.option("--require-suffix", default="obj,fbx,FBX,dae,glb,gltf,vrm", show_default=True)
@click.pass_context
def merge_cmd(ctx: click.Context, source: Path, target: Path, output: Path, require_suffix: str) -> None:
    """Combina resultado da fase skin com o mesh original (GLB rigado)."""
    root, py = _ctx_root_py(ctx)
    args = [
        f"--require_suffix={require_suffix}",
        "--num_runs=1",
        "--id=0",
        f"--source={_shell_path(source)}",
        f"--target={_shell_path(target)}",
        f"--output={_shell_path(output)}",
    ]
    rc = _run_module(root, py, "src.inference.merge", args)
    if rc != 0:
        raise click.ClickException(f"merge terminou com código {rc}")
    console.print("[green]Merge concluído.[/green]")


# --- pipeline ---


@cli.command("pipeline")
@click.option("--input", "-i", "mesh", type=click.Path(exists=True, path_type=Path), required=True)
@click.option("--output", "-o", "out", type=click.Path(path_type=Path), required=True)
@click.option("--work-dir", type=click.Path(file_okay=False, path_type=Path), default=None, help="Dir intermédio")
@click.option("--seed", type=int, default=12345, show_default=True)
@click.option("--keep-temp", is_flag=True, help="Não apagar work-dir temporário")
@click.pass_context
def pipeline_cmd(ctx: click.Context, mesh: Path, out: Path, work_dir: Path | None, seed: int, keep_temp: bool) -> None:
    """Encadeia skeleton → skin → merge até um GLB rigado."""
    root, py = _ctx_root_py(ctx)
    _require_bash()

    cleanup: Path | None = None
    if work_dir is None:
        cleanup = Path(tempfile.mkdtemp(prefix="rigging3d_"))
        wd = cleanup
    else:
        wd = work_dir
        wd.mkdir(parents=True, exist_ok=True)

    skel = wd / "_skeleton.fbx"
    skin = wd / "_skin.fbx"
    try:
        rc = _run_bash(
            root, "launch/inference/generate_skeleton.sh",
            ["--input", _shell_path(mesh), "--output", _shell_path(skel), "--seed", str(seed)],
        )
        if rc != 0:
            raise click.ClickException(f"skeleton falhou ({rc})")

        rc = _run_bash(
            root, "launch/inference/generate_skin.sh",
            ["--input", _shell_path(skel), "--output", _shell_path(skin), "--seed", str(seed)],
        )
        if rc != 0:
            raise click.ClickException(f"skin falhou ({rc})")

        rc = _run_module(
            root, py, "src.inference.merge",
            [
                "--require_suffix=obj,fbx,FBX,dae,glb,gltf,vrm", "--num_runs=1", "--id=0",
                f"--source={_shell_path(skin)}", f"--target={_shell_path(mesh)}", f"--output={_shell_path(out)}",
            ],
        )
        if rc != 0:
            raise click.ClickException(f"merge falhou ({rc})")
    finally:
        if cleanup is not None and not keep_temp:
            shutil.rmtree(cleanup, ignore_errors=True)

    console.print(f"[green]Pipeline concluído:[/green] {out}")


# ---------------------------------------------------------------------------
# Helpers I/O
# ---------------------------------------------------------------------------


def _validate_io(
    input_path: Path | None, output_path: Path | None, input_dir: Path | None, output_dir: Path | None,
) -> None:
    if input_dir is not None:
        if output_dir is None:
            raise click.ClickException("Com --input-dir indica também --output-dir.")
    elif input_path is None or output_path is None:
        raise click.ClickException("Indica --input e --output, ou --input-dir e --output-dir.")


def _io_args(
    input_path: Path | None, output_path: Path | None, input_dir: Path | None, output_dir: Path | None,
) -> list[str]:
    args: list[str] = []
    if input_dir is not None:
        args += ["--input_dir", _shell_path(input_dir), "--output_dir", _shell_path(output_dir)]
    else:
        assert input_path is not None and output_path is not None
        args += ["--input", _shell_path(input_path), "--output", _shell_path(output_path)]
        if output_dir is not None:
            args += ["--output_dir", _shell_path(output_dir)]
    return args


# ---------------------------------------------------------------------------
# Entry
# ---------------------------------------------------------------------------


def main() -> None:
    cli(obj={})


if __name__ == "__main__":
    main()
