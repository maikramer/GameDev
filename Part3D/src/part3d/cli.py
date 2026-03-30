"""Part3D CLI — decomposição de meshes 3D em partes semânticas."""

from __future__ import annotations

import sys
import time
from pathlib import Path

import click
import rich_click as rclick

from . import defaults as _d

rclick.rich_click.USE_RICH_MARKUP = True
rclick.rich_click.SHOW_ARGUMENTS = True


@click.group()
@click.version_option(package_name="part3d")
def main() -> None:
    """Part3D — Decomposição semântica de meshes 3D via Hunyuan3D-Part."""
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(line_buffering=True)
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(line_buffering=True)


@main.command()
@click.argument("mesh_path", type=click.Path(exists=True))
@click.option("-o", "--output", "output_path", type=click.Path(), default=None, help="Caminho de saída (.glb)")
@click.option("--output-segmented", type=click.Path(), default=None, help="Exportar mesh segmentada (cores por parte)")
@click.option(
    "--octree-resolution",
    type=int,
    default=None,
    help="Resolução do octree (default: autotune por geometria/VRAM)",
)
@click.option(
    "--steps",
    type=int,
    default=None,
    help="Passos DiT (default: autotune)",
)
@click.option(
    "--num-chunks",
    type=int,
    default=None,
    help="Chunks marching cubes (default: autotune)",
)
@click.option("--seed", type=int, default=42, show_default=True)
@click.option(
    "--no-auto-tune",
    is_flag=True,
    help="Desactivar ajuste automático (usa valores fixos de defaults.py)",
)
@click.option("--no-cpu-offload", is_flag=True, help="Desactivar CPU offloading (requer >10 GB VRAM)")
@click.option("--device", type=str, default=None, help="Forçar device (cuda/cpu)")
@click.option("--segment-only", is_flag=True, help="Apenas segmentar, sem gerar partes")
@click.option("-v", "--verbose", is_flag=True)
def decompose(
    mesh_path: str,
    output_path: str | None,
    output_segmented: str | None,
    octree_resolution: int | None,
    steps: int | None,
    num_chunks: int | None,
    seed: int,
    no_auto_tune: bool,
    no_cpu_offload: bool,
    device: str | None,
    segment_only: bool,
    verbose: bool,
) -> None:
    """Decompõe uma mesh 3D em partes semânticas.

    Usa P3-SAM para segmentação e X-Part para geração das partes.
    Optimizado para ~6 GB VRAM com CPU offloading sequencial.
    """
    from gamedev_shared.env import ensure_pytorch_cuda_alloc_conf

    ensure_pytorch_cuda_alloc_conf()

    from rich.console import Console
    from rich.panel import Panel

    console = Console()

    mesh_name = Path(mesh_path).stem
    if output_path is None:
        output_path = str(Path(mesh_path).parent / f"{mesh_name}_parts.glb")
    if output_segmented is None:
        output_segmented = str(Path(mesh_path).parent / f"{mesh_name}_segmented.glb")

    mode = "fixo (defaults)" if no_auto_tune else "autotune (geometria + VRAM)"
    oc_disp = octree_resolution if octree_resolution is not None else "auto"
    st_disp = steps if steps is not None else "auto"
    console.print(
        Panel.fit(
            f"[bold]Part3D[/] — Decomposição de [cyan]{Path(mesh_path).name}[/]\n"
            f"Saída: [green]{output_path}[/]\n"
            f"Modo: {mode}\n"
            f"Octree: {oc_disp} | Steps: {st_disp} | Seed: {seed}\n"
            f"CPU Offload: {'[green]SIM[/]' if not no_cpu_offload else '[red]NÃO[/]'}",
            title="Hunyuan3D-Part",
        )
    )

    import torch

    if torch.cuda.is_available():
        gpu_info = torch.cuda.get_device_properties(0)
        vram_gb = gpu_info.total_memory / (1024**3)
        console.print(f"GPU: {gpu_info.name} ({vram_gb:.1f} GB VRAM)")
        if vram_gb < 6.0 and not no_cpu_offload:
            console.print("[yellow]Aviso: VRAM < 6 GB — resultado pode ser instável[/]")
    else:
        console.print("[yellow]Sem CUDA — execução em CPU (muito lento)[/]")

    import numpy as np
    import trimesh

    from .pipeline import Part3DPipeline

    t_start = time.time()

    def _gen_kwargs() -> dict:
        if no_auto_tune:
            return {
                "octree_resolution": octree_resolution
                if octree_resolution is not None
                else _d.DEFAULT_OCTREE_RESOLUTION,
                "num_inference_steps": steps if steps is not None else _d.DEFAULT_NUM_INFERENCE_STEPS,
                "num_chunks": num_chunks if num_chunks is not None else _d.DEFAULT_NUM_CHUNKS,
            }
        out: dict = {}
        if octree_resolution is not None:
            out["octree_resolution"] = octree_resolution
        if steps is not None:
            out["num_inference_steps"] = steps
        if num_chunks is not None:
            out["num_chunks"] = num_chunks
        return out

    with Part3DPipeline(
        device=device,
        cpu_offload=not no_cpu_offload,
        verbose=verbose,
        autotune=not no_auto_tune,
    ) as pipe:
        if segment_only:
            mesh = trimesh.load(mesh_path, force="mesh", process=False)
            _aabb, face_ids, clean_mesh = pipe.segment(mesh, seed=seed)

            color_map = {}
            for uid in np.unique(face_ids):
                if uid < 0:
                    continue
                color_map[uid] = np.random.randint(0, 255, size=3)

            face_colors = np.array([color_map.get(fid, [0, 0, 0]) for fid in face_ids], dtype=np.uint8)
            clean_mesh.visual.face_colors = face_colors
            clean_mesh.export(output_segmented)
            console.print(f"[green]Mesh segmentada gravada em:[/] {output_segmented}")

        else:
            parts_scene, face_ids, clean_mesh = pipe(mesh_path, seed=seed, **_gen_kwargs())

            parts_scene.export(output_path)
            console.print(f"[green]Partes gravadas em:[/] {output_path}")

            # Gravar segmentação também
            color_map = {}
            for uid in np.unique(face_ids):
                if uid < 0:
                    continue
                color_map[uid] = np.random.randint(0, 255, size=3)

            face_colors = np.array([color_map.get(fid, [0, 0, 0]) for fid in face_ids], dtype=np.uint8)
            clean_mesh.visual.face_colors = face_colors
            clean_mesh.export(output_segmented)
            console.print(f"[green]Mesh segmentada gravada em:[/] {output_segmented}")

    elapsed = time.time() - t_start
    console.print(f"\n[bold green]Concluído em {elapsed:.1f}s[/]")


if __name__ == "__main__":
    main()
